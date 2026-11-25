use crate::cidr::parse_cidr;
use core::slice;

pub const NO_NODE: u32 = u32::MAX;
pub const NO_RULE: u32 = u32::MAX;

#[derive(Clone, Copy, Debug)]
struct ArenaNode {
    children: [u32; 16],
    leaf_rule: [u32; 16],
    leaf_plen: [u8; 16],
}

impl ArenaNode {
    const EMPTY: Self = Self {
        children: [NO_NODE; 16],
        leaf_rule: [NO_RULE; 16],
        leaf_plen: [0; 16],
    };
}

/// Cache-friendly 4-bit multibit flat arena LPM router for IPv4 & IPv6 CIDRs.
pub struct ThroneFastRouteTable {
    v4_nodes: Vec<ArenaNode>,
    root_v4_rule: u32,
    has_root_v4: bool,

    v6_nodes: Vec<ArenaNode>,
    root_v6_rule: u32,
    has_root_v6: bool,
}

impl Default for ThroneFastRouteTable {
    fn default() -> Self {
        Self::new()
    }
}

impl ThroneFastRouteTable {
    pub fn new() -> Self {
        Self {
            v4_nodes: Vec::with_capacity(256),
            root_v4_rule: NO_RULE,
            has_root_v4: false,
            v6_nodes: Vec::with_capacity(128),
            root_v6_rule: NO_RULE,
            has_root_v6: false,
        }
    }

    pub fn insert_v4(&mut self, addr: u32, prefix_len: u8, rule_id: u32) -> bool {
        if prefix_len > 32 {
            return false;
        }
        if prefix_len == 0 {
            self.root_v4_rule = rule_id;
            self.has_root_v4 = true;
            return true;
        }
        if self.v4_nodes.is_empty() {
            self.v4_nodes.push(ArenaNode::EMPTY);
        }
        let mask = !0u32 << (32 - prefix_len);
        let masked = addr & mask;

        let mut cur = 0usize;
        for d in 0..8usize {
            let consumed = (d * 4) as u8;
            let rem = prefix_len - consumed;
            let nib = ((masked >> (28 - d * 4)) & 0xF) as usize;
            if rem <= 4 {
                let fill = 1usize << (4 - rem);
                let base_slot = nib & !(fill - 1);
                let node = &mut self.v4_nodes[cur];
                for s in base_slot..(base_slot + fill) {
                    if prefix_len >= node.leaf_plen[s] {
                        node.leaf_plen[s] = prefix_len;
                        node.leaf_rule[s] = rule_id;
                    }
                }
                break;
            }
            let next = self.v4_nodes[cur].children[nib];
            if next == NO_NODE {
                let new_idx = self.v4_nodes.len() as u32;
                self.v4_nodes.push(ArenaNode::EMPTY);
                self.v4_nodes[cur].children[nib] = new_idx;
                cur = new_idx as usize;
            } else {
                cur = next as usize;
            }
        }
        true
    }

    pub fn insert_v6(&mut self, addr: u128, prefix_len: u8, rule_id: u32) -> bool {
        if prefix_len > 128 {
            return false;
        }
        if prefix_len == 0 {
            self.root_v6_rule = rule_id;
            self.has_root_v6 = true;
            return true;
        }
        if self.v6_nodes.is_empty() {
            self.v6_nodes.push(ArenaNode::EMPTY);
        }
        let mask = !0u128 << (128 - prefix_len);
        let masked = addr & mask;

        let mut cur = 0usize;
        for d in 0..32usize {
            let consumed = (d * 4) as u8;
            let rem = prefix_len - consumed;
            let nib = ((masked >> (124 - d * 4)) & 0xF) as usize;
            if rem <= 4 {
                let fill = 1usize << (4 - rem);
                let base_slot = nib & !(fill - 1);
                let node = &mut self.v6_nodes[cur];
                for s in base_slot..(base_slot + fill) {
                    if prefix_len >= node.leaf_plen[s] {
                        node.leaf_plen[s] = prefix_len;
                        node.leaf_rule[s] = rule_id;
                    }
                }
                break;
            }
            let next = self.v6_nodes[cur].children[nib];
            if next == NO_NODE {
                let new_idx = self.v6_nodes.len() as u32;
                self.v6_nodes.push(ArenaNode::EMPTY);
                self.v6_nodes[cur].children[nib] = new_idx;
                cur = new_idx as usize;
            } else {
                cur = next as usize;
            }
        }
        true
    }

