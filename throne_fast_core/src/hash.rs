use core::slice;

const XXH_PRIME64_1: u64 = 0x9E37_79B1_85EB_CA87;
const XXH_PRIME64_2: u64 = 0xC2B2_AE3D_27D4_EB4F;
const XXH_PRIME64_3: u64 = 0x1656_67B1_9E37_79F9;
const XXH_PRIME64_4: u64 = 0x85EB_CA77_C2B2_AE63;
const XXH_PRIME64_5: u64 = 0x27D4_EB2F_1656_67C5;

#[inline(always)]
fn xxh64_round(acc: u64, input: u64) -> u64 {
    acc.wrapping_add(input.wrapping_mul(XXH_PRIME64_2))
        .rotate_left(31)
        .wrapping_mul(XXH_PRIME64_1)
}

#[inline(always)]
fn xxh64_merge_round(mut acc: u64, val: u64) -> u64 {
    let v = xxh64_round(0, val);
    acc ^= v;
    acc.wrapping_mul(XXH_PRIME64_1).wrapping_add(XXH_PRIME64_4)
}

#[inline(always)]
fn read_u64_le(slice: &[u8]) -> u64 {
    let arr: [u8; 8] = slice[..8].try_into().unwrap_or([0; 8]);
    u64::from_le_bytes(arr)
}

#[inline(always)]
fn read_u32_le(slice: &[u8]) -> u32 {
    let arr: [u8; 4] = slice[..4].try_into().unwrap_or([0; 4]);
    u32::from_le_bytes(arr)
}

#[inline]
pub fn xxhash64(data: &[u8], seed: u64) -> u64 {
    let len = data.len();
    let mut h64: u64;
    let mut idx = 0usize;

    if len >= 32 {
        let mut v1 = seed.wrapping_add(XXH_PRIME64_1).wrapping_add(XXH_PRIME64_2);
        let mut v2 = seed.wrapping_add(XXH_PRIME64_2);
        let mut v3 = seed;
        let mut v4 = seed.wrapping_sub(XXH_PRIME64_1);

        let limit = len - 32;
        while idx <= limit {
            v1 = xxh64_round(v1, read_u64_le(&data[idx..idx + 8]));
            v2 = xxh64_round(v2, read_u64_le(&data[idx + 8..idx + 16]));
            v3 = xxh64_round(v3, read_u64_le(&data[idx + 16..idx + 24]));
            v4 = xxh64_round(v4, read_u64_le(&data[idx + 24..idx + 32]));
            idx += 32;
        }

        h64 = v1
            .rotate_left(1)
            .wrapping_add(v2.rotate_left(7))
            .wrapping_add(v3.rotate_left(12))
            .wrapping_add(v4.rotate_left(18));
        h64 = xxh64_merge_round(h64, v1);
        h64 = xxh64_merge_round(h64, v2);
        h64 = xxh64_merge_round(h64, v3);
        h64 = xxh64_merge_round(h64, v4);
    } else {
        h64 = seed.wrapping_add(XXH_PRIME64_5);
    }

    h64 = h64.wrapping_add(len as u64);

    while idx + 8 <= len {
        let k1 = xxh64_round(0, read_u64_le(&data[idx..idx + 8]));
        h64 ^= k1;
        h64 = h64
            .rotate_left(27)
            .wrapping_mul(XXH_PRIME64_1)
            .wrapping_add(XXH_PRIME64_4);
        idx += 8;
    }

    if idx + 4 <= len {
        let k1 = (read_u32_le(&data[idx..idx + 4]) as u64).wrapping_mul(XXH_PRIME64_1);
        h64 ^= k1;
        h64 = h64
            .rotate_left(23)
            .wrapping_mul(XXH_PRIME64_2)
            .wrapping_add(XXH_PRIME64_3);
        idx += 4;
    }

    while idx < len {
        h64 ^= (data[idx] as u64).wrapping_mul(XXH_PRIME64_5);
        h64 = h64.rotate_left(11).wrapping_mul(XXH_PRIME64_1);
        idx += 1;
    }

    h64 ^= h64 >> 33;
    h64 = h64.wrapping_mul(XXH_PRIME64_2);
    h64 ^= h64 >> 29;
    h64 = h64.wrapping_mul(XXH_PRIME64_3);
    h64 ^= h64 >> 32;
    h64
}

#[inline]
pub fn fnv1a_64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

#[inline]
pub fn murmur3_32(data: &[u8], seed: u32) -> u32 {
    const C1: u32 = 0xcc9e_2d51;
    const C2: u32 = 0x1b87_3593;
    let mut h = seed;
    let mut idx = 0usize;
    let len = data.len();

    while idx + 4 <= len {
        let mut k = read_u32_le(&data[idx..idx + 4]);
        k = k.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
        h ^= k;
        h = h.rotate_left(13).wrapping_mul(5).wrapping_add(0xe654_6b64);
        idx += 4;
    }

    let mut tail: u32 = 0;
    let rem = len - idx;
    if rem >= 3 {
        tail ^= (data[idx + 2] as u32) << 16;
    }
    if rem >= 2 {
        tail ^= (data[idx + 1] as u32) << 8;
    }
    if rem >= 1 {
        tail ^= data[idx] as u32;
        tail = tail.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
        h ^= tail;
    }

    h ^= len as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    h
}

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

