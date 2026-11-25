use core::slice;
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::{self, FromStr};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ThroneFastCidr {
    pub addr: [u8; 16], // IPv4 uses addr[0..4], IPv6 uses addr[0..16] in big-endian
    pub prefix_len: u8,
    pub family: u8, // 4 = IPv4, 6 = IPv6
    pub _pad: [u8; 6],
}

impl ThroneFastCidr {
    pub fn from_v4(ip: u32, prefix_len: u8) -> Self {
        let mut addr = [0u8; 16];
        addr[..4].copy_from_slice(&ip.to_be_bytes());
        Self {
            addr,
            prefix_len,
            family: 4,
            _pad: [0; 6],
        }
    }

    pub fn from_v6(ip: u128, prefix_len: u8) -> Self {
        Self {
            addr: ip.to_be_bytes(),
            prefix_len,
            family: 6,
            _pad: [0; 6],
        }
    }
}

pub fn parse_cidr(input: &str) -> Option<ThroneFastCidr> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (ip_part, prefix_part) = match trimmed.split_once('/') {
        Some((a, b)) => (a.trim(), Some(b.trim())),
        None => (trimmed, None),
    };

    // Strip optional square brackets around IPv6 literal e.g. [::1]
    let ip_str = if ip_part.starts_with('[') && ip_part.ends_with(']') && ip_part.len() >= 2 {
        &ip_part[1..ip_part.len() - 1]
    } else {
        ip_part
    };

    match IpAddr::from_str(ip_str).ok()? {
        IpAddr::V4(v4) => {
            let plen = match prefix_part {
                Some(p) => parse_u8_strict(p, 32)?,
                None => 32,
            };
            let mask = if plen == 0 {
                0u32
            } else {
                !0u32 << (32 - plen)
            };
            let masked = u32::from(v4) & mask;
            Some(ThroneFastCidr::from_v4(masked, plen))
        }
        IpAddr::V6(v6) => {
            let plen = match prefix_part {
                Some(p) => parse_u8_strict(p, 128)?,
                None => 128,
            };
            let mask = if plen == 0 {
                0u128
            } else {
                !0u128 << (128 - plen)
            };
            let masked = u128::from(v6) & mask;
            Some(ThroneFastCidr::from_v6(masked, plen))
        }
    }
}

#[inline]
fn parse_u8_strict(s: &str, max: u8) -> Option<u8> {
    if s.is_empty() || s.len() > 3 {
        return None;
    }
    let mut val: u16 = 0;
    for &b in s.as_bytes() {
        if !b.is_ascii_digit() {
            return None;
        }
        val = val * 10 + (b - b'0') as u16;
    }
    if val <= max as u16 {
        Some(val as u8)
    } else {
        None
    }
}

pub fn format_cidr(cidr: &ThroneFastCidr, buf: &mut [u8]) -> Option<usize> {
    let mut cur = std::io::Cursor::new(buf);
    match cidr.family {
        4 => {
            let v4 = Ipv4Addr::new(cidr.addr[0], cidr.addr[1], cidr.addr[2], cidr.addr[3]);
            write!(cur, "{}/{}", v4, cidr.prefix_len).ok()?;
        }
        6 => {
            let v6 = Ipv6Addr::from(cidr.addr);
            write!(cur, "{}/{}", v6, cidr.prefix_len).ok()?;
        }
        _ => return None,
    }
    Some(cur.position() as usize)
}