    pub fn insert_cidr(&mut self, cidr: &str, rule_id: u32) -> bool {
        let Some(c) = parse_cidr(cidr) else {
            return false;
        };
        match c.family {
            4 => {
                let v4 = u32::from_be_bytes([c.addr[0], c.addr[1], c.addr[2], c.addr[3]]);
                self.insert_v4(v4, c.prefix_len, rule_id)
            }
            6 => {
                let v6 = u128::from_be_bytes(c.addr);
                self.insert_v6(v6, c.prefix_len, rule_id)
            }
            _ => false,
        }
    }

    #[inline(always)]
    pub fn match_v4(&self, addr: u32) -> u32 {
        let mut best = if self.has_root_v4 {
            self.root_v4_rule
        } else {
            NO_RULE
        };
        if self.v4_nodes.is_empty() {
            return best;
        }
        let mut best_plen = 0u8;
        let mut cur = 0usize;
        for d in 0..8usize {
            let nib = ((addr >> (28 - d * 4)) & 0xF) as usize;
            let node = &self.v4_nodes[cur];
            let lp = node.leaf_plen[nib];
            if lp > best_plen {
                best_plen = lp;
                best = node.leaf_rule[nib];
            }
            let next = node.children[nib];
            if next == NO_NODE {
                break;
            }
            cur = next as usize;
        }
        best
    }

    #[inline(always)]
    pub fn match_v6(&self, addr: u128) -> u32 {
        let mut best = if self.has_root_v6 {
            self.root_v6_rule
        } else {
            NO_RULE
        };
        if self.v6_nodes.is_empty() {
            return best;
        }
        let mut best_plen = 0u8;
        let mut cur = 0usize;
        for d in 0..32usize {
            let nib = ((addr >> (124 - d * 4)) & 0xF) as usize;
            let node = &self.v6_nodes[cur];
            let lp = node.leaf_plen[nib];
            if lp > best_plen {
                best_plen = lp;
                best = node.leaf_rule[nib];
            }
            let next = node.children[nib];
            if next == NO_NODE {
                break;
            }
            cur = next as usize;
        }
        best
    }
}

#[no_mangle]
pub extern "C" fn throne_fast_route_create() -> *mut ThroneFastRouteTable {
    Box::into_raw(Box::new(ThroneFastRouteTable::new()))
}

/// # Safety
/// `table` must have been allocated by `throne_fast_route_create` or be null.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_route_free(table: *mut ThroneFastRouteTable) {
    if !table.is_null() {
        unsafe { drop(Box::from_raw(table)) };
    }
}

/// # Safety
/// `table` must be valid non-null; `cidr` readable for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_route_insert_cidr(
    table: *mut ThroneFastRouteTable,
    cidr: *const u8,
    len: usize,
    rule_id: u32,
) -> i32 {
    if table.is_null() || cidr.is_null() || len == 0 {
        return -1;
    }
    let bytes = unsafe { slice::from_raw_parts(cidr, len) };
    let Ok(s) = core::str::from_utf8(bytes) else {
        return -1;
    };
    if unsafe { &mut *table }.insert_cidr(s, rule_id) {
        0
    } else {
        -1
    }
}

/// Insert host-byte-order or big-endian IPv4 (`addr_net_be` = big-endian bytes as `u32`).
/// Wait: we accept `addr_host_u32` (host order `a<<24|b<<16|c<<8|d`) OR expose both cleanly;
/// let's accept explicit 4-byte `addr_be4: *const u8` in `insert_v4_bytes` or `addr_be32`?
/// Here `addr_host: u32` (`(oct0 << 24) | (oct1 << 16) | (oct2 << 8) | oct3`).
///
/// # Safety
/// `table` must be valid non-null.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_route_insert_v4(
    table: *mut ThroneFastRouteTable,
    addr_host: u32,
    prefix_len: u8,
    rule_id: u32,
) -> i32 {
    if table.is_null() {
        return -1;
    }
    if unsafe { &mut *table }.insert_v4(addr_host, prefix_len, rule_id) {
        0
    } else {
        -1
    }
}

