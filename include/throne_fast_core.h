#pragma once

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
#include <array>
#include <cstring>
#include <memory>
#include <string>
#include <string_view>
#include <vector>

extern "C" {
#endif

#define THRONE_FAST_ABI_VERSION 0x00020001u
#define THRONE_FAST_NO_RULE 0xFFFFFFFFu

typedef struct ThroneFastRouteTable ThroneFastRouteTable;
typedef struct ThroneFastDomainRouter ThroneFastDomainRouter;
typedef struct ThroneFastGeoIndex ThroneFastGeoIndex;
typedef struct ThroneFastScanPlan ThroneFastScanPlan;

typedef struct ThroneFastCidr {
    uint8_t addr[16];
    uint8_t prefix_len;
    uint8_t family;
    uint8_t _pad[6];
} ThroneFastCidr;

typedef struct ThroneFastScanRangeDesc {
    uint8_t base[16];
    uint64_t start;
    uint64_t end;
    uint64_t high_key;
    uint64_t sampler_key;
    uint32_t port_offset;
    uint32_t port_count;
    uint8_t family;
    uint8_t host_bits;
    uint8_t sampled;
    uint8_t _pad[5];
} ThroneFastScanRangeDesc;

typedef struct ThroneFastScanTarget {
    uint8_t addr[16];
    uint16_t port;
    uint8_t family;
    uint8_t _pad[5];
} ThroneFastScanTarget;

typedef struct ThroneFastXdnsResult {
    uint8_t matched;
    uint8_t rcode;
    uint8_t has_txt;
    uint8_t _pad;
} ThroneFastXdnsResult;

typedef struct ThroneFastGeoRange {
    uint32_t start;
    uint32_t end;
    uint8_t country[2];
    uint8_t _pad[2];
} ThroneFastGeoRange;

uint32_t throne_fast_abi_version(void);
const char* throne_fast_version_string(void);
uint64_t throne_fast_features(void);

uint64_t throne_fast_xxhash64(const uint8_t* data, size_t len, uint64_t seed);
uint64_t throne_fast_hash(const uint8_t* data, size_t len, uint32_t algo, uint64_t seed);
int32_t throne_fast_sha256_raw(const uint8_t* data, size_t len, uint8_t* out32);
int32_t throne_fast_sha256_hex(const uint8_t* data, size_t len, uint8_t* out64);

int32_t throne_fast_parse_ip(const uint8_t* input, size_t len, uint8_t* out_family, uint8_t* out_addr16);
int32_t throne_fast_parse_cidr(const uint8_t* input, size_t len, ThroneFastCidr* out);
int32_t throne_fast_normalize_cidr(const uint8_t* input, size_t len, uint8_t* out_buf, size_t out_cap, size_t* out_len);
int32_t throne_fast_normalize_cidr_compact(const uint8_t* input, size_t len, uint8_t* out_buf, size_t out_cap, size_t* out_len);
int32_t throne_fast_format_cidr_compact(const ThroneFastCidr* cidr, uint8_t* out_buf, size_t out_cap, size_t* out_len);
int32_t throne_fast_cover_range(const uint8_t* start_be16, const uint8_t* end_be16, uint8_t family, ThroneFastCidr* out_cidrs, size_t max_out);

ThroneFastScanPlan* throne_fast_scan_create(
    const ThroneFastScanRangeDesc* descs,
    size_t desc_count,
    const uint16_t* ports,
    size_t port_count,
    uint64_t total,
    uint8_t shuffle,
    uint64_t seed);
void throne_fast_scan_free(ThroneFastScanPlan* plan);
size_t throne_fast_scan_fill_batch(
    const ThroneFastScanPlan* plan,
    uint64_t start_pos,
    size_t count,
    ThroneFastScanTarget* out_targets);

int32_t throne_fast_dns_validate_packet(const uint8_t* pkt, size_t len);
int32_t throne_fast_dns_match_xdns_reply(
    const uint8_t* pkt,
    size_t pkt_len,
    uint16_t expected_id,
    const uint8_t* expected_name,
    size_t name_len,
    ThroneFastXdnsResult* out);

ThroneFastRouteTable* throne_fast_route_create(void);
void throne_fast_route_free(ThroneFastRouteTable* table);
int32_t throne_fast_route_insert_cidr(ThroneFastRouteTable* table, const uint8_t* cidr, size_t len, uint32_t rule_id);
int32_t throne_fast_route_insert_v4(ThroneFastRouteTable* table, uint32_t addr_host, uint8_t prefix_len, uint32_t rule_id);
int32_t throne_fast_route_insert_v6(ThroneFastRouteTable* table, const uint8_t* addr_be16, uint8_t prefix_len, uint32_t rule_id);
uint32_t throne_fast_route_match_v4(const ThroneFastRouteTable* table, uint32_t addr_host);
uint32_t throne_fast_route_match_v6(const ThroneFastRouteTable* table, const uint8_t* addr_be16);
size_t throne_fast_route_match_batch_v4(
    const ThroneFastRouteTable* table,
    const uint32_t* addrs_host,
    uint32_t* out_rules,
    size_t count);

ThroneFastDomainRouter* throne_fast_domain_create(void);
void throne_fast_domain_free(ThroneFastDomainRouter* router);
int32_t throne_fast_domain_insert(ThroneFastDomainRouter* router, const uint8_t* pattern, size_t len, uint8_t kind, uint32_t rule_id);
uint32_t throne_fast_domain_match(const ThroneFastDomainRouter* router, const uint8_t* host, size_t len);

ThroneFastGeoIndex* throne_fast_geo_create(const ThroneFastGeoRange* ranges, size_t count);
void throne_fast_geo_free(ThroneFastGeoIndex* index);
int32_t throne_fast_geo_lookup_v4(const ThroneFastGeoIndex* index, uint32_t ip_host, uint8_t* out_cc2);

#ifdef __cplusplus
} // extern "C"

