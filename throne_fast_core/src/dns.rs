use core::slice;

pub const DNS_TYPE_TXT: u16 = 16;
pub const DNS_CLASS_IN: u16 = 1;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThroneFastXdnsResult {
    pub matched: u8,
    pub rcode: u8,
    pub has_txt: u8,
    pub _pad: u8,
}

#[inline(always)]
fn read_be16(pkt: &[u8], off: usize) -> Option<u16> {
    if off.checked_add(2)? > pkt.len() {
        return None;
    }
    Some(u16::from_be_bytes([pkt[off], pkt[off + 1]]))
}

/// Unpack a wire-format DNS name (RFC 1035 with bounded compression pointers) into `out_buf`.
/// Returns `(decoded_len, next_wire_offset_after_name)` on success.
/// Canonical format matches `dnsmessage.Name`: labels separated by `.` ending with `.`.
pub fn unpack_dns_name(pkt: &[u8], start: usize, out_buf: &mut [u8; 256]) -> Option<(usize, usize)> {
    let mut cur = start;
    let mut jumped = false;
    let mut return_off = 0usize;
    let mut out_len = 0usize;
    let mut hops = 0usize;

    loop {
        if cur >= pkt.len() {
            return None;
        }
        let len_byte = pkt[cur];
        if len_byte == 0 {
            let end_off = if jumped { return_off } else { cur + 1 };
            if out_len == 0 {
                // Root domain "."
                out_buf[0] = b'.';
                return Some((1, end_off));
            }
            return Some((out_len, end_off));
        }

        let tag = len_byte & 0xC0;
        if tag == 0xC0 {
            if cur + 1 >= pkt.len() {
                return None;
            }
            let ptr = (((len_byte & 0x3F) as usize) << 8) | (pkt[cur + 1] as usize);
            if ptr >= pkt.len() {
                return None;
            }
            if !jumped {
                return_off = cur + 2;
                jumped = true;
            }
            hops += 1;
            if hops > 10 {
                return None; // Compression pointer cycle / depth guard (matches Go dnsmessage)
            }
            cur = ptr;
        } else if tag == 0 {
            let label_len = len_byte as usize;
            if label_len > 63 {
                return None;
            }
            let label_start = cur + 1;
            let label_end = label_start.checked_add(label_len)?;
            if label_end > pkt.len() {
                return None;
            }
            // Reject literal '.' inside a wire label (matches Go dnsmessage.Name.unpack).
            if pkt[label_start..label_end].contains(&b'.') {
                return None;
            }
            // Match Go dnsmessage.nonEncodedNameMax (254): out_len + label_len + trailing '.' <= 254.
            if out_len + label_len >= 254 {
                return None;
            }
            out_buf[out_len..out_len + label_len].copy_from_slice(&pkt[label_start..label_end]);
            out_len += label_len;
            out_buf[out_len] = b'.';
            out_len += 1;
            cur = label_end;
        } else {
            // Extended/reserved label types 01 / 10 invalid in standard DNS names
            return None;
        }
    }
}

#[inline]
fn dns_name_eq_ignore_ascii_case(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for i in 0..a.len() {
        if !a[i].eq_ignore_ascii_case(&b[i]) {
            return false;
        }
    }
    true
}

/// Skip a wire DNS name matching Go `dnsmessage.skipName` (validates literals & tag bits, stops on 2-byte pointer).
fn skip_dns_name(pkt: &[u8], start: usize) -> Option<usize> {
    let mut cur = start;
    loop {
        if cur >= pkt.len() {
            return None;
        }
        let b = pkt[cur];
        let tag = b & 0xC0;
        if tag == 0 {
            if b == 0 {
                return Some(cur + 1);
            }
            let next = cur + 1 + (b as usize);
            if next > pkt.len() {
                return None;
            }
            cur = next;
        } else if tag == 0xC0 {
            if cur + 1 >= pkt.len() {
                return None;
            }
            return Some(cur + 2);
        } else {
            return None;
        }
    }
}

