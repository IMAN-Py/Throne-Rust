//! High-performance non-cryptographic hash functions for route tokenization,
//! connection table fingerprinting, and Bloom filter acceleration.

/// 64-bit FNV-1a streaming hash (standard offset basis & prime)
pub const FNV1A_64_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
pub const FNV1A_64_PRIME: u64 = 0x100000001b3;

#[inline]
pub fn fnv1a_64(data: &[u8]) -> u64 {
    let mut hash = FNV1A_64_OFFSET_BASIS;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV1A_64_PRIME);
    }
    hash
}

/// 32-bit FNV-1a hash
pub const FNV1A_32_OFFSET_BASIS: u32 = 0x811c9dc5;
pub const FNV1A_32_PRIME: u32 = 0x01000193;

#[inline]
pub fn fnv1a_32(data: &[u8]) -> u32 {
    let mut hash = FNV1A_32_OFFSET_BASIS;
    for &byte in data {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(FNV1A_32_PRIME);
    }
    hash
}

/// xxHash64 constants
const PRIME64_1: u64 = 0x9E3779B185EBCA87;
const PRIME64_2: u64 = 0xC2B2AE3D27D4EB4F;
const PRIME64_3: u64 = 0x165667B19E3779F9;
const PRIME64_4: u64 = 0x85EBCA77C2B2AE63;
const PRIME64_5: u64 = 0x27D4EB2F165667C5;

#[inline]
fn rotl64(x: u64, r: u32) -> u64 {
    (x << r) | (x >> (64 - r))
}

#[inline]
fn round64(acc: u64, input: u64) -> u64 {
    rotl64(acc.wrapping_add(input.wrapping_mul(PRIME64_2)), 31).wrapping_mul(PRIME64_1)
}

/// Fast xxHash64 implementation
pub fn xxhash64(input: &[u8], seed: u64) -> u64 {
    let len = input.len();
    let mut h64: u64;

    if len >= 32 {
        let mut v1 = seed.wrapping_add(PRIME64_1).wrapping_add(PRIME64_2);
        let mut v2 = seed.wrapping_add(PRIME64_2);
        let mut v3 = seed;
        let mut v4 = seed.wrapping_sub(PRIME64_1);

        let mut offset = 0;
        let limit = len - 32;

        while offset <= limit {
            v1 = round64(v1, read_u64_le(&input[offset..offset + 8]));
            v2 = round64(v2, read_u64_le(&input[offset + 8..offset + 16]));
            v3 = round64(v3, read_u64_le(&input[offset + 16..offset + 24]));
            v4 = round64(v4, read_u64_le(&input[offset + 24..offset + 32]));
            offset += 32;
        }

        h64 = rotl64(v1, 1)
            .wrapping_add(rotl64(v2, 7))
            .wrapping_add(rotl64(v3, 12))
            .wrapping_add(rotl64(v4, 18));

        h64 = merge_round64(h64, v1);
        h64 = merge_round64(h64, v2);
        h64 = merge_round64(h64, v3);
        h64 = merge_round64(h64, v4);
    } else {
        h64 = seed.wrapping_add(PRIME64_5);
    }

    h64 = h64.wrapping_add(len as u64);

    let mut remaining = len & 31;
    let mut offset = len - remaining;

    while remaining >= 8 {
        let k1 = round64(0, read_u64_le(&input[offset..offset + 8]));
        h64 = rotl64(h64 ^ k1, 27).wrapping_mul(PRIME64_1).wrapping_add(PRIME64_4);
        offset += 8;
        remaining -= 8;
    }

    if remaining >= 4 {
        h64 = rotl64(
            h64 ^ (read_u32_le(&input[offset..offset + 4]) as u64).wrapping_mul(PRIME64_1),
            23,
        )
        .wrapping_mul(PRIME64_2)
        .wrapping_add(PRIME64_3);
        offset += 4;
        remaining -= 4;
    }

    while remaining > 0 {
        h64 = rotl64(h64 ^ (input[offset] as u64).wrapping_mul(PRIME64_5), 11)
            .wrapping_mul(PRIME64_1);
        offset += 1;
        remaining -= 1;
    }

    // Avalanche finalizer
    h64 ^= h64 >> 33;
    h64 = h64.wrapping_mul(PRIME64_2);
    h64 ^= h64 >> 29;
    h64 = h64.wrapping_mul(PRIME64_3);
    h64 ^= h64 >> 32;

    h64
}

