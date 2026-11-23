//! throne_fast_core
//! High-performance acceleration module for Throne (empowered by sing-box).
//! Provides zero-allocation routing table lookups, stream tokenization, and hashing.

pub mod cidr;
pub mod dns;
pub mod geo;
pub mod hash;
pub mod pool;
pub mod protocol;
pub mod router;
pub mod stats;
pub mod trie;

use std::ffi::{c_char, c_int};
use std::slice;

pub use cidr::{Ipv4Cidr, Ipv6Cidr};
pub use dns::{DnsHeader, DnsMessageParser, DnsRecord};
pub use geo::GeoIpIndex;
pub use hash::{fnv1a_32, fnv1a_64, murmur3_32, xxhash64};
pub use pool::{BufferPool, PacketBuffer, RingBuffer};
pub use protocol::{SimpleProtobufReader, SimpleProtobufWriter, VarintCodec};
pub use router::{DomainPattern, DomainRouter, RouteAction, UnifiedRouter};
pub use stats::{EwmaRate, LatencyHistogram, TrafficCounters};
pub use trie::{Ipv4RadixTrie, Ipv6RadixTrie};

/// Version identifier for the acceleration module
pub const MODULE_VERSION: &str = "1.4.2-fast";

/// Fast RouteEngine wrapping the optimized Ipv4RadixTrie
pub struct RouteEngine {
    trie: Ipv4RadixTrie,
}

impl Default for RouteEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RouteEngine {
    pub fn new() -> Self {
        Self {
            trie: Ipv4RadixTrie::new(),
        }
    }

    #[inline]
    pub fn insert_prefix(&mut self, ip: u32, prefix_len: u8, tag: u16) {
        self.trie.insert(ip, prefix_len, tag);
    }

    #[inline]
    pub fn match_ip(&self, ip: u32) -> Option<u16> {
        self.trie.lookup(ip)
    }

    #[inline]
    pub fn count(&self) -> usize {
        self.trie.count()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.trie.clear();
    }
}

/// FNV-1a streaming 64-bit hash for fast deduplication
#[inline]
pub fn fast_hash_64(data: &[u8]) -> u64 {
    fnv1a_64(data)
}

// ---------------------------------------------------------------------------
// C-ABI Exported Interfaces (Preserves complete backwards compatibility)
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn throne_fast_version() -> *const c_char {
    static VER_BYTES: &[u8] = b"1.4.2-fast\0";
    VER_BYTES.as_ptr() as *const c_char
}

#[no_mangle]
pub extern "C" fn throne_fast_hash(data_ptr: *const u8, len: usize) -> u64 {
    if data_ptr.is_null() || len == 0 {
        return 0;
    }
    let slice = unsafe { slice::from_raw_parts(data_ptr, len) };
    fast_hash_64(slice)
}

#[no_mangle]
pub extern "C" fn throne_fast_route_create() -> *mut RouteEngine {
    Box::into_raw(Box::new(RouteEngine::new()))
}

#[no_mangle]
pub extern "C" fn throne_fast_route_free(ptr: *mut RouteEngine) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

#[no_mangle]
pub extern "C" fn throne_fast_route_insert_prefix(
    engine: *mut RouteEngine,
    ip_be: u32,
    prefix_len: u8,
    tag: u16,
) -> c_int {
    if engine.is_null() || prefix_len > 32 {
        return -1;
    }
    let engine = unsafe { &mut *engine };
    engine.insert_prefix(ip_be, prefix_len, tag);
    0
}

#[no_mangle]
pub extern "C" fn throne_fast_route_match_ip(engine: *const RouteEngine, ip_be: u32) -> c_int {
    if engine.is_null() {
        return -1;
    }
    let engine = unsafe { &*engine };
    match engine.match_ip(ip_be) {
        Some(tag) => tag as c_int,
        None => -1,
    }
}

#[no_mangle]
pub extern "C" fn throne_fast_route_clear(engine: *mut RouteEngine) -> c_int {
    if engine.is_null() {
        return -1;
    }
    let engine = unsafe { &mut *engine };
    engine.clear();
    0
}

#[no_mangle]
pub extern "C" fn throne_fast_route_add(
    engine: *mut RouteEngine,
    ip: u32,
    prefix_len: u8,
    tag: u16,
) -> c_int {
    throne_fast_route_insert_prefix(engine, ip, prefix_len, tag)
}

#[no_mangle]
pub extern "C" fn throne_fast_route_match(engine: *const RouteEngine, ip: u32) -> c_int {
    throne_fast_route_match_ip(engine, ip)
}

#[no_mangle]
pub extern "C" fn throne_fast_benchmark(iterations: u32) -> f64 {
    let mut engine = RouteEngine::new();
    // Populate sample routing table
    engine.insert_prefix(0x0a000000, 8, 1);   // 10.0.0.0/8 -> Direct
    engine.insert_prefix(0xac100000, 12, 1);  // 172.16.0.0/12 -> Direct
    engine.insert_prefix(0xc0a80000, 16, 1);  // 192.168.0.0/16 -> Direct
    engine.insert_prefix(0x7f000000, 8, 1);   // 127.0.0.0/8 -> Direct

    let start = std::time::Instant::now();
    let mut sum: u64 = 0;
    for i in 0..iterations {
        let test_ip = 0x0a000000 | (i & 0x00ffffff);
        if let Some(tag) = engine.match_ip(test_ip) {
            sum += tag as u64;
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    if sum == 0 {
        return -1.0;
    }
    elapsed
}
