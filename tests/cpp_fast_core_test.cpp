#include "throne_fast_core.h"

#include <algorithm>
#include <array>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <iostream>
#include <random>
#include <string>
#include <vector>

namespace {

int failures = 0;

#define CHECK(cond, msg)                                                                           \
    do {                                                                                           \
        if (!(cond)) {                                                                             \
            std::cerr << "FAIL [" << __LINE__ << "]: " << (msg) << "\n";                           \
            ++failures;                                                                            \
        }                                                                                          \
    } while (0)

struct RefRuleV4 {
    uint32_t net;
    uint32_t mask;
    uint8_t plen;
    uint32_t rule;
};

uint32_t naive_lpm_v4(const std::vector<RefRuleV4>& rules, uint32_t ip) {
    uint32_t best = THRONE_FAST_NO_RULE;
    int best_plen = -1;
    for (const auto& r : rules) {
        if (r.plen > best_plen && (ip & r.mask) == r.net) {
            best_plen = r.plen;
            best = r.rule;
        }
    }
    return best;
}

std::string to_cidr_v4(uint32_t net, uint8_t plen) {
    char buf[32];
    std::snprintf(buf, sizeof(buf), "%u.%u.%u.%u/%u",
                  (net >> 24) & 0xFFu, (net >> 16) & 0xFFu, (net >> 8) & 0xFFu, net & 0xFFu, plen);
    return buf;
}

uint64_t scalar_fnv1a(const uint8_t* p, size_t n) {
    uint64_t h = 0xcbf29ce484222325ULL;
    for (size_t i = 0; i < n; ++i) {
        h ^= p[i];
        h *= 0x0100000001b3ULL;
    }
    return h;
}

} // namespace