#[inline]
fn merge_round64(mut acc: u64, val: u64) -> u64 {
    let val_rounded = round64(0, val);
    acc ^= val_rounded;
    acc.wrapping_mul(PRIME64_1).wrapping_add(PRIME64_4)
}

#[inline]
fn read_u64_le(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes[0..8].try_into().unwrap())
}

#[inline]
fn read_u32_le(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes[0..4].try_into().unwrap())
}

/// Fast Murmur3A 32-bit hash for routing buckets
pub fn murmur3_32(data: &[u8], seed: u32) -> u32 {
    let c1: u32 = 0xcc9e2d51;
    let c2: u32 = 0x1b873593;
    let mut h1 = seed;

    let nblocks = data.len() / 4;
    for i in 0..nblocks {
        let mut k1 = read_u32_le(&data[i * 4..(i + 1) * 4]);
        k1 = k1.wrapping_mul(c1);
        k1 = (k1 << 15) | (k1 >> 17);
        k1 = k1.wrapping_mul(c2);

        h1 ^= k1;
        h1 = (h1 << 13) | (h1 >> 19);
        h1 = h1.wrapping_mul(5).wrapping_add(0xe6546b64);
    }

    let tail = &data[nblocks * 4..];
    let mut k1 = 0u32;
    match tail.len() {
        3 => {
            k1 ^= (tail[2] as u32) << 16;
            k1 ^= (tail[1] as u32) << 8;
            k1 ^= tail[0] as u32;
            k1 = k1.wrapping_mul(c1);
            k1 = (k1 << 15) | (k1 >> 17);
            k1 = k1.wrapping_mul(c2);
            h1 ^= k1;
        }
        2 => {
            k1 ^= (tail[1] as u32) << 8;
            k1 ^= tail[0] as u32;
            k1 = k1.wrapping_mul(c1);
            k1 = (k1 << 15) | (k1 >> 17);
            k1 = k1.wrapping_mul(c2);
            h1 ^= k1;
        }
        1 => {
            k1 ^= tail[0] as u32;
            k1 = k1.wrapping_mul(c1);
            k1 = (k1 << 15) | (k1 >> 17);
            k1 = k1.wrapping_mul(c2);
            h1 ^= k1;
        }
        _ => {}
    }

    h1 ^= data.len() as u32;
    h1 ^= h1 >> 16;
    h1 = h1.wrapping_mul(0x85ebca6b);
    h1 ^= h1 >> 13;
    h1 = h1.wrapping_mul(0xc2b2ae35);
    h1 ^= h1 >> 16;

    h1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fnv1a_determinism() {
        let input = b"throne-fast-core-routing-test";
        let h1 = fnv1a_64(input);
        let h2 = fnv1a_64(input);
        assert_eq!(h1, h2);
        assert_ne!(h1, 0);
    }

    #[test]
    fn test_xxhash64_determinism() {
        let input = b"proxy-node-fingerprint-de-fra-01";
        let h1 = xxhash64(input, 0);
        let h2 = xxhash64(input, 0);
        assert_eq!(h1, h2);
        assert_ne!(h1, 0);
    }

    #[test]
    fn test_murmur3_determinism() {
        let input = b"api.sing-box.throne.internal";
        let h1 = murmur3_32(input, 42);
        let h2 = murmur3_32(input, 42);
        assert_eq!(h1, h2);
    }
}
