#include <iostream>
#include <vector>
#include <thread>
#include <atomic>
#include <chrono>
#include <random>
#include <cassert>
#include <cstdint>
#include <cstring>
#include <throne_fast_core.h>

#ifdef _WIN32
#include <windows.h>
#include <psapi.h>
#endif

// Returns current working set size in bytes
static size_t getMemoryUsage() {
#ifdef _WIN32
    PROCESS_MEMORY_COUNTERS pmc;
    if (GetProcessMemoryInfo(GetCurrentProcess(), &pmc, sizeof(pmc))) {
        return pmc.WorkingSetSize;
    }
#endif
    return 0;
}

struct QueryItem {
    uint32_t ip;
    int expected_tag;
};

// Test Suite 1: C++ Multi-Threaded Read Concurrency across 8 Threads
bool run_route_concurrency_stress(int trial, unsigned int num_threads, uint64_t queries_per_thread) {
    std::cout << "\n----------------------------------------------------------------------\n";
    std::cout << "TRIAL " << trial << ": 8-THREAD ROUTE READ CONCURRENCY (" 
              << (num_threads * queries_per_thread) << " QUERIES)\n";
    std::cout << "----------------------------------------------------------------------\n";

    throne::fast::FastRouteEngine engine;
    if (!engine.isValid()) {
        std::cerr << "[FATAL] Failed to initialize FastRouteEngine\n";
        return false;
    }

    // 1. Base hierarchy
    engine.addPrefix(0, 0, 0, 0, 0, 1);                  // 0.0.0.0/0 -> tag 1 (default)
    engine.addPrefix(10, 0, 0, 0, 8, 100);              // 10.0.0.0/8 -> tag 100
    engine.addPrefix(10, 1, 0, 0, 16, 101);             // 10.1.0.0/16 -> tag 101
    engine.addPrefix(10, 1, 2, 0, 24, 102);             // 10.1.2.0/24 -> tag 102
    engine.addPrefix(10, 1, 2, 3, 32, 103);             // 10.1.2.3/32 -> tag 103
    engine.addPrefix(172, 16, 0, 0, 12, 200);           // 172.16.0.0/12 -> tag 200
    engine.addPrefix(192, 168, 0, 0, 16, 300);          // 192.168.0.0/16 -> tag 300
    engine.addPrefix(192, 168, 1, 0, 24, 301);          // 192.168.1.0/24 -> tag 301
    engine.addPrefix(192, 168, 1, 50, 31, 302);         // 192.168.1.50/31 -> tag 302 (.50, .51)
    engine.addPrefix(192, 168, 1, 51, 32, 303);         // 192.168.1.51/32 -> tag 303
    engine.addPrefix(127, 0, 0, 0, 8, 400);             // 127.0.0.0/8 -> tag 400
    engine.addPrefix(255, 255, 255, 255, 32, 500);      // 255.255.255.255/32 -> tag 500

    // 2. Generate 1,000 synthetic prefixes in 200.0.0.0/8 (0xC8000000)
    std::mt19937 rng(1337 + trial);
    int extra_routes = 0;
    for (int i = 0; i < 1000; ++i) {
        uint32_t net = 0xC8000000 | (rng() & 0x00FFFFFF);
        uint8_t plen = static_cast<uint8_t>(12 + (rng() % 17)); // /12 to /28
        uint16_t tag = static_cast<uint16_t>(1000 + i);
        if (engine.addPrefix(net, plen, tag)) {
            extra_routes++;
        }
    }

    std::vector<QueryItem> query_corpus = {
        { throne::fast::FastRouteEngine::makeIp(10, 1, 2, 3), 103 },
        { throne::fast::FastRouteEngine::makeIp(10, 1, 2, 4), 102 },
        { throne::fast::FastRouteEngine::makeIp(10, 1, 5, 1), 101 },
        { throne::fast::FastRouteEngine::makeIp(10, 2, 0, 1), 100 },
        { throne::fast::FastRouteEngine::makeIp(172, 16, 1, 1), 200 },
        { throne::fast::FastRouteEngine::makeIp(172, 31, 255, 254), 200 },
        { throne::fast::FastRouteEngine::makeIp(192, 168, 0, 1), 300 },
        { throne::fast::FastRouteEngine::makeIp(192, 168, 1, 1), 301 },
        { throne::fast::FastRouteEngine::makeIp(192, 168, 1, 50), 302 },
        { throne::fast::FastRouteEngine::makeIp(192, 168, 1, 51), 303 },
        { throne::fast::FastRouteEngine::makeIp(127, 0, 0, 1), 400 },
        { throne::fast::FastRouteEngine::makeIp(255, 255, 255, 255), 500 },
        { throne::fast::FastRouteEngine::makeIp(1, 1, 1, 1), 1 }, // default route
        { throne::fast::FastRouteEngine::makeIp(8, 8, 8, 8), 1 }, // default route
    };

    for (const auto& item : query_corpus) {
        int res = engine.match(item.ip);
        if (res != item.expected_tag) {
            std::cerr << "[FAIL] Pre-verification error for IP 0x" << std::hex << item.ip
                      << ": expected " << std::dec << item.expected_tag << ", got " << res << "\n";
            return false;
        }
    }

    const uint64_t total_expected = num_threads * queries_per_thread;
    size_t mem_before = getMemoryUsage();

    std::atomic<bool> start_gate{false};
    std::atomic<uint32_t> ready_count{0};
    std::atomic<uint64_t> total_completed_queries{0};
    std::atomic<uint64_t> total_mismatches{0};
    std::atomic<uint64_t> total_crashes{0};

    std::vector<std::thread> workers;
    workers.reserve(num_threads);

    for (unsigned int tid = 0; tid < num_threads; ++tid) {
        workers.emplace_back([&, tid]() {
            ready_count.fetch_add(1, std::memory_order_release);
            while (!start_gate.load(std::memory_order_acquire)) {
                std::this_thread::yield();
            }

            uint64_t local_completed = 0;
            uint64_t local_errors = 0;
            const size_t corpus_size = query_corpus.size();

            try {
                for (uint64_t q = 0; q < queries_per_thread; ++q) {
                    const auto& item = query_corpus[(tid + q) % corpus_size];
                    int actual_tag = engine.match(item.ip);
                    if (actual_tag != item.expected_tag) {
                        local_errors++;
                    }
                    local_completed++;
                }
            } catch (...) {
                total_crashes.fetch_add(1, std::memory_order_relaxed);
            }

            total_completed_queries.fetch_add(local_completed, std::memory_order_relaxed);
            total_mismatches.fetch_add(local_errors, std::memory_order_relaxed);
        });
    }

    while (ready_count.load(std::memory_order_acquire) < num_threads) {
        std::this_thread::yield();
    }

    auto t_start = std::chrono::high_resolution_clock::now();
    start_gate.store(true, std::memory_order_release);

    for (auto& w : workers) {
        w.join();
    }
    auto t_end = std::chrono::high_resolution_clock::now();

    double elapsed_sec = std::chrono::duration<double>(t_end - t_start).count();
    size_t mem_after = getMemoryUsage();
    int64_t mem_delta_kb = (static_cast<int64_t>(mem_after) - static_cast<int64_t>(mem_before)) / 1024;

    double qps = static_cast<double>(total_completed_queries.load()) / elapsed_sec;
    double m_qps = qps / 1000000.0;
    double avg_latency_ns = (elapsed_sec / static_cast<double>(total_completed_queries.load())) * 1e9;

    std::cout << "  Elapsed Wall Time:   " << elapsed_sec << " s\n";
    std::cout << "  Queries Executed:    " << total_completed_queries.load() << " / " << total_expected << "\n";
    std::cout << "  Races / Mismatches:  " << total_mismatches.load() << "\n";
    std::cout << "  Crashes/Exceptions:  " << total_crashes.load() << "\n";
    std::cout << "  Aggregate QPS:       " << m_qps << " Million queries/sec\n";
    std::cout << "  Average Latency:     " << avg_latency_ns << " ns/query\n";
    std::cout << "  Memory Delta:        " << mem_delta_kb << " KB\n";

    return (total_mismatches.load() == 0 && total_crashes.load() == 0 && 
            total_completed_queries.load() == total_expected);
}