int main() {
    CHECK(throne_fast_abi_version() == THRONE_FAST_ABI_VERSION, "ABI version mismatch");
    CHECK(throne_fast_features() == 0xFFull, "expected all 8 acceleration features");
    const char* ver = throne_fast_version_string();
    CHECK(ver != nullptr && std::strstr(ver, "1.4.2-rust") != nullptr, "unexpected version string");

    // Hashing vectors
    CHECK(throne::fast::xxhash64("") == 0xEF46DB3751D8E999ULL, "xxhash64 empty vector");
    CHECK(throne::fast::xxhash64("abc") == 0x44BC2CF5AD770999ULL, "xxhash64 abc vector");
    CHECK(throne::fast::sha256_hex("") ==
              "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
          "sha256 empty");
    CHECK(throne::fast::sha256_hex("abc") ==
              "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
          "sha256 abc");

    // CIDR normalization & edge cases
    CHECK(throne::fast::normalize_cidr("192.168.1.99/24") == "192.168.1.0/24", "v4 cidr mask");
    CHECK(throne::fast::normalize_cidr(" 2001:0db8:0000:0000:0001::5/32 ") == "2001:db8::/32", "v6 cidr rfc5952");
    CHECK(throne::fast::normalize_cidr("[::1]/128") == "::1/128", "bracketed ipv6 cidr");
    CHECK(throne::fast::normalize_cidr("10.0.0.1/33").empty(), "reject >32 prefix");
    CHECK(throne::fast::normalize_cidr("not-an-ip").empty(), "reject invalid ip");

    CHECK(throne::fast::normalize_cidr_compact("1.2.3.4/32") == "1.2.3.4", "compact v4 omits /32");
    CHECK(throne::fast::normalize_cidr_compact("2001:db8::1/128") == "2001:db8::1", "compact v6 omits /128");
    CHECK(throne::fast::normalize_cidr_compact("::ffff:192.0.2.5/120") == "192.0.2.0/24", "compact unmaps ipv4-mapped v6");
    CHECK(throne::fast::normalize_cidr_compact("fe80::1%eth0/64") == "fe80::/64", "compact strips zone");
    CHECK(throne::fast::normalize_cidr_compact("0.0.0.0/0").empty(), "compact rejects /0");

    uint8_t small_buf[4]{};
    size_t out_len = 0;
    CHECK(throne_fast_normalize_cidr(reinterpret_cast<const uint8_t*>("1.1.1.1"), 7, small_buf, sizeof(small_buf), &out_len) == -1,
          "small buffer must return -1");

    // Range cover (192.168.1.5 - 192.168.1.10 -> 4 CIDRs) + full v4 0.0.0.0-255.255.255.255 + inverted error
    {
        uint8_t s16[16]{192, 168, 1, 5};
        uint8_t e16[16]{192, 168, 1, 10};
        ThroneFastCidr out[8]{};
        int cnt = throne_fast_cover_range(s16, e16, 4, out, 8);
        CHECK(cnt == 4, "cover_range expected 4 prefixes for .5-.10");
        CHECK(out[0].prefix_len == 32 && out[1].prefix_len == 31 && out[2].prefix_len == 31 && out[3].prefix_len == 32,
              "cover_range prefix lengths mismatch");
        CHECK(throne_fast_cover_range(e16, s16, 4, out, 8) == -1, "inverted range must fail");

        uint8_t zero16[16]{};
        uint8_t max16[16];
        std::memset(max16, 0xFF, sizeof(max16));
        CHECK(throne_fast_cover_range(zero16, max16, 4, out, 8) == 1 && out[0].prefix_len == 0, "v4 full /0 cover");
        CHECK(throne_fast_cover_range(zero16, max16, 6, out, 8) == 1 && out[0].prefix_len == 0, "v6 full /0 cover");
    }

    // Domain router
    {
        throne::fast::DomainRouter dr;
        CHECK(dr.insert("google.com", 1, 10), "insert suffix");
        CHECK(dr.insert("mail.google.com", 0, 20), "insert exact");
        CHECK(dr.insert("analytics", 2, 30), "insert keyword");
        CHECK(dr.match("MAIL.GOOGLE.COM.") == 20, "exact beats suffix");
        CHECK(dr.match("drive.google.com") == 10, "suffix match");
        CHECK(dr.match("notgoogle.com") == THRONE_FAST_NO_RULE, "label boundary check");
        CHECK(dr.match("my-analytics.example.net") == 30, "keyword match");
    }

    // GeoIP
    {
        std::vector<ThroneFastGeoRange> gr = {
            {0x08080800u, 0x080808FFu, {'U', 'S'}, {0, 0}},
            {0x01010100u, 0x010101FFu, {'A', 'U'}, {0, 0}},
            {0x01010200u, 0x010102FFu, {'A', 'U'}, {0, 0}},
        };
        ThroneFastGeoIndex* gi = throne_fast_geo_create(gr.data(), gr.size());
        CHECK(gi != nullptr, "geo_create non-null");
        uint8_t cc[2]{};
        CHECK(throne_fast_geo_lookup_v4(gi, 0x08080808u, cc) == 1 && cc[0] == 'U' && cc[1] == 'S', "8.8.8.8 == US");
        CHECK(throne_fast_geo_lookup_v4(gi, 0x01010101u, cc) == 1 && cc[0] == 'A' && cc[1] == 'U', "1.1.1.1 == AU");
        CHECK(throne_fast_geo_lookup_v4(gi, 0x09090909u, cc) == 0, "miss returns 0");
        throne_fast_geo_free(gi);
    }

    // Differential test: 4-bit Arena Trie vs Linear Reference Oracle + Benchmark
    {
        std::mt19937_64 rng(0x13579BDF2468ACE0ULL);
        throne::fast::RouteTable rt;
        std::vector<RefRuleV4> ref;

        // Include boundary rules (/0, /1, /25, /32) plus 500 random CIDRs
        auto add_rule = [&](uint32_t net, uint8_t plen, uint32_t rule) {
            uint32_t mask = (plen == 0) ? 0u : (~0u << (32 - plen));
            net &= mask;
            rt.insert_cidr(to_cidr_v4(net, plen), rule);
            // Replace if identical (last write wins matches trie semantics)
            bool updated = false;
            for (auto& existing : ref) {
                if (existing.plen == plen && existing.net == net) {
                    existing.rule = rule;
                    updated = true;
                    break;
                }
            }
            if (!updated) {
                ref.push_back(RefRuleV4{net, mask, plen, rule});
            }
        };

        add_rule(0x00000000u, 0, 1);
        add_rule(0x0A000000u, 8, 2);
        add_rule(0x0A000000u, 24, 3);
        add_rule(0x0A000000u, 25, 4);
        add_rule(0xFFFFFFFFu, 32, 5);
        for (uint32_t i = 0; i < 500; ++i) {
            uint32_t ip = static_cast<uint32_t>(rng());
            uint8_t plen = static_cast<uint8_t>(1 + (rng() % 32));
            add_rule(ip, plen, 100 + i);
        }

        std::vector<uint32_t> probes(10000);
        for (size_t i = 0; i < probes.size(); ++i) {
            probes[i] = static_cast<uint32_t>(rng());
        }
        probes[0] = 0u;
        probes[1] = 0xFFFFFFFFu;
        probes[2] = 0x0A00007Fu;
        probes[3] = 0x0A000080u;

        std::vector<uint32_t> batch_out(probes.size(), 0);
        throne_fast_route_match_batch_v4(rt.raw(), probes.data(), batch_out.data(), probes.size());

        for (size_t i = 0; i < probes.size(); ++i) {
            const uint32_t expected = naive_lpm_v4(ref, probes[i]);
            const uint32_t got_single = rt.match_v4(probes[i]);
            CHECK(got_single == expected, "trie single vs ref mismatch");
            CHECK(batch_out[i] == expected, "trie batch vs ref mismatch");
        }

        // Benchmark: Arena Batch LPM vs Scalar Linear Prefix Scan on 500 CIDRs x 10,000 queries
        volatile uint64_t sink = 0;
        const auto t0 = std::chrono::steady_clock::now();
        for (int round = 0; round < 50; ++round) {
            throne_fast_route_match_batch_v4(rt.raw(), probes.data(), batch_out.data(), probes.size());
            sink += batch_out[round & 255];
        }
        const auto t1 = std::chrono::steady_clock::now();
        for (int round = 0; round < 50; ++round) {
            for (size_t i = 0; i < probes.size(); ++i) {
                sink += naive_lpm_v4(ref, probes[i]);
            }
        }
        const auto t2 = std::chrono::steady_clock::now();
        const double rustMs = std::chrono::duration<double, std::milli>(t1 - t0).count();
        const double naiveMs = std::chrono::duration<double, std::milli>(t2 - t1).count();
        std::cout << "BENCH route_v4_lpm (500k lookups over 505 CIDRs): Rust Arena Batch="
                  << rustMs << " ms (" << (rustMs * 1e6 / 500000.0) << " ns/lookup) vs Linear="
                  << naiveMs << " ms (" << (naiveMs * 1e6 / 500000.0) << " ns/lookup) -> "
                  << (naiveMs / rustMs) << "x speedup\n";

        // Benchmark xxHash64 vs scalar FNV1a on 4 KB buffers
        std::vector<uint8_t> payload(4096, 0x5A);
        const auto h0 = std::chrono::steady_clock::now();
        for (int i = 0; i < 200000; ++i) {
            sink += throne_fast_xxhash64(payload.data(), payload.size(), i);
        }
        const auto h1 = std::chrono::steady_clock::now();
        for (int i = 0; i < 200000; ++i) {
            sink += scalar_fnv1a(payload.data(), payload.size());
        }
        const auto h2 = std::chrono::steady_clock::now();
        const double xxhMs = std::chrono::duration<double, std::milli>(h1 - h0).count();
        const double fnvMs = std::chrono::duration<double, std::milli>(h2 - h1).count();
        const double gb = (200000.0 * 4096.0) / (1024.0 * 1024.0 * 1024.0);
        std::cout << "BENCH hash_4kb (800 MB total): Rust xxHash64=" << xxhMs << " ms ("
                  << (gb / (xxhMs / 1000.0)) << " GB/s) vs Scalar=" << fnvMs << " ms ("
                  << (gb / (fnvMs / 1000.0)) << " GB/s) -> " << (fnvMs / xxhMs) << "x speedup\n";
    }

    if (failures == 0) {
        std::cout << "ALL C++ <-> Rust differential & ABI checks PASSED!\n";
        return 0;
    }
    return 1;
}