/// Parse a CIDR or bare IP with IP-list semantics:
/// - allows `[v6]/prefix` or `[v6/prefix]`
/// - strips optional `%zone` on IPv6 literals
/// - unmaps `::ffff:w.x.y.z/96..128` to IPv4 `/0..32`
/// - rejects `/0` prefix length
pub fn parse_cidr_compact(input: &str) -> Option<ThroneFastCidr> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (body, prefix_part) = if trimmed.starts_with('[') {
        let close = trimmed.find(']')?;
        let inner = &trimmed[1..close];
        let suffix = &trimmed[close + 1..];
        if !suffix.is_empty() && !suffix.starts_with('/') {
            return None;
        }
        if let Some((ip_p, pre_p)) = inner.split_once('/') {
            if !suffix.is_empty() {
                return None;
            }
            (ip_p, Some(pre_p))
        } else if !suffix.is_empty() {
            (inner, Some(&suffix[1..]))
        } else {
            (inner, None)
        }
    } else if let Some((ip_p, pre_p)) = trimmed.split_once('/') {
        (ip_p, Some(pre_p))
    } else {
        (trimmed, None)
    };

    if body.contains(':') {
        let ip_str = match body.find('%') {
            Some(zone) => &body[..zone],
            None => body,
        };
        let v6 = Ipv6Addr::from_str(ip_str).ok()?;
        let mut plen = match prefix_part {
            Some(p) => parse_u8_strict(p, 128)?,
            None => 128,
        };
        let raw = u128::from(v6);
        let hi = (raw >> 64) as u64;
        let lo = raw as u64;
        if hi == 0 && (lo >> 32) == 0xffff && plen >= 96 {
            plen -= 96;
            if plen == 0 {
                return None;
            }
            let mask = !0u32 << (32 - plen);
            return Some(ThroneFastCidr::from_v4((lo as u32) & mask, plen));
        }
        if plen == 0 {
            return None;
        }
        let mask = !0u128 << (128 - plen);
        Some(ThroneFastCidr::from_v6(raw & mask, plen))
    } else {
        let v4 = Ipv4Addr::from_str(body).ok()?;
        let plen = match prefix_part {
            Some(p) => parse_u8_strict(p, 32)?,
            None => 32,
        };
        if plen == 0 {
            return None;
        }
        let mask = !0u32 << (32 - plen);
        Some(ThroneFastCidr::from_v4(u32::from(v4) & mask, plen))
    }
}

/// Format CIDR in compact IP-list style:
/// - pure hex RFC 5952 for IPv6 (always 8 16-bit groups, longest >= 2 zero run as `::`)
/// - omits `/32` for IPv4 host and `/128` for IPv6 host
pub fn format_cidr_compact(cidr: &ThroneFastCidr, buf: &mut [u8]) -> Option<usize> {
    let mut cur = std::io::Cursor::new(buf);
    match cidr.family {
        4 => {
            if cidr.prefix_len > 32 {
                return None;
            }
            let v4 = Ipv4Addr::new(cidr.addr[0], cidr.addr[1], cidr.addr[2], cidr.addr[3]);
            if cidr.prefix_len == 32 {
                write!(cur, "{}", v4).ok()?;
            } else {
                write!(cur, "{}/{}", v4, cidr.prefix_len).ok()?;
            }
        }
        6 => {
            if cidr.prefix_len > 128 {
                return None;
            }
            let mut groups = [0u16; 8];
            for i in 0..8 {
                groups[i] = u16::from_be_bytes([cidr.addr[i * 2], cidr.addr[i * 2 + 1]]);
            }
            let mut best_start: Option<usize> = None;
            let mut best_len = 1usize;
            let mut i = 0usize;
            while i < 8 {
                if groups[i] != 0 {
                    i += 1;
                    continue;
                }
                let mut j = i;
                while j < 8 && groups[j] == 0 {
                    j += 1;
                }
                if j - i > best_len {
                    best_start = Some(i);
                    best_len = j - i;
                }
                i = j;
            }
            let mut wrote_any = false;
            let mut ends_colon = false;
            let mut idx = 0usize;
            while idx < 8 {
                if Some(idx) == best_start {
                    cur.write_all(b"::").ok()?;
                    wrote_any = true;
                    ends_colon = true;
                    idx += best_len;
                    continue;
                }
                if wrote_any && !ends_colon {
                    cur.write_all(b":").ok()?;
                }
                write!(cur, "{:x}", groups[idx]).ok()?;
                wrote_any = true;
                ends_colon = false;
                idx += 1;
            }
            if cidr.prefix_len < 128 {
                write!(cur, "/{}", cidr.prefix_len).ok()?;
            }
        }
        _ => return None,
    }
    Some(cur.position() as usize)
}

