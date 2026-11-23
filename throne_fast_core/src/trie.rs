//! Zero-allocation Radix Prefix Trie for Longest Prefix Match (LPM) IP routing.
//! Supports both IPv4 (32-bit) and IPv6 (128-bit) routing table indexing.

use crate::cidr::Ipv4Cidr;

#[derive(Default)]
pub struct TrieNode {
    pub children: [Option<Box<TrieNode>>; 2],
    pub tag: Option<u16>,
}

/// IPv4 Radix Trie route lookup engine
pub struct Ipv4RadixTrie {
    root: TrieNode,
    total_prefixes: usize,
}

impl Default for Ipv4RadixTrie {
    fn default() -> Self {
        Self::new()
    }
}

impl Ipv4RadixTrie {
    pub fn new() -> Self {
        Self {
            root: TrieNode::default(),
            total_prefixes: 0,
        }
    }

    #[inline]
    pub fn insert(&mut self, ip: u32, prefix_len: u8, tag: u16) {
        if prefix_len > 32 {
            return;
        }
        let mut curr = &mut self.root;
        for bit in 0..prefix_len {
            let idx = ((ip >> (31 - bit)) & 1) as usize;
            if curr.children[idx].is_none() {
                curr.children[idx] = Some(Box::new(TrieNode::default()));
            }
            curr = curr.children[idx].as_deref_mut().unwrap();
        }
        if curr.tag.is_none() {
            self.total_prefixes += 1;
        }
        curr.tag = Some(tag);
    }

    #[inline]
    pub fn insert_cidr(&mut self, cidr: &Ipv4Cidr, tag: u16) {
        self.insert(cidr.addr, cidr.prefix_len, tag);
    }

    #[inline]
    pub fn lookup(&self, ip: u32) -> Option<u16> {
        let mut curr = &self.root;
        let mut best_tag = curr.tag;
        for bit in 0..32 {
            let idx = ((ip >> (31 - bit)) & 1) as usize;
            match curr.children[idx].as_deref() {
                Some(next) => {
                    curr = next;
                    if curr.tag.is_some() {
                        best_tag = curr.tag;
                    }
                }
                None => break,
            }
        }
        best_tag
    }

    #[inline]
    pub fn count(&self) -> usize {
        self.total_prefixes
    }

    pub fn clear(&mut self) {
        self.root = TrieNode::default();
        self.total_prefixes = 0;
    }
}

/// IPv6 Radix Trie for 128-bit prefix matching
pub struct Ipv6RadixTrie {
    root: TrieNode,
    total_prefixes: usize,
}

impl Default for Ipv6RadixTrie {
    fn default() -> Self {
        Self::new()
    }
}

impl Ipv6RadixTrie {
    pub fn new() -> Self {
        Self {
            root: TrieNode::default(),
            total_prefixes: 0,
        }
    }

    pub fn insert(&mut self, ip: u128, prefix_len: u8, tag: u16) {
        if prefix_len > 128 {
            return;
        }
        let mut curr = &mut self.root;
        for bit in 0..prefix_len {
            let idx = ((ip >> (127 - bit)) & 1) as usize;
            if curr.children[idx].is_none() {
                curr.children[idx] = Some(Box::new(TrieNode::default()));
            }
            curr = curr.children[idx].as_deref_mut().unwrap();
        }
        if curr.tag.is_none() {
            self.total_prefixes += 1;
        }
        curr.tag = Some(tag);
    }

    pub fn lookup(&self, ip: u128) -> Option<u16> {
        let mut curr = &self.root;
        let mut best_tag = curr.tag;
        for bit in 0..128 {
            let idx = ((ip >> (127 - bit)) & 1) as usize;
            match curr.children[idx].as_deref() {
                Some(next) => {
                    curr = next;
                    if curr.tag.is_some() {
                        best_tag = curr.tag;
                    }
                }
                None => break,
            }
        }
        best_tag
    }

    pub fn count(&self) -> usize {
        self.total_prefixes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_longest_prefix_match() {
        let mut trie = Ipv4RadixTrie::new();

        // 10.0.0.0/8 -> Tag 10
        trie.insert(0x0A000000, 8, 10);
        // 10.1.0.0/16 -> Tag 20
        trie.insert(0x0A010000, 16, 20);
        // 10.1.1.0/24 -> Tag 30
        trie.insert(0x0A010100, 24, 30);
        // 10.1.1.128/25 -> Tag 40
        trie.insert(0x0A010180, 25, 40);

        // Exact match on /25
        assert_eq!(trie.lookup(0x0A010185), Some(40));
        // Match falls back to /24
        assert_eq!(trie.lookup(0x0A010105), Some(30));
        // Match falls back to /16
        assert_eq!(trie.lookup(0x0A010205), Some(20));
        // Match falls back to /8
        assert_eq!(trie.lookup(0x0A020101), Some(10));
        // No match outside /8
        assert_eq!(trie.lookup(0x0B000001), None);
    }
}
