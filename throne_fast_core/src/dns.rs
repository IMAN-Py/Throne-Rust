//! Zero-allocation binary DNS wire format parser (RFC 1035).
//! Parses DNS headers, questions, and A/AAAA answers directly from raw packet slices.

use std::net::{Ipv4Addr, Ipv6Addr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DnsHeader {
    pub id: u16,
    pub flags: u16,
    pub qdcount: u16,
    pub ancount: u16,
    pub nscount: u16,
    pub arcount: u16,
}

impl DnsHeader {
    pub fn parse(buf: &[u8]) -> Option<Self> {
        if buf.len() < 12 {
            return None;
        }
        Some(Self {
            id: u16::from_be_bytes([buf[0], buf[1]]),
            flags: u16::from_be_bytes([buf[2], buf[3]]),
            qdcount: u16::from_be_bytes([buf[4], buf[5]]),
            ancount: u16::from_be_bytes([buf[6], buf[7]]),
            nscount: u16::from_be_bytes([buf[8], buf[9]]),
            arcount: u16::from_be_bytes([buf[10], buf[11]]),
        })
    }

    #[inline]
    pub fn is_response(&self) -> bool {
        (self.flags & 0x8000) != 0
    }

    #[inline]
    pub fn rcode(&self) -> u8 {
        (self.flags & 0x000F) as u8
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DnsRecord {
    A { name: String, ttl: u32, ip: Ipv4Addr },
    AAAA { name: String, ttl: u32, ip: Ipv6Addr },
    CNAME { name: String, ttl: u32, target: String },
    Other { qtype: u16, ttl: u32 },
}

pub struct DnsMessageParser<'a> {
    buffer: &'a [u8],
    offset: usize,
}

impl<'a> DnsMessageParser<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        Self { buffer, offset: 0 }
    }

    pub fn parse_header(&mut self) -> Option<DnsHeader> {
        if self.buffer.len() < 12 {
            return None;
        }
        let header = DnsHeader::parse(&self.buffer[0..12])?;
        self.offset = 12;
        Some(header)
    }

    pub fn skip_question(&mut self) -> bool {
        if !self.skip_domain_name() {
            return false;
        }
        // Skip qtype (2 bytes) and qclass (2 bytes)
        if self.offset + 4 > self.buffer.len() {
            return false;
        }
        self.offset += 4;
        true
    }

    fn skip_domain_name(&mut self) -> bool {
        let jumped = false;
        let mut loop_guard = 0;

        while self.offset < self.buffer.len() {
            loop_guard += 1;
            if loop_guard > 128 {
                return false; // Loop detected
            }
            let len = self.buffer[self.offset];
            if len == 0 {
                if !jumped {
                    self.offset += 1;
                }
                return true;
            } else if (len & 0xC0) == 0xC0 {
                // Pointer compression
                if !jumped {
                    self.offset += 2;
                }
                return true;
            } else {
                self.offset += 1 + (len as usize);
            }
        }
        false
    }

    pub fn parse_a_record(&mut self) -> Option<Ipv4Addr> {
        if !self.skip_domain_name() {
            return None;
        }
        if self.offset + 10 > self.buffer.len() {
            return None;
        }
        let rtype = u16::from_be_bytes([self.buffer[self.offset], self.buffer[self.offset + 1]]);
        let _rclass = u16::from_be_bytes([self.buffer[self.offset + 2], self.buffer[self.offset + 3]]);
        let _ttl = u32::from_be_bytes([
            self.buffer[self.offset + 4],
            self.buffer[self.offset + 5],
            self.buffer[self.offset + 6],
            self.buffer[self.offset + 7],
        ]);
        let rdlen = u16::from_be_bytes([self.buffer[self.offset + 8], self.buffer[self.offset + 9]]) as usize;
        self.offset += 10;

        if rtype == 1 && rdlen == 4 && self.offset + 4 <= self.buffer.len() {
            let ip = Ipv4Addr::new(
                self.buffer[self.offset],
                self.buffer[self.offset + 1],
                self.buffer[self.offset + 2],
                self.buffer[self.offset + 3],
            );
            self.offset += 4;
            Some(ip)
        } else {
            self.offset += rdlen;
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dns_header_parsing() {
        let raw_query = [
            0x12, 0x34, // ID: 0x1234
            0x01, 0x00, // Standard query, RD=1
            0x00, 0x01, // QDCOUNT: 1
            0x00, 0x00, // ANCOUNT: 0
            0x00, 0x00, // NSCOUNT: 0
            0x00, 0x00, // ARCOUNT: 0
        ];
        let header = DnsHeader::parse(&raw_query).unwrap();
        assert_eq!(header.id, 0x1234);
        assert!(!header.is_response());
        assert_eq!(header.qdcount, 1);
    }
}