pub fn cover_range_v4(start: u32, end: u32, out: &mut Vec<ThroneFastCidr>) -> bool {
    if start > end {
        return false;
    }
    let mut cur = start;
    loop {
        let tz = if cur == 0 { 32 } else { cur.trailing_zeros() };
        let rem = (end - cur) as u64 + 1;
        let rem_bits = 63 - rem.leading_zeros();
        let step_bits = tz.min(rem_bits);
        let plen = (32 - step_bits) as u8;
        out.push(ThroneFastCidr::from_v4(cur, plen));
        if step_bits == 32 {
            break;
        }
        let step = 1u32 << step_bits;
        match cur.checked_add(step) {
            Some(next) if next <= end => cur = next,
            _ => break,
        }
    }
    true
}

pub fn cover_range_v6(start: u128, end: u128, out: &mut Vec<ThroneFastCidr>) -> bool {
    if start > end {
        return false;
    }
    let mut cur = start;
    loop {
        let tz = if cur == 0 { 128 } else { cur.trailing_zeros() };
        let rem = end - cur;
        let rem_bits = if rem == u128::MAX {
            128
        } else {
            127 - (rem + 1).leading_zeros()
        };
        let step_bits = tz.min(rem_bits);
        let plen = (128 - step_bits) as u8;
        out.push(ThroneFastCidr::from_v6(cur, plen));
        if step_bits == 128 {
            break;
        }
        let step = 1u128 << step_bits;
        match cur.checked_add(step) {
            Some(next) if next <= end => cur = next,
            _ => break,
        }
    }
    true
}

/// Parse an IPv4 or IPv6 address (no prefix).
/// Returns 0 on success (`out_family` set to 4 or 6, `out_addr16` populated in big-endian), -1 on error.
///
/// # Safety
/// Pointer parameters must be valid non-null buffers when `len > 0`.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_parse_ip(
    input: *const u8,
    len: usize,
    out_family: *mut u8,
    out_addr16: *mut u8,
) -> i32 {
    if input.is_null() || out_family.is_null() || out_addr16.is_null() || len == 0 {
        return -1;
    }
    let bytes = unsafe { slice::from_raw_parts(input, len) };
    let Ok(s) = str::from_utf8(bytes) else {
        return -1;
    };
    let trimmed = s.trim();
    let Ok(ip) = IpAddr::from_str(trimmed) else {
        return -1;
    };
    let out = unsafe { slice::from_raw_parts_mut(out_addr16, 16) };
    out.fill(0);
    match ip {
        IpAddr::V4(v4) => {
            unsafe { *out_family = 4 };
            out[..4].copy_from_slice(&v4.octets());
        }
        IpAddr::V6(v6) => {
            unsafe { *out_family = 6 };
            out.copy_from_slice(&v6.octets());
        }
    }
    0
}

/// Parse and mask a CIDR / bare IP string into canonical `ThroneFastCidr`.
/// Returns 0 on success, -1 on invalid input.
///
/// # Safety
/// `input` must point to `len` readable bytes; `out` must point to a writable `ThroneFastCidr`.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_parse_cidr(
    input: *const u8,
    len: usize,
    out: *mut ThroneFastCidr,
) -> i32 {
    if input.is_null() || out.is_null() || len == 0 {
        return -1;
    }
    let bytes = unsafe { slice::from_raw_parts(input, len) };
    let Ok(s) = str::from_utf8(bytes) else {
        return -1;
    };
    match parse_cidr(s) {
        Some(c) => {
            unsafe { *out = c };
            0
        }
        None => -1,
    }
}