pub fn match_xdns_reply(pkt: &[u8], expected_id: u16, expected_name: &[u8]) -> ThroneFastXdnsResult {
    let miss = ThroneFastXdnsResult {
        matched: 0,
        rcode: 0,
        has_txt: 0,
        _pad: 0,
    };
    if pkt.len() < 12 {
        return miss;
    }
    let id = u16::from_be_bytes([pkt[0], pkt[1]]);
    let flags = u16::from_be_bytes([pkt[2], pkt[3]]);
    let qr = (flags & 0x8000) != 0;
    let rcode = (flags & 0x000F) as u8;
    if !qr || id != expected_id {
        return miss;
    }

    let qdcount = u16::from_be_bytes([pkt[4], pkt[5]]);
    let ancount = u16::from_be_bytes([pkt[6], pkt[7]]);
    let mut off = 12usize;

    if qdcount == 0 {
        return ThroneFastXdnsResult {
            matched: u8::from(rcode != 0),
            rcode,
            has_txt: 0,
            _pad: 0,
        };
    }

    let mut name_buf = [0u8; 256];
    let Some((qname_len, after_qname)) = unpack_dns_name(pkt, off, &mut name_buf) else {
        return miss;
    };
    if !dns_name_eq_ignore_ascii_case(&name_buf[..qname_len], expected_name) {
        return miss;
    }
    // Check QTYPE + QCLASS (4 bytes)
    let Some(after_q) = after_qname.checked_add(4) else {
        return miss;
    };
    if after_q > pkt.len() {
        return miss;
    }
    off = after_q;

    let reply_base = ThroneFastXdnsResult {
        matched: 1,
        rcode,
        has_txt: 0,
        _pad: 0,
    };

    // Skip remaining questions cleanly
    for _ in 1..qdcount {
        let Some(after_name) = skip_dns_name(pkt, off) else {
            return reply_base;
        };
        let Some(next) = after_name.checked_add(4) else {
            return reply_base;
        };
        if next > pkt.len() {
            return reply_base;
        }
        off = next;
    }

    // Scan Answer section for IN TXT matching query name
    for _ in 0..ancount {
        let Some((aname_len, after_name)) = unpack_dns_name(pkt, off, &mut name_buf) else {
            return reply_base;
        };
        let Some(rtype) = read_be16(pkt, after_name) else {
            return reply_base;
        };
        let Some(rclass) = read_be16(pkt, after_name + 2) else {
            return reply_base;
        };
        // TTL = 4 bytes at +4..+8, RDLEN at +8..+10
        let Some(rdlen) = read_be16(pkt, after_name + 8) else {
            return reply_base;
        };
        let Some(rdata_start) = after_name.checked_add(10) else {
            return reply_base;
        };

        if rtype == DNS_TYPE_TXT
            && rclass == DNS_CLASS_IN
            && dns_name_eq_ignore_ascii_case(&name_buf[..aname_len], expected_name)
        {
            return ThroneFastXdnsResult {
                matched: 1,
                rcode,
                has_txt: 1,
                _pad: 0,
            };
        }
        let Some(rdata_end) = rdata_start.checked_add(rdlen as usize) else {
            return reply_base;
        };
        if rdata_end > pkt.len() {
            return reply_base;
        }
        off = rdata_end;
    }

    reply_base
}

/// Validates RFC 1035 DNS packet structure (header, questions, answers, and compression pointers).
/// Returns 1 if structurally valid, 0 otherwise.
///
/// # Safety
/// `pkt` must be valid for `len` bytes if `len > 0`.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_dns_validate_packet(pkt: *const u8, len: usize) -> i32 {
    if pkt.is_null() || len < 12 {
        return 0;
    }
    let b = unsafe { slice::from_raw_parts(pkt, len) };
    let qd = u16::from_be_bytes([b[4], b[5]]);
    let an = u16::from_be_bytes([b[6], b[7]]);
    let mut off = 12usize;
    let mut scratch = [0u8; 256];
    for _ in 0..qd {
        let Some((_, n_off)) = unpack_dns_name(b, off, &mut scratch) else {
            return 0;
        };
        let Some(next) = n_off.checked_add(4) else {
            return 0;
        };
        if next > b.len() {
            return 0;
        }
        off = next;
    }
    for _ in 0..an {
        let Some((_, n_off)) = unpack_dns_name(b, off, &mut scratch) else {
            return 0;
        };
        let Some(rdlen) = read_be16(b, n_off + 8) else {
            return 0;
        };
        let Some(next) = n_off.checked_add(10 + rdlen as usize) else {
            return 0;
        };
        if next > b.len() {
            return 0;
        }
        off = next;
    }
    1
}

