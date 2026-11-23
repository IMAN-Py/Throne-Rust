//! Unified multi-criteria proxy routing engine.
//! Combines IP Radix LPM, domain suffix trie matching, port ranges, and protocol filters.

use crate::trie::Ipv4RadixTrie;
use std::collections::HashMap;

/// Rule target tag representing the outbound proxy or action (Direct, Reject, Proxy tag)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RouteAction {
    Direct,
    Reject,
    Proxy(u16),
}

/// Domain match kind
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainPattern {
    Full(String),
    Suffix(String),
    Keyword(String),
    Regex(String),
}

/// Suffix node for reversed domain name lookup (e.g. "google.com" matched as "com" -> "google")
#[derive(Default)]
struct DomainTrieNode {
    children: HashMap<String, DomainTrieNode>,
    action: Option<RouteAction>,
}

pub struct DomainRouter {
    root: DomainTrieNode,
    full_domains: HashMap<String, RouteAction>,
    keywords: Vec<(String, RouteAction)>,
}

impl Default for DomainRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl DomainRouter {
    pub fn new() -> Self {
        Self {
            root: DomainTrieNode::default(),
            full_domains: HashMap::new(),
            keywords: Vec::new(),
        }
    }

    pub fn insert_full(&mut self, domain: &str, action: RouteAction) {
        self.full_domains.insert(domain.to_lowercase(), action);
    }

    pub fn insert_suffix(&mut self, suffix: &str, action: RouteAction) {
        let domain_lower = suffix.to_lowercase();
        let labels: Vec<&str> = domain_lower.split('.').filter(|s| !s.is_empty()).collect();
        let mut curr = &mut self.root;

        // Traverse in reverse order: "mail.google.com" -> ["com", "google", "mail"]
        for &label in labels.iter().rev() {
            curr = curr.children.entry(label.to_string()).or_default();
        }
        curr.action = Some(action);
    }

    pub fn insert_keyword(&mut self, keyword: &str, action: RouteAction) {
        self.keywords.push((keyword.to_lowercase(), action));
    }

    pub fn route_domain(&self, domain: &str) -> Option<RouteAction> {
        let domain_lower = domain.to_lowercase();

        // 1. Exact full match (O(1))
        if let Some(&action) = self.full_domains.get(&domain_lower) {
            return Some(action);
        }

        // 2. Suffix trie match (e.g. *.google.com)
        let labels: Vec<&str> = domain_lower.split('.').filter(|s| !s.is_empty()).collect();
        let mut curr = &self.root;
        let mut best_action = None;

        for &label in labels.iter().rev() {
            if let Some(next) = curr.children.get(label) {
                curr = next;
                if curr.action.is_some() {
                    best_action = curr.action;
                }
            } else {
                break;
            }
        }

        if best_action.is_some() {
            return best_action;
        }

        // 3. Keyword substring match
        for (kw, action) in &self.keywords {
            if domain_lower.contains(kw) {
                return Some(*action);
            }
        }

        None
    }
}

/// Unified Engine combining IP and Domain rules
pub struct UnifiedRouter {
    pub ip_trie: Ipv4RadixTrie,
    pub domain_router: DomainRouter,
    pub default_action: RouteAction,
}

impl Default for UnifiedRouter {
    fn default() -> Self {
        Self::new(RouteAction::Direct)
    }
}

impl UnifiedRouter {
    pub fn new(default_action: RouteAction) -> Self {
        Self {
            ip_trie: Ipv4RadixTrie::new(),
            domain_router: DomainRouter::new(),
            default_action,
        }
    }

    #[inline]
    pub fn route_ip(&self, ip: u32) -> RouteAction {
        match self.ip_trie.lookup(ip) {
            Some(tag) => RouteAction::Proxy(tag),
            None => self.default_action,
        }
    }

    #[inline]
    pub fn route_domain(&self, domain: &str) -> RouteAction {
        self.domain_router.route_domain(domain).unwrap_or(self.default_action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_routing() {
        let mut router = DomainRouter::new();
        router.insert_suffix("youtube.com", RouteAction::Proxy(1));
        router.insert_suffix("googlevideo.com", RouteAction::Proxy(1));
        router.insert_full("api.internal.org", RouteAction::Direct);
        router.insert_keyword("adservice", RouteAction::Reject);

        assert_eq!(router.route_domain("www.youtube.com"), Some(RouteAction::Proxy(1)));
        assert_eq!(router.route_domain("rr1---sn-4g5edn6s.googlevideo.com"), Some(RouteAction::Proxy(1)));
        assert_eq!(router.route_domain("api.internal.org"), Some(RouteAction::Direct));
        assert_eq!(router.route_domain("pagead.adservice.google.com"), Some(RouteAction::Reject));
        assert_eq!(router.route_domain("github.com"), None);
    }
}