/// Parse + mask + format a CIDR / bare IP into canonical ASCII form (`1.2.3.0/24` or RFC 5952 IPv6 CIDR).
/// Returns 0 on success, -1 on invalid input or insufficient output buffer (`out_cap`).
///
/// # Safety
/// Caller pointers must be valid for their respective byte lengths.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_normalize_cidr(
    input: *const u8,
    len: usize,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> i32 {
    if input.is_null() || out_buf.is_null() || out_len.is_null() || len == 0 || out_cap == 0 {
        return -1;
    }
    let bytes = unsafe { slice::from_raw_parts(input, len) };
    let Ok(s) = str::from_utf8(bytes) else {
        return -1;
    };
    let Some(cidr) = parse_cidr(s) else {
        return -1;
    };
    let out = unsafe { slice::from_raw_parts_mut(out_buf, out_cap) };
    match format_cidr(&cidr, out) {
        Some(n) => {
            unsafe { *out_len = n };
            0
        }
        None => -1,
    }
}

/// Decompose an inclusive IP range `[start_be16, end_be16]` into minimal aligned CIDR blocks.
/// Writes up to `max_out` entries and returns total count needed (negative on invalid input).
///
/// # Safety
/// `start_be16` and `end_be16` must point to 16 readable bytes; `out_cidrs` must be null (count query)
/// or writable for `max_out` elements.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_cover_range(
    start_be16: *const u8,
    end_be16: *const u8,
    family: u8,
    out_cidrs: *mut ThroneFastCidr,
    max_out: usize,
) -> i32 {
    if start_be16.is_null() || end_be16.is_null() {
        return -1;
    }
    let sb = unsafe { slice::from_raw_parts(start_be16, 16) };
    let eb = unsafe { slice::from_raw_parts(end_be16, 16) };
    let mut vec = Vec::with_capacity(64);
    let ok = match family {
        4 => {
            let s = u32::from_be_bytes([sb[0], sb[1], sb[2], sb[3]]);
            let e = u32::from_be_bytes([eb[0], eb[1], eb[2], eb[3]]);
            cover_range_v4(s, e, &mut vec)
        }
        6 => {
            let s = u128::from_be_bytes(sb.try_into().unwrap());
            let e = u128::from_be_bytes(eb.try_into().unwrap());
            cover_range_v6(s, e, &mut vec)
        }
        _ => false,
    };
    if !ok {
        return -1;
    }
    if !out_cidrs.is_null() && max_out > 0 {
        let copy_len = vec.len().min(max_out);
        let dst = unsafe { slice::from_raw_parts_mut(out_cidrs, copy_len) };
        dst.copy_from_slice(&vec[..copy_len]);
    }
    vec.len() as i32
}

/// Parse + mask + format a CIDR / bare IP in compact IP-list style:
/// omits `/32` and `/128`, unmaps `::ffff:w.x.y.z/96..128`, strips `%zone`, rejects `/0`.
///
/// # Safety
/// Caller pointers must be valid for their respective byte lengths.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_normalize_cidr_compact(
    input: *const u8,
    len: usize,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> i32 {
    if input.is_null() || out_buf.is_null() || out_len.is_null() || len == 0 || out_cap == 0 {
        return -1;
    }
    let bytes = unsafe { slice::from_raw_parts(input, len) };
    let Ok(s) = str::from_utf8(bytes) else {
        return -1;
    };
    let Some(cidr) = parse_cidr_compact(s) else {
        return -1;
    };
    let out = unsafe { slice::from_raw_parts_mut(out_buf, out_cap) };
    match format_cidr_compact(&cidr, out) {
        Some(n) => {
            unsafe { *out_len = n };
            0
        }
        None => -1,
    }
}