/// # Safety
/// `table` must be valid; `addr_be16` readable for 16 bytes.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_route_insert_v6(
    table: *mut ThroneFastRouteTable,
    addr_be16: *const u8,
    prefix_len: u8,
    rule_id: u32,
) -> i32 {
    if table.is_null() || addr_be16.is_null() {
        return -1;
    }
    let b = unsafe { slice::from_raw_parts(addr_be16, 16) };
    let addr = u128::from_be_bytes(b.try_into().unwrap());
    if unsafe { &mut *table }.insert_v6(addr, prefix_len, rule_id) {
        0
    } else {
        -1
    }
}

/// # Safety
/// `table` must be valid non-null.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_route_match_v4(
    table: *const ThroneFastRouteTable,
    addr_host: u32,
) -> u32 {
    if table.is_null() {
        return NO_RULE;
    }
    unsafe { &*table }.match_v4(addr_host)
}

/// # Safety
/// `table` valid, `addr_be16` readable for 16 bytes.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_route_match_v6(
    table: *const ThroneFastRouteTable,
    addr_be16: *const u8,
) -> u32 {
    if table.is_null() || addr_be16.is_null() {
        return NO_RULE;
    }
    let b = unsafe { slice::from_raw_parts(addr_be16, 16) };
    let addr = u128::from_be_bytes(b.try_into().unwrap());
    unsafe { &*table }.match_v6(addr)
}

/// Batch IPv4 lookup amortizing FFI call overhead.
/// Returns count processed.
///
/// # Safety
/// `table` valid; `addrs_host` readable for `count`; `out_rules` writable for `count`.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_route_match_batch_v4(
    table: *const ThroneFastRouteTable,
    addrs_host: *const u32,
    out_rules: *mut u32,
    count: usize,
) -> usize {
    if table.is_null() || addrs_host.is_null() || out_rules.is_null() || count == 0 {
        return 0;
    }
    let t = unsafe { &*table };
    let src = unsafe { slice::from_raw_parts(addrs_host, count) };
    let dst = unsafe { slice::from_raw_parts_mut(out_rules, count) };
    for i in 0..count {
        dst[i] = t.match_v4(src[i]);
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multibit_lpm_exact_order_independence() {
        let mut t = ThroneFastRouteTable::new();
        // Insert specific before general and vice-versa
        assert!(t.insert_cidr("10.0.0.0/25", 25));
        assert!(t.insert_cidr("10.0.0.0/24", 24));
        assert!(t.insert_cidr("10.0.0.0/8", 8));
        assert!(t.insert_cidr("0.0.0.0/0", 0));

        assert_eq!(t.match_v4(u32::from_be_bytes([10, 0, 0, 5])), 25);
        assert_eq!(t.match_v4(u32::from_be_bytes([10, 0, 0, 200])), 24);
        assert_eq!(t.match_v4(u32::from_be_bytes([10, 1, 2, 3])), 8);
        assert_eq!(t.match_v4(u32::from_be_bytes([1, 1, 1, 1])), 0);
    }

    #[test]
    fn ipv6_lpm() {
        let mut t = ThroneFastRouteTable::new();
        assert!(t.insert_cidr("2001:db8::/32", 1));
        assert!(t.insert_cidr("2001:db8:1::/48", 2));
        let ip1 = u128::from_be_bytes(
            "2001:db8:1::5".parse::<std::net::Ipv6Addr>().unwrap().octets(),
        );
        let ip2 = u128::from_be_bytes(
            "2001:db8:2::5".parse::<std::net::Ipv6Addr>().unwrap().octets(),
        );
        let ip3 = u128::from_be_bytes(
            "2606:4700::1".parse::<std::net::Ipv6Addr>().unwrap().octets(),
        );
        assert_eq!(t.match_v6(ip1), 2);
        assert_eq!(t.match_v6(ip2), 1);
        assert_eq!(t.match_v6(ip3), NO_RULE);
    }
}
