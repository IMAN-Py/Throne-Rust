pub mod cidr;
pub mod dns;
pub mod geo;
pub mod hash;
pub mod router;
pub mod scan_accel;
pub mod trie;

use core::ffi::c_char;

pub const THRONE_FAST_ABI_VERSION: u32 = 0x0002_0001;
pub const FEATURE_XXHASH64: u64 = 1 << 0;
pub const FEATURE_SHA256: u64 = 1 << 1;
pub const FEATURE_CIDR_ARENA: u64 = 1 << 2;
pub const FEATURE_SCAN_BATCH: u64 = 1 << 3;
pub const FEATURE_DNS_XDNS: u64 = 1 << 4;
pub const FEATURE_DOMAIN_ROUTER: u64 = 1 << 5;
pub const FEATURE_GEO_INDEX: u64 = 1 << 6;

static VERSION_CSTR: &[u8] = b"throne_fast_core 1.4.2-rust (ABI 2.1)\0";

#[no_mangle]
pub extern "C" fn throne_fast_abi_version() -> u32 {
    THRONE_FAST_ABI_VERSION
}

#[no_mangle]
pub extern "C" fn throne_fast_version_string() -> *const c_char {
    VERSION_CSTR.as_ptr() as *const c_char
}

#[no_mangle]
pub extern "C" fn throne_fast_features() -> u64 {
    FEATURE_XXHASH64
        | FEATURE_SHA256
        | FEATURE_CIDR_ARENA
        | FEATURE_SCAN_BATCH
        | FEATURE_DNS_XDNS
        | FEATURE_DOMAIN_ROUTER
        | FEATURE_GEO_INDEX
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_and_features_present() {
        assert_eq!(throne_fast_abi_version(), THRONE_FAST_ABI_VERSION);
        assert_ne!(throne_fast_features(), 0);
    }
}