/// Format an already-parsed `ThroneFastCidr` in compact IP-list style (omitting `/32` and `/128`).
///
/// # Safety
/// `cidr` readable; `out_buf` writable for `out_cap` bytes; `out_len` writable.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_format_cidr_compact(
    cidr: *const ThroneFastCidr,
    out_buf: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> i32 {
    if cidr.is_null() || out_buf.is_null() || out_len.is_null() || out_cap == 0 {
        return -1;
    }
    let c = unsafe { &*cidr };
    let out = unsafe { slice::from_raw_parts_mut(out_buf, out_cap) };
    match format_cidr_compact(c, out) {
        Some(n) => {
            unsafe { *out_len = n };
            0
        }
        None => -1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_ipv4_and_ipv6() {
        let mut buf = [0u8; 64];
        let c = parse_cidr("192.168.1.42/24").unwrap();
        let n = format_cidr(&c, &mut buf).unwrap();
        assert_eq!(&buf[..n], b"192.168.1.0/24");

        let c6 = parse_cidr("2001:0db8:0000:0000:0000:ff00:0042:8329/32").unwrap();
        let n6 = format_cidr(&c6, &mut buf).unwrap();
        assert_eq!(&buf[..n6], b"2001:db8::/32");

        let c_host = parse_cidr("10.0.0.5").unwrap();
        assert_eq!(c_host.prefix_len, 32);
        assert!(parse_cidr("10.0.0.5/33").is_none());
        assert!(parse_cidr("10.0.0.5/+24").is_none());
        assert!(parse_cidr("10.0.0.5/24%eth0").is_none());
    }

    #[test]
    fn compact_iplist_cidr_parity() {
        let norm = |s: &str| -> Option<String> {
            let c = parse_cidr_compact(s)?;
            let mut buf = [0u8; 64];
            let n = format_cidr_compact(&c, &mut buf)?;
            Some(String::from_utf8_lossy(&buf[..n]).into_owned())
        };
        assert_eq!(norm("1.2.3.4/32").as_deref(), Some("1.2.3.4"));
        assert_eq!(norm("192.168.1.99/24").as_deref(), Some("192.168.1.0/24"));
        assert_eq!(
            norm("::ffff:192.168.1.99/120").as_deref(),
            Some("192.168.1.0/24")
        );
        assert_eq!(norm("::ffff:1.2.3.4").as_deref(), Some("1.2.3.4"));
        assert_eq!(norm("fe80::1%eth0").as_deref(), Some("fe80::1"));
        assert_eq!(norm("[2001:db8::1]/64").as_deref(), Some("2001:db8::/64"));
        assert_eq!(norm("::ffff:0:0/64").as_deref(), Some("::/64"));
        assert_eq!(norm("2001:0:0:1::1").as_deref(), Some("2001:0:0:1::1"));
        assert_eq!(norm("0.0.0.0/0"), None);
        assert_eq!(norm("::/0"), None);
        assert_eq!(norm("::ffff:0:0/96"), None);
    }

    #[test]
    fn range_cover_v4_exact_and_full() {
        let mut out = Vec::new();
        assert!(cover_range_v4(
            u32::from(Ipv4Addr::new(192, 168, 1, 5)),
            u32::from(Ipv4Addr::new(192, 168, 1, 10)),
            &mut out
        ));
        assert_eq!(out.len(), 4);
        assert_eq!(out[0].prefix_len, 32);
        assert_eq!(out[1].prefix_len, 31);
        assert_eq!(out[2].prefix_len, 31);
        assert_eq!(out[3].prefix_len, 32);

        out.clear();
        assert!(cover_range_v4(0, u32::MAX, &mut out));
        assert_eq!(out, vec![ThroneFastCidr::from_v4(0, 0)]);

        out.clear();
        assert!(!cover_range_v4(10, 5, &mut out));
    }

    #[test]
    fn range_cover_v6_full() {
        let mut out = Vec::new();
        assert!(cover_range_v6(0, u128::MAX, &mut out));
        assert_eq!(out, vec![ThroneFastCidr::from_v6(0, 0)]);
    }
}
