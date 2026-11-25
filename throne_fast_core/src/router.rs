use core::slice;
use std::collections::HashMap;

pub const DOMAIN_NO_MATCH: u32 = u32::MAX;

#[derive(Default)]
struct DomainNode {
    children: HashMap<String, usize>,
    exact_rule: u32,
    suffix_rule: u32,
}

impl DomainNode {
    fn new() -> Self {
        Self {
            children: HashMap::new(),
            exact_rule: DOMAIN_NO_MATCH,
            suffix_rule: DOMAIN_NO_MATCH,
        }
    }
}

pub struct ThroneFastDomainRouter {
    nodes: Vec<DomainNode>,
    keywords: Vec<(String, u32)>,
}

impl Default for ThroneFastDomainRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl ThroneFastDomainRouter {
    pub fn new() -> Self {
        Self {
            nodes: vec![DomainNode::new()],
            keywords: Vec::new(),
        }
    }

    pub fn insert_domain(&mut self, domain: &str, rule_id: u32, is_suffix: bool) -> bool {
        let trimmed = domain.trim().trim_matches('.');
        if trimmed.is_empty() {
            return false;
        }
        let lower = trimmed.to_ascii_lowercase();
        let mut cur = 0usize;
        for label in lower.rsplit('.') {
            if label.is_empty() {
                continue;
            }
            if let Some(&next) = self.nodes[cur].children.get(label) {
                cur = next;
            } else {
                let next = self.nodes.len();
                self.nodes.push(DomainNode::new());
                self.nodes[cur].children.insert(label.to_owned(), next);
                cur = next;
            }
        }
        if is_suffix {
            self.nodes[cur].suffix_rule = rule_id;
        } else {
            self.nodes[cur].exact_rule = rule_id;
        }
        true
    }

    pub fn insert_keyword(&mut self, keyword: &str, rule_id: u32) -> bool {
        let trimmed = keyword.trim();
        if trimmed.is_empty() {
            return false;
        }
        self.keywords.push((trimmed.to_ascii_lowercase(), rule_id));
        true
    }

    /// Lookup order: exact domain > longest matching domain suffix > first keyword match.
    pub fn match_host(&self, host: &str) -> u32 {
        let trimmed = host.trim().trim_matches('.');
        if trimmed.is_empty() || trimmed.len() > 255 {
            return DOMAIN_NO_MATCH;
        }

        // Stack lowercasing avoiding heap allocation.
        let mut buf = [0u8; 256];
        for (i, &b) in trimmed.as_bytes().iter().enumerate() {
            buf[i] = b.to_ascii_lowercase();
        }
        let lower = unsafe { core::str::from_utf8_unchecked(&buf[..trimmed.len()]) };

        let mut cur = 0usize;
        let mut best_suffix = DOMAIN_NO_MATCH;
        let mut exhausted = false;

        for label in lower.rsplit('.') {
            if label.is_empty() {
                continue;
            }
            match self.nodes[cur].children.get(label) {
                Some(&next) => {
                    cur = next;
                    if self.nodes[cur].suffix_rule != DOMAIN_NO_MATCH {
                        best_suffix = self.nodes[cur].suffix_rule;
                    }
                }
                None => {
                    exhausted = true;
                    break;
                }
            }
        }

        if !exhausted && self.nodes[cur].exact_rule != DOMAIN_NO_MATCH {
            return self.nodes[cur].exact_rule;
        }
        if best_suffix != DOMAIN_NO_MATCH {
            return best_suffix;
        }
        for (kw, rule_id) in &self.keywords {
            if lower.contains(kw.as_str()) {
                return *rule_id;
            }
        }
        DOMAIN_NO_MATCH
    }
}

#[no_mangle]
pub extern "C" fn throne_fast_domain_create() -> *mut ThroneFastDomainRouter {
    Box::into_raw(Box::new(ThroneFastDomainRouter::new()))
}

/// # Safety
/// `router` must have been allocated by `throne_fast_domain_create` or be null.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_domain_free(router: *mut ThroneFastDomainRouter) {
    if !router.is_null() {
        unsafe { drop(Box::from_raw(router)) };
    }
}

/// `kind`: 0 = exact, 1 = suffix, 2 = keyword.
///
/// # Safety
/// `router` valid; `pattern` readable for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_domain_insert(
    router: *mut ThroneFastDomainRouter,
    pattern: *const u8,
    len: usize,
    kind: u8,
    rule_id: u32,
) -> i32 {
    if router.is_null() || pattern.is_null() || len == 0 {
        return -1;
    }
    let bytes = unsafe { slice::from_raw_parts(pattern, len) };
    let Ok(s) = core::str::from_utf8(bytes) else {
        return -1;
    };
    let r = unsafe { &mut *router };
    let ok = match kind {
        0 => r.insert_domain(s, rule_id, false),
        1 => r.insert_domain(s, rule_id, true),
        2 => r.insert_keyword(s, rule_id),
        _ => false,
    };
    if ok {
        0
    } else {
        -1
    }
}

/// # Safety
/// `router` valid; `host` readable for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_domain_match(
    router: *const ThroneFastDomainRouter,
    host: *const u8,
    len: usize,
) -> u32 {
    if router.is_null() || host.is_null() || len == 0 {
        return DOMAIN_NO_MATCH;
    }
    let bytes = unsafe { slice::from_raw_parts(host, len) };
    let Ok(s) = core::str::from_utf8(bytes) else {
        return DOMAIN_NO_MATCH;
    };
    unsafe { &*router }.match_host(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_exact_suffix_and_keyword() {
        let mut r = ThroneFastDomainRouter::new();
        r.insert_domain("google.com", 1, true);
        r.insert_domain("mail.google.com", 2, false);
        r.insert_keyword("analytics", 3);

        assert_eq!(r.match_host("MAIL.GOOGLE.COM."), 2);
        assert_eq!(r.match_host("docs.google.com"), 1);
        assert_eq!(r.match_host("google.com"), 1);
        assert_eq!(r.match_host("notgoogle.com"), DOMAIN_NO_MATCH);
        assert_eq!(r.match_host("my-analytics.example.org"), 3);
    }
}
