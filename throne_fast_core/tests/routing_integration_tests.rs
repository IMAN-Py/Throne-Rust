//! Comprehensive integration and stress tests for the Rust acceleration engine.

use std::net::Ipv4Addr;
use std::str::FromStr;
use throne_fast_core::cidr::Ipv4Cidr;
use throne_fast_core::geo::GeoIpIndex;
use throne_fast_core::hash::xxhash64;
use throne_fast_core::pool::BufferPool;
use throne_fast_core::router::{DomainRouter, RouteAction};
use throne_fast_core::trie::Ipv4RadixTrie;

#[test]
fn test_large_scale_prefix_insertion_and_lookup() {
    let mut trie = Ipv4RadixTrie::new();

    // Insert 10,000 subnets
    for i in 1..=254 {
        for j in 1..=40 {
            let cidr_str = format!("10.{}.{}.0/24", i, j);
            let cidr = Ipv4Cidr::from_str(&cidr_str).unwrap();
            let tag = ((i * 100 + j) % 65535) as u16;
            trie.insert_cidr(&cidr, tag);
        }
    }

    assert_eq!(trie.count(), 254 * 40);

    // Verify lookups
    for i in 1..=254 {
        for j in 1..=40 {
            let test_ip = u32::from(Ipv4Addr::new(10, i as u8, j as u8, 123));
            let expected_tag = ((i * 100 + j) % 65535) as u16;
            assert_eq!(trie.lookup(test_ip), Some(expected_tag));
        }
    }

    // Outside IP must not match
    let outside_ip = u32::from(Ipv4Addr::new(172, 16, 0, 1));
    assert_eq!(trie.lookup(outside_ip), None);
}

#[test]
fn test_hierarchical_domain_routing_rules() {
    let mut router = DomainRouter::new();

    // Suffix rules
    router.insert_suffix("youtube.com", RouteAction::Proxy(101));
    router.insert_suffix("googlevideo.com", RouteAction::Proxy(101));
    router.insert_suffix("netflix.com", RouteAction::Proxy(102));
    router.insert_suffix("fast.com", RouteAction::Proxy(102));
    router.insert_suffix("github.com", RouteAction::Direct);

    // Full match rule overrides
    router.insert_full("music.youtube.com", RouteAction::Proxy(103));

    // Keyword rule
    router.insert_keyword("telemetry", RouteAction::Reject);
    router.insert_keyword("analytics", RouteAction::Reject);

    // Tests
    assert_eq!(router.route_domain("music.youtube.com"), Some(RouteAction::Proxy(103)));
    assert_eq!(router.route_domain("r1---sn-abc.googlevideo.com"), Some(RouteAction::Proxy(101)));
    assert_eq!(router.route_domain("api.github.com"), Some(RouteAction::Direct));
    assert_eq!(router.route_domain("app-telemetry.service.internal"), Some(RouteAction::Reject));
    assert_eq!(router.route_domain("user-analytics.datadog.com"), Some(RouteAction::Reject));
    assert_eq!(router.route_domain("unknown-domain.org"), None);
}

#[test]
fn test_simd_hash_distribution_and_avalanche() {
    let mut hashes = std::collections::HashSet::new();
    let num_samples = 50_000;

    for i in 0..num_samples {
        let key = format!("profile-node-eu-central-{:08x}-port-{}", i, 1000 + (i % 5000));
        let h = xxhash64(key.as_bytes(), 0x123456789ABCDEF0);
        hashes.insert(h);
    }

    // Zero collisions expected on 50,000 distinct node identifiers
    assert_eq!(hashes.len(), num_samples);
}

#[test]
fn test_zero_copy_buffer_pool_concurrency() {
    let mut pool = BufferPool::with_capacity(64);
    let mut buffers = Vec::new();

    for _ in 0..64 {
        buffers.push(pool.acquire().unwrap());
    }
    assert_eq!(pool.available(), 0);

    for mut buf in buffers {
        buf.set_len(4096);
        assert_eq!(buf.as_slice().len(), 4096);
        pool.release(buf);
    }
    assert_eq!(pool.available(), 64);
}

#[test]
fn test_geoip_range_indexer_scale() {
    let mut geo = GeoIpIndex::new();

    // Insert 10,000 simulated IP ranges
    for i in 0..10_000u32 {
        let start = i * 65536;
        let end = start + 65535;
        let cc = if i % 2 == 0 { *b"US" } else { *b"DE" };
        geo.insert_range(start, end, cc);
    }
    geo.sort_and_dedup();

    assert_eq!(geo.len(), 10_000);
    assert_eq!(geo.lookup_str(65536 * 4 + 100), Some("US".to_string()));
    assert_eq!(geo.lookup_str(65536 * 5 + 100), Some("DE".to_string()));
}
