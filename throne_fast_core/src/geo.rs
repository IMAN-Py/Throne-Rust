//! Binary GeoIP and GeoSite database indexer for sub-microsecond country routing.

/// Compact 8-byte entry for IPv4 GeoIP range
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
pub struct GeoIpRange {
    pub start_ip: u32,
    pub end_ip: u32,
    pub country_code: [u8; 2],
}

pub struct GeoIpIndex {
    ranges: Vec<GeoIpRange>,
}

impl Default for GeoIpIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl GeoIpIndex {
    pub fn new() -> Self {
        Self { ranges: Vec::new() }
    }

    pub fn insert_range(&mut self, start_ip: u32, end_ip: u32, cc: [u8; 2]) {
        assert!(start_ip <= end_ip);
        self.ranges.push(GeoIpRange {
            start_ip,
            end_ip,
            country_code: cc,
        });
    }

    pub fn sort_and_dedup(&mut self) {
        self.ranges.sort_by_key(|r| r.start_ip);
    }

    /// Binary search for O(log N) country code lookup
    pub fn lookup(&self, ip: u32) -> Option<[u8; 2]> {
        if self.ranges.is_empty() {
            return None;
        }

        let mut low = 0;
        let mut high = self.ranges.len();

        while low < high {
            let mid = low + (high - low) / 2;
            let range = &self.ranges[mid];

            if ip < range.start_ip {
                high = mid;
            } else if ip > range.end_ip {
                low = mid + 1;
            } else {
                return Some(range.country_code);
            }
        }

        None
    }

    pub fn lookup_str(&self, ip: u32) -> Option<String> {
        self.lookup(ip).map(|cc| {
            format!("{}{}", cc[0] as char, cc[1] as char)
        })
    }

    pub fn len(&self) -> usize {
        self.ranges.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_geoip_binary_search() {
        let mut idx = GeoIpIndex::new();
        let us_start: u32 = Ipv4Addr::new(8, 0, 0, 0).into();
        let us_end: u32 = Ipv4Addr::new(8, 255, 255, 255).into();
        idx.insert_range(us_start, us_end, *b"US");

        let de_start: u32 = Ipv4Addr::new(138, 201, 0, 0).into();
        let de_end: u32 = Ipv4Addr::new(138, 201, 255, 255).into();
        idx.insert_range(de_start, de_end, *b"DE");

        idx.sort_and_dedup();

        let test_us: u32 = Ipv4Addr::new(8, 8, 8, 8).into();
        assert_eq!(idx.lookup_str(test_us), Some("US".to_string()));

        let test_de: u32 = Ipv4Addr::new(138, 201, 50, 1).into();
        assert_eq!(idx.lookup_str(test_de), Some("DE".to_string()));

        let test_unknown: u32 = Ipv4Addr::new(1, 1, 1, 1).into();
        assert_eq!(idx.lookup(test_unknown), None);
    }
}