// Test Suite 2: Multi-Threaded FNV-1a Hash Concurrency Stress
bool run_hash_concurrency_stress(unsigned int num_threads, uint64_t hashes_per_thread) {
    std::cout << "\n----------------------------------------------------------------------\n";
    std::cout << "8-THREAD FNV-1a HASH CONCURRENCY STRESS (" 
              << (num_threads * hashes_per_thread) << " HASH OPERATIONS)\n";
    std::cout << "----------------------------------------------------------------------\n";

    const char* sample_str = "throne-concurrency-test-hash-string-sample-for-benchmark";
    const size_t sample_len = strlen(sample_str);
    const uint64_t expected_hash = throne_fast_hash(reinterpret_cast<const uint8_t*>(sample_str), sample_len);

    std::atomic<bool> start_gate{false};
    std::atomic<uint32_t> ready_count{0};
    std::atomic<uint64_t> total_completed{0};
    std::atomic<uint64_t> total_hash_mismatches{0};

    std::vector<std::thread> workers;
    workers.reserve(num_threads);

    for (unsigned int tid = 0; tid < num_threads; ++tid) {
        workers.emplace_back([&]() {
            ready_count.fetch_add(1, std::memory_order_release);
            while (!start_gate.load(std::memory_order_acquire)) {
                std::this_thread::yield();
            }

            uint64_t local_completed = 0;
            uint64_t local_err = 0;
            for (uint64_t i = 0; i < hashes_per_thread; ++i) {
                uint64_t h = throne_fast_hash(reinterpret_cast<const uint8_t*>(sample_str), sample_len);
                if (h != expected_hash) {
                    local_err++;
                }
                local_completed++;
            }
            total_completed.fetch_add(local_completed, std::memory_order_relaxed);
            total_hash_mismatches.fetch_add(local_err, std::memory_order_relaxed);
        });
    }

    while (ready_count.load(std::memory_order_acquire) < num_threads) {
        std::this_thread::yield();
    }

    auto t_start = std::chrono::high_resolution_clock::now();
    start_gate.store(true, std::memory_order_release);

    for (auto& w : workers) {
        w.join();
    }
    auto t_end = std::chrono::high_resolution_clock::now();

    double elapsed_sec = std::chrono::duration<double>(t_end - t_start).count();
    double hps = static_cast<double>(total_completed.load()) / elapsed_sec;
    double m_hps = hps / 1000000.0;

    std::cout << "  Hash Wall Time:      " << elapsed_sec << " s\n";
    std::cout << "  Completed Hashes:    " << total_completed.load() << "\n";
    std::cout << "  Hash Mismatches:     " << total_hash_mismatches.load() << "\n";
    std::cout << "  Hash Throughput:     " << m_hps << " Million hashes/sec\n";

    return (total_hash_mismatches.load() == 0 && total_completed.load() == (num_threads * hashes_per_thread));
}