namespace throne::fast {

inline uint64_t xxhash64(std::string_view sv, uint64_t seed = 0) noexcept {
    return throne_fast_xxhash64(reinterpret_cast<const uint8_t*>(sv.data()), sv.size(), seed);
}

inline std::string sha256_hex(std::string_view sv) {
    std::string out(64, '\0');
    throne_fast_sha256_hex(
        reinterpret_cast<const uint8_t*>(sv.data()),
        sv.size(),
        reinterpret_cast<uint8_t*>(out.data()));
    return out;
}

inline std::array<uint8_t, 32> sha256_raw(std::string_view sv) noexcept {
    std::array<uint8_t, 32> out{};
    throne_fast_sha256_raw(
        reinterpret_cast<const uint8_t*>(sv.data()),
        sv.size(),
        out.data());
    return out;
}

inline bool parse_cidr(std::string_view sv, ThroneFastCidr& out) noexcept {
    return throne_fast_parse_cidr(reinterpret_cast<const uint8_t*>(sv.data()), sv.size(), &out) == 0;
}

inline std::string normalize_cidr(std::string_view sv) {
    char buf[96];
    size_t out_len = 0;
    if (throne_fast_normalize_cidr(
            reinterpret_cast<const uint8_t*>(sv.data()),
            sv.size(),
            reinterpret_cast<uint8_t*>(buf),
            sizeof(buf),
            &out_len) == 0) {
        return std::string(buf, out_len);
    }
    return {};
}

inline std::string normalize_cidr_compact(std::string_view sv) {
    char buf[96];
    size_t out_len = 0;
    if (throne_fast_normalize_cidr_compact(
            reinterpret_cast<const uint8_t*>(sv.data()),
            sv.size(),
            reinterpret_cast<uint8_t*>(buf),
            sizeof(buf),
            &out_len) == 0) {
        return std::string(buf, out_len);
    }
    return {};
}

inline std::string format_cidr_compact(const ThroneFastCidr& cidr) {
    char buf[96];
    size_t out_len = 0;
    if (throne_fast_format_cidr_compact(
            &cidr,
            reinterpret_cast<uint8_t*>(buf),
            sizeof(buf),
            &out_len) == 0) {
        return std::string(buf, out_len);
    }
    return {};
}

class RouteTable {
public:
    RouteTable() : ptr_(throne_fast_route_create()) {}
    ~RouteTable() {
        if (ptr_) {
            throne_fast_route_free(ptr_);
        }
    }
    RouteTable(const RouteTable&) = delete;
    RouteTable& operator=(const RouteTable&) = delete;
    RouteTable(RouteTable&& o) noexcept : ptr_(o.ptr_) { o.ptr_ = nullptr; }
    RouteTable& operator=(RouteTable&& o) noexcept {
        if (this != &o) {
            if (ptr_) throne_fast_route_free(ptr_);
            ptr_ = o.ptr_;
            o.ptr_ = nullptr;
        }
        return *this;
    }

    bool insert_cidr(std::string_view cidr, uint32_t rule_id) {
        return ptr_ && throne_fast_route_insert_cidr(ptr_, reinterpret_cast<const uint8_t*>(cidr.data()), cidr.size(), rule_id) == 0;
    }
    uint32_t match_v4(uint32_t addr_host) const noexcept {
        return ptr_ ? throne_fast_route_match_v4(ptr_, addr_host) : THRONE_FAST_NO_RULE;
    }
    ThroneFastRouteTable* raw() const noexcept { return ptr_; }

private:
    ThroneFastRouteTable* ptr_ = nullptr;
};

class DomainRouter {
public:
    DomainRouter() : ptr_(throne_fast_domain_create()) {}
    ~DomainRouter() {
        if (ptr_) {
            throne_fast_domain_free(ptr_);
        }
    }
    DomainRouter(const DomainRouter&) = delete;
    DomainRouter& operator=(const DomainRouter&) = delete;

    bool insert(std::string_view pattern, uint8_t kind, uint32_t rule_id) {
        return ptr_ && throne_fast_domain_insert(ptr_, reinterpret_cast<const uint8_t*>(pattern.data()), pattern.size(), kind, rule_id) == 0;
    }
    uint32_t match(std::string_view host) const noexcept {
        return ptr_ ? throne_fast_domain_match(ptr_, reinterpret_cast<const uint8_t*>(host.data()), host.size()) : THRONE_FAST_NO_RULE;
    }

private:
    ThroneFastDomainRouter* ptr_ = nullptr;
};

} // namespace throne::fast
#endif
