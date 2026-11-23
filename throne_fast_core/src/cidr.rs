//! CIDR and IP prefix manipulation utilities for fast routing.
//! Supports IPv4 and IPv6 subnet calculations, bitwise masks, and prefix matching.

use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ipv4Cidr {
    pub addr: u32,
    pub prefix_len: u8,
}

impl Ipv4Cidr {
    #[inline]
    pub const fn new(addr: u32, prefix_len: u8) -> Option<Self> {
        if prefix_len > 32 {
            None
        } else {
            let mask = Self::mask_for_prefix(prefix_len);
            Some(Self {
                addr: addr & mask,
                prefix_len,
            })
        }
    }

    #[inline]
    pub const fn mask_for_prefix(prefix_len: u8) -> u32 {
        if prefix_len == 0 {
            0
        } else if prefix_len >= 32 {
            !0u32
        } else {
            !((1u32 << (32 - prefix_len)) - 1)
        }
    }

    #[inline]
    pub const fn contains(&self, ip: u32) -> bool {
        let mask = Self::mask_for_prefix(self.prefix_len);
        (ip & mask) == self.addr
    }

    #[inline]
    pub fn broadcast(&self) -> u32 {
        self.addr | (!Self::mask_for_prefix(self.prefix_len))
    }

    #[inline]
    pub fn host_count(&self) -> u64 {
        if self.prefix_len >= 32 {
            1
        } else {
            1u64 << (32 - self.prefix_len)
        }
    }
}

impl FromStr for Ipv4Cidr {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.is_empty() || parts.len() > 2 {
            return Err("Invalid CIDR format: expected IP/prefix");
        }

        let ip_str = parts[0].trim();
        let ip = Ipv4Addr::from_str(ip_str).map_err(|_| "Invalid IPv4 address")?;
        let ip_u32 = u32::from(ip);

        let prefix_len = if parts.len() == 2 {
            parts[1].trim().parse::<u8>().map_err(|_| "Invalid prefix length")?
        } else {
            32
        };

        if prefix_len > 32 {
            return Err("Prefix length must be between 0 and 32");
        }

        Ipv4Cidr::new(ip_u32, prefix_len).ok_or("Failed to construct CIDR")
    }
}

impl fmt::Display for Ipv4Cidr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ip = Ipv4Addr::from(self.addr);
        write!(f, "{}/{}", ip, self.prefix_len)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ipv6Cidr {
    pub addr: u128,
    pub prefix_len: u8,
}

impl Ipv6Cidr {
    #[inline]
    pub const fn new(addr: u128, prefix_len: u8) -> Option<Self> {
        if prefix_len > 128 {
            None
        } else {
            let mask = Self::mask_for_prefix(prefix_len);
            Some(Self {
                addr: addr & mask,
                prefix_len,
            })
        }
    }

    #[inline]
    pub const fn mask_for_prefix(prefix_len: u8) -> u128 {
        if prefix_len == 0 {
            0
        } else if prefix_len >= 128 {
            !0u128
        } else {
            !((1u128 << (128 - prefix_len)) - 1)
        }
    }

    #[inline]
    pub const fn contains(&self, ip: u128) -> bool {
        let mask = Self::mask_for_prefix(self.prefix_len);
        (ip & mask) == self.addr
    }
}

impl FromStr for Ipv6Cidr {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.is_empty() || parts.len() > 2 {
            return Err("Invalid IPv6 CIDR format: expected IP/prefix");
        }

        let ip = Ipv6Addr::from_str(parts[0].trim()).map_err(|_| "Invalid IPv6 address")?;
        let ip_u128 = u128::from(ip);

        let prefix_len = if parts.len() == 2 {
            parts[1].trim().parse::<u8>().map_err(|_| "Invalid IPv6 prefix length")?
        } else {
            128
        };

        if prefix_len > 128 {
            return Err("IPv6 prefix length must be between 0 and 128");
        }

        Ipv6Cidr::new(ip_u128, prefix_len).ok_or("Failed to construct IPv6 CIDR")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipv4_cidr_containment() {
        let cidr = Ipv4Cidr::from_str("192.168.1.0/24").unwrap();
        let inside: u32 = Ipv4Addr::new(192, 168, 1, 42).into();
        let outside: u32 = Ipv4Addr::new(192, 168, 2, 1).into();

        assert!(cidr.contains(inside));
        assert!(!cidr.contains(outside));
        assert_eq!(cidr.host_count(), 256);
    }

    #[test]
    fn test_ipv6_cidr_containment() {
        let cidr = Ipv6Cidr::from_str("2001:db8::/32").unwrap();
        let inside = u128::from(Ipv6Addr::from_str("2001:db8:ffff::1").unwrap());
        let outside = u128::from(Ipv6Addr::from_str("2001:db9::1").unwrap());

        assert!(cidr.contains(inside));
        assert!(!cidr.contains(outside));
    }
}