int main() {
    std::cout << "======================================================================\n";
    std::cout << "THRONE-FAST MULTI-THREADED CONCURRENCY & HASH STRESS TEST (C++20)\n";
    std::cout << "======================================================================\n";

    const char* version = throne_fast_version();
    std::cout << "Target Engine Version: " << (version ? version : "NULL") << "\n";
    assert(version != nullptr);

    // Run 3 trials of 8-thread read concurrency: 8 threads x 500,000 queries = 4,000,000 queries per trial!
    const unsigned int NUM_THREADS = 8;
    const uint64_t QUERIES_PER_THREAD = 500000; // 500,000 queries per thread
    
    bool all_passed = true;
    for (int trial = 1; trial <= 3; ++trial) {
        if (!run_route_concurrency_stress(trial, NUM_THREADS, QUERIES_PER_THREAD)) {
            all_passed = false;
        }
    }

    // Run 8-thread concurrent FNV-1a hashing: 8 threads x 1,000,000 hashes = 8,000,000 hashes!
    if (!run_hash_concurrency_stress(NUM_THREADS, 1000000)) {
        all_passed = false;
    }

    std::cout << "\n======================================================================\n";
    if (all_passed) {
        std::cout << "ALL NATIVE C++20 CONCURRENCY STRESS SUITES PASSED (0 RACES, 0 CRASHES)!\n";
        std::cout << "======================================================================\n";
        return 0;
    } else {
        std::cerr << "FAILURES DETECTED IN C++20 CONCURRENCY SUITES!\n";
        std::cout << "======================================================================\n";
        return 1;
    }
}