#[inline]
fn sha256_compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for i in 0..16 {
        let base = i * 4;
        w[i] = u32::from_be_bytes([block[base], block[base + 1], block[base + 2], block[base + 3]]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }

    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ ((!e) & g);
        let temp1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(SHA256_K[i])
            .wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let temp2 = s0.wrapping_add(maj);

        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(temp1);
        d = c;
        c = b;
        b = a;
        a = temp1.wrapping_add(temp2);
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

pub fn sha256_raw(data: &[u8]) -> [u8; 32] {
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];

    let mut offset = 0usize;
    while offset + 64 <= data.len() {
        let block: &[u8; 64] = data[offset..offset + 64].try_into().unwrap();
        sha256_compress(&mut state, block);
        offset += 64;
    }

    let rem = data.len() - offset;
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut tail = [0u8; 128];
    tail[..rem].copy_from_slice(&data[offset..]);
    tail[rem] = 0x80;

    let total_tail = if rem + 9 <= 64 { 64 } else { 128 };
    tail[total_tail - 8..total_tail].copy_from_slice(&bit_len.to_be_bytes());

    let b0: &[u8; 64] = tail[0..64].try_into().unwrap();
    sha256_compress(&mut state, b0);
    if total_tail == 128 {
        let b1: &[u8; 64] = tail[64..128].try_into().unwrap();
        sha256_compress(&mut state, b1);
    }

    let mut out = [0u8; 32];
    for (i, word) in state.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

pub fn sha256_hex(data: &[u8]) -> [u8; 64] {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = sha256_raw(data);
    let mut out = [0u8; 64];
    for (i, &b) in digest.iter().enumerate() {
        out[i * 2] = HEX[(b >> 4) as usize];
        out[i * 2 + 1] = HEX[(b & 0x0f) as usize];
    }
    out
}

/// # Safety
/// `data` must be valid for `len` readable bytes (or `len == 0`).
#[no_mangle]
pub unsafe extern "C" fn throne_fast_xxhash64(data: *const u8, len: usize, seed: u64) -> u64 {
    if len == 0 {
        return xxhash64(&[], seed);
    }
    if data.is_null() {
        return 0;
    }
    let slice = unsafe { slice::from_raw_parts(data, len) };
    xxhash64(slice, seed)
}

/// `algo`: 0 = xxhash64, 1 = fnv1a_64, 2 = murmur3_32 (widened to u64).
///
/// # Safety
/// `data` must be valid for `len` readable bytes (or `len == 0`).
#[no_mangle]
pub unsafe extern "C" fn throne_fast_hash(
    data: *const u8,
    len: usize,
    algo: u32,
    seed: u64,
) -> u64 {
    let slice = if len == 0 {
        &[]
    } else if data.is_null() {
        return 0;
    } else {
        unsafe { slice::from_raw_parts(data, len) }
    };
    match algo {
        0 => xxhash64(slice, seed),
        1 => fnv1a_64(slice),
        2 => murmur3_32(slice, seed as u32) as u64,
        _ => xxhash64(slice, seed),
    }
}

/// # Safety
/// `out32` must be writable for at least 32 bytes; `data` readable for `len` bytes if `len > 0`.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_sha256_raw(
    data: *const u8,
    len: usize,
    out32: *mut u8,
) -> i32 {
    if out32.is_null() {
        return -1;
    }
    let slice = if len == 0 {
        &[]
    } else if data.is_null() {
        return -1;
    } else {
        unsafe { slice::from_raw_parts(data, len) }
    };
    let digest = sha256_raw(slice);
    unsafe { slice::from_raw_parts_mut(out32, 32) }.copy_from_slice(&digest);
    0
}

/// # Safety
/// `out64` must be writable for at least 64 bytes; `data` readable for `len` bytes if `len > 0`.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_sha256_hex(
    data: *const u8,
    len: usize,
    out64: *mut u8,
) -> i32 {
    if out64.is_null() {
        return -1;
    }
    let slice = if len == 0 {
        &[]
    } else if data.is_null() {
        return -1;
    } else {
        unsafe { slice::from_raw_parts(data, len) }
    };
    let hex = sha256_hex(slice);
    unsafe { slice::from_raw_parts_mut(out64, 64) }.copy_from_slice(&hex);
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xxhash64_known_vectors() {
        assert_eq!(xxhash64(b"", 0), 0xEF46_DB37_51D8_E999);
        assert_eq!(xxhash64(b"", 0x9E37_79B1), 0xAC75_FDA2_929B_17EF);
        assert_eq!(xxhash64(b"abc", 0), 0x44BC_2CF5_AD77_0999);
        // > 32 bytes boundary check
        let long = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
        assert_ne!(xxhash64(long, 0), 0);
        assert_eq!(xxhash64(long, 42), xxhash64(long, 42));
    }

    #[test]
    fn sha256_nist_vectors() {
        let empty = sha256_hex(b"");
        assert_eq!(
            &empty,
            b"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        let abc = sha256_hex(b"abc");
        assert_eq!(
            &abc,
            b"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // 56-byte boundary (forces 2-block padding)
        let msg56 = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
        assert_eq!(
            &sha256_hex(msg56),
            b"248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }
}