/// Fast zero-alloc XDNS TXT reply matcher matching `core/internal/scan.parseXDNSReply`.
/// `expected_name` is canonical dot-terminated ASCII DNS name (e.g. `"abc.example.com."`).
///
/// # Safety
/// `pkt` readable for `pkt_len` bytes, `expected_name` readable for `name_len` bytes, `out` writable.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_dns_match_xdns_reply(
    pkt: *const u8,
    pkt_len: usize,
    expected_id: u16,
    expected_name: *const u8,
    name_len: usize,
    out: *mut ThroneFastXdnsResult,
) -> i32 {
    if pkt.is_null() || expected_name.is_null() || out.is_null() {
        return -1;
    }
    let pkt_slice = unsafe { slice::from_raw_parts(pkt, pkt_len) };
    let name_slice = unsafe { slice::from_raw_parts(expected_name, name_len) };
    let res = match_xdns_reply(pkt_slice, expected_id, name_slice);
    unsafe { *out = res };
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_xdns_reply(id: u16, rcode: u8, with_txt: bool, compressed_answer: bool) -> Vec<u8> {
        let mut p = Vec::new();
        p.extend_from_slice(&id.to_be_bytes());
        let flags: u16 = 0x8180 | (rcode as u16 & 0x0F);
        p.extend_from_slice(&flags.to_be_bytes());
        p.extend_from_slice(&1u16.to_be_bytes()); // qd=1
        p.extend_from_slice(&(if with_txt { 1u16 } else { 0u16 }).to_be_bytes());
        p.extend_from_slice(&0u32.to_be_bytes());
        // QNAME at offset 12: 3abc7example3com0
        p.extend_from_slice(&[3, b'a', b'b', b'c', 7, b'e', b'x', b'a', b'm', b'p', b'l', b'e', 3, b'c', b'o', b'm', 0]);
        p.extend_from_slice(&DNS_TYPE_TXT.to_be_bytes());
        p.extend_from_slice(&DNS_CLASS_IN.to_be_bytes());
        if with_txt {
            if compressed_answer {
                p.extend_from_slice(&[0xC0, 12]);
            } else {
                p.extend_from_slice(&[3, b'A', b'B', b'C', 7, b'e', b'x', b'a', b'm', b'p', b'l', b'e', 3, b'c', b'o', b'm', 0]);
            }
            p.extend_from_slice(&DNS_TYPE_TXT.to_be_bytes());
            p.extend_from_slice(&DNS_CLASS_IN.to_be_bytes());
            p.extend_from_slice(&60u32.to_be_bytes());
            p.extend_from_slice(&3u16.to_be_bytes()); // rdlen=3
            p.extend_from_slice(&[2, b'o', b'k']);
        }
        p
    }

    #[test]
    fn xdns_reply_compressed_and_case_insensitive() {
        let pkt = sample_xdns_reply(0x4242, 0, true, true);
        let r = match_xdns_reply(&pkt, 0x4242, b"abc.example.com.");
        assert_eq!(r.matched, 1);
        assert_eq!(r.rcode, 0);
        assert_eq!(r.has_txt, 1);

        let pkt2 = sample_xdns_reply(0x4242, 0, true, false);
        let r2 = match_xdns_reply(&pkt2, 0x4242, b"abc.example.com.");
        assert_eq!(r2.matched, 1);
        assert_eq!(r2.has_txt, 1);
    }

    #[test]
    fn xdns_compression_loop_and_truncated_safe() {
        let mut bad = vec![0u8; 18];
        bad[0..4].copy_from_slice(&[0x12, 0x34, 0x80, 0x00]);
        bad[4..6].copy_from_slice(&1u16.to_be_bytes());
        // Self-referential pointer loop at offset 12 -> 0xC0 0x0C with QTYPE+QCLASS fitting in 18 bytes
        bad[12] = 0xC0;
        bad[13] = 12;
        bad[15] = 16;
        bad[17] = 1;
        let r = match_xdns_reply(&bad, 0x1234, b"a.com.");
        assert_eq!(r.matched, 0);
        assert_eq!(unsafe { throne_fast_dns_validate_packet(bad.as_ptr(), bad.len()) }, 0);

        // Out-of-bounds compression pointer
        let mut oob = bad.clone();
        oob[13] = 200;
        assert_eq!(unsafe { throne_fast_dns_validate_packet(oob.as_ptr(), oob.len()) }, 0);

        // Valid compressed packet must validate as 1
        let ok = sample_xdns_reply(0x4242, 0, true, true);
        assert_eq!(unsafe { throne_fast_dns_validate_packet(ok.as_ptr(), ok.len()) }, 1);

        // Label containing literal '.' must be rejected (matches Go dnsmessage)
        let mut dot_label = vec![0x12, 0x34, 0x80, 0x00, 0, 1, 0, 0, 0, 0, 0, 0];
        dot_label.extend_from_slice(&[3, b'a', b'.', b'b', 0, 0, 16, 0, 1]);
        assert_eq!(match_xdns_reply(&dot_label, 0x1234, b"a.b.").matched, 0);
        assert_eq!(unsafe { throne_fast_dns_validate_packet(dot_label.as_ptr(), dot_label.len()) }, 0);
    }

    #[test]
    fn xdns_rejects_more_than_ten_pointer_hops() {
        // Build chain of 11 pointers: at offset 12 -> 14 -> ... -> 32 -> root 0
        let mut pkt = vec![0x12, 0x34, 0x80, 0x00, 0, 1, 0, 0, 0, 0, 0, 0];
        for hop in 0..11usize {
            let target = 12 + (hop + 1) * 2;
            pkt.push(0xC0);
            pkt.push(target as u8);
        }
        pkt.extend_from_slice(&[1, b'a', 0, 0, 16, 0, 1]);
        let mut buf = [0u8; 256];
        assert!(unpack_dns_name(&pkt, 12, &mut buf).is_none());
    }
}
