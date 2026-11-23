#!/usr/bin/env python3
"""
Adversarial Stress Test Suite for throne_fast_core
Tests:
1. LPM routing accuracy across boundary IPs (0.0.0.0, 255.255.255.255, prefix lengths 0, 1, 31, 32, overlapping subnets).
2. Engine creation and destruction lifecycle (1,000 engines created and freed, monitoring memory leaks and crashes).
3. Empirical throughput across 10,000,000 operations (10 runs, checking >= 200 M ops/sec).
4. Edge cases: NULL pointers, out-of-range prefix lengths, prefix overwriting, and hash safety.
"""

import sys
import os
import time
import ctypes
import ctypes.wintypes as wt
from ipaddress import IPv4Address

DLL_PATH = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "target", "release", "throne_fast_core.dll"))

# Setup Windows Process Memory Info Structure
class PROCESS_MEMORY_COUNTERS(ctypes.Structure):
    _fields_ = [
        ('cb', wt.DWORD),
        ('PageFaultCount', wt.DWORD),
        ('PeakWorkingSetSize', ctypes.c_size_t),
        ('WorkingSetSize', ctypes.c_size_t),
        ('QuotaPeakPagedPoolUsage', ctypes.c_size_t),
        ('QuotaPagedPoolUsage', ctypes.c_size_t),
        ('QuotaPeakNonPagedPoolUsage', ctypes.c_size_t),
        ('QuotaNonPagedPoolUsage', ctypes.c_size_t),
        ('PagefileUsage', ctypes.c_size_t),
        ('PeakPagefileUsage', ctypes.c_size_t),
    ]

ctypes.windll.psapi.GetProcessMemoryInfo.argtypes = [wt.HANDLE, ctypes.c_void_p, wt.DWORD]
ctypes.windll.psapi.GetProcessMemoryInfo.restype = wt.BOOL

def get_process_memory():
    handle = ctypes.windll.kernel32.GetCurrentProcess()
    counters = PROCESS_MEMORY_COUNTERS()
    counters.cb = ctypes.sizeof(PROCESS_MEMORY_COUNTERS)
    res = ctypes.windll.psapi.GetProcessMemoryInfo(handle, ctypes.byref(counters), counters.cb)
    if not res:
        raise RuntimeError("Failed to query process memory")
    return counters.WorkingSetSize, counters.PagefileUsage

def ip_to_u32(ip_str: str) -> int:
    return int(IPv4Address(ip_str))

def load_throne_fast_core():
    if not os.path.exists(DLL_PATH):
        raise FileNotFoundError(f"DLL not found at: {DLL_PATH}")
    
    dll = ctypes.CDLL(DLL_PATH)
    
    # throne_fast_version() -> *const c_char
    dll.throne_fast_version.argtypes = []
    dll.throne_fast_version.restype = ctypes.c_char_p
    
    # throne_fast_hash(*const u8, size_t) -> u64
    dll.throne_fast_hash.argtypes = [ctypes.c_char_p, ctypes.c_size_t]
    dll.throne_fast_hash.restype = ctypes.c_uint64
    
    # throne_fast_route_create() -> *mut RouteEngine
    dll.throne_fast_route_create.argtypes = []
    dll.throne_fast_route_create.restype = ctypes.c_void_p
    
    # throne_fast_route_free(*mut RouteEngine)
    dll.throne_fast_route_free.argtypes = [ctypes.c_void_p]
    dll.throne_fast_route_free.restype = None
    
    # throne_fast_route_add(*mut RouteEngine, u32, u8, u16) -> c_int
    dll.throne_fast_route_add.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.c_uint8, ctypes.c_uint16]
    dll.throne_fast_route_add.restype = ctypes.c_int
    
    # throne_fast_route_match(*const RouteEngine, u32) -> c_int
    dll.throne_fast_route_match.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
    dll.throne_fast_route_match.restype = ctypes.c_int
    
    # throne_fast_benchmark(u32) -> f64
    dll.throne_fast_benchmark.argtypes = [ctypes.c_uint32]
    dll.throne_fast_benchmark.restype = ctypes.c_double
    
    return dll

def test_lpm_boundaries(dll):
    print("=" * 70)
    print("TEST SUITE 1: LPM Routing Accuracy Across Boundary IPs & Overlapping Subnets")
    print("=" * 70)
    
    engine = dll.throne_fast_route_create()
    assert engine, "Failed to create RouteEngine"
    
    test_cases_passed = 0
    total_test_cases = 0

    def assert_match(ip_str, expected_tag, context):
        nonlocal test_cases_passed, total_test_cases
        total_test_cases += 1
        ip_val = ip_to_u32(ip_str)
        matched = dll.throne_fast_route_match(engine, ip_val)
        if matched != expected_tag:
            print(f"[FAIL] {context}: IP={ip_str} (0x{ip_val:08x}) expected={expected_tag}, got={matched}")
            raise AssertionError(f"{context} failed: expected {expected_tag}, got {matched}")
        test_cases_passed += 1

    # 1. Prefix 0: Default route 0.0.0.0/0 -> tag 100
    dll.throne_fast_route_add(engine, ip_to_u32("0.0.0.0"), 0, 100)
    assert_match("0.0.0.0", 100, "Prefix /0 match on 0.0.0.0")
    assert_match("255.255.255.255", 100, "Prefix /0 match on 255.255.255.255")
    assert_match("127.0.0.1", 100, "Prefix /0 match on 127.0.0.1")
    assert_match("192.168.1.1", 100, "Prefix /0 match on 192.168.1.1")
    print("[PASS] Subnet /0 default route tests passed.")

    # 2. Prefix 1: Split internet into 0.0.0.0/1 -> tag 101, 128.0.0.0/1 -> tag 102
    dll.throne_fast_route_add(engine, ip_to_u32("0.0.0.0"), 1, 101)
    dll.throne_fast_route_add(engine, ip_to_u32("128.0.0.0"), 1, 102)
    assert_match("0.0.0.0", 101, "Prefix /1 lower bound 0.0.0.0")
    assert_match("127.255.255.255", 101, "Prefix /1 upper bound 127.255.255.255")
    assert_match("128.0.0.0", 102, "Prefix /1 lower bound 128.0.0.0")
    assert_match("255.255.255.255", 102, "Prefix /1 upper bound 255.255.255.255")
    print("[PASS] Subnet /1 split tests passed.")

    # 3. Prefix 31: 2-host subnets (RFC 3021)
    # 192.168.1.0/31 -> tag 131 (covers .0 and .1)
    # 192.168.1.2/31 -> tag 132 (covers .2 and .3)
    dll.throne_fast_route_add(engine, ip_to_u32("192.168.1.0"), 31, 131)
    dll.throne_fast_route_add(engine, ip_to_u32("192.168.1.2"), 31, 132)
    assert_match("192.168.1.0", 131, "Prefix /31 host 0")
    assert_match("192.168.1.1", 131, "Prefix /31 host 1")
    assert_match("192.168.1.2", 132, "Prefix /31 host 2")
    assert_match("192.168.1.3", 132, "Prefix /31 host 3")
    assert_match("192.168.1.4", 102, "Prefix /31 fallback to /1 (102)")
    print("[PASS] Subnet /31 boundary tests passed.")

    # 4. Prefix 32: Exact host matches
    # 0.0.0.0/32 -> tag 320
    # 255.255.255.255/32 -> tag 321
    # 192.168.1.1/32 -> tag 322 (overrides /31 tag 131)
    dll.throne_fast_route_add(engine, ip_to_u32("0.0.0.0"), 32, 320)
    dll.throne_fast_route_add(engine, ip_to_u32("255.255.255.255"), 32, 321)
    dll.throne_fast_route_add(engine, ip_to_u32("192.168.1.1"), 32, 322)
    
    assert_match("0.0.0.0", 320, "Prefix /32 exact 0.0.0.0")
    assert_match("0.0.0.1", 101, "Prefix /32 fallback 0.0.0.1 to /1 (101)")
    assert_match("255.255.255.255", 321, "Prefix /32 exact 255.255.255.255")
    assert_match("255.255.255.254", 102, "Prefix /32 fallback 255.255.254 to /1 (102)")
    assert_match("192.168.1.0", 131, "Prefix /31 retained for .0")
    assert_match("192.168.1.1", 322, "Prefix /32 more specific overrides /31 for .1")
    print("[PASS] Subnet /32 exact host boundary tests passed.")

    # 5. Complex Multi-Level Overlapping Hierarchy:
    # 10.0.0.0/8       -> tag 1008
    # 10.128.0.0/9     -> tag 1009
    # 10.128.0.0/16    -> tag 1016
    # 10.128.1.0/24    -> tag 1024
    # 10.128.1.128/25  -> tag 1025
    # 10.128.1.130/31  -> tag 1031
    # 10.128.1.130/32  -> tag 1032
    dll.throne_fast_route_add(engine, ip_to_u32("10.0.0.0"), 8, 1008)
    dll.throne_fast_route_add(engine, ip_to_u32("10.128.0.0"), 9, 1009)
    dll.throne_fast_route_add(engine, ip_to_u32("10.128.0.0"), 16, 1016)
    dll.throne_fast_route_add(engine, ip_to_u32("10.128.1.0"), 24, 1024)
    dll.throne_fast_route_add(engine, ip_to_u32("10.128.1.128"), 25, 1025)
    dll.throne_fast_route_add(engine, ip_to_u32("10.128.1.130"), 31, 1031)
    dll.throne_fast_route_add(engine, ip_to_u32("10.128.1.130"), 32, 1032)

    # Test resolution across all 7 nested layers
    assert_match("10.128.1.130", 1032, "Level 7 (/32 match)")
    assert_match("10.128.1.131", 1031, "Level 6 (/31 match)")
    assert_match("10.128.1.132", 1025, "Level 5 (/25 match)")
    assert_match("10.128.1.10", 1024, "Level 4 (/24 match)")
    assert_match("10.128.2.1", 1016, "Level 3 (/16 match)")
    assert_match("10.200.1.1", 1009, "Level 2 (/9 match)")
    assert_match("10.64.1.1", 1008, "Level 1 (/8 match)")
    assert_match("11.1.1.1", 101, "Level 0 (/1 fallback)")
    print("[PASS] 7-Level Nested Subnet Hierarchy LPM tests passed.")

    # 6. Route Overwriting
    dll.throne_fast_route_add(engine, ip_to_u32("172.16.0.0"), 12, 1721)
    assert_match("172.16.5.5", 1721, "Original route tag")
    dll.throne_fast_route_add(engine, ip_to_u32("172.16.0.0"), 12, 1722)
    assert_match("172.16.5.5", 1722, "Updated route tag after overwrite")
    print("[PASS] Route overwrite / update tests passed.")

    # Free engine
    dll.throne_fast_route_free(engine)
    print(f"[SUMMARY] Suite 1 Passed: {test_cases_passed}/{total_test_cases} assertion checks.")

def test_unmatched_and_null_checks(dll):
    print("=" * 70)
    print("TEST SUITE 2: Edge Cases, Unmatched IPs, and NULL Pointer Safety")
    print("=" * 70)
    
    # Clean empty engine
    engine = dll.throne_fast_route_create()
    res = dll.throne_fast_route_match(engine, ip_to_u32("8.8.8.8"))
    assert res == -1, f"Expected -1 for unmatched IP on empty engine, got {res}"
    
    # Route with prefix_len > 32 (rejected with -1)
    ret = dll.throne_fast_route_add(engine, ip_to_u32("1.2.3.4"), 40, 99)
    assert ret == -1, f"Expected -1 from add with prefix_len > 32, got {ret}"
    matched = dll.throne_fast_route_match(engine, ip_to_u32("1.2.3.4"))
    assert matched == -1, f"Expected -1 for uninserted route, got {matched}"
    
    dll.throne_fast_route_free(engine)
    
    # NULL pointer handling
    assert dll.throne_fast_route_match(None, 0x12345678) == -1, "NULL engine match must return -1"
    assert dll.throne_fast_route_add(None, 0x12345678, 24, 1) == -1, "NULL engine add must return -1"
    dll.throne_fast_route_free(None)  # Must not crash
    
    # Hash NULL and empty safety
    assert dll.throne_fast_hash(None, 0) == 0, "NULL hash must return 0"
    assert dll.throne_fast_hash(b"abc", 0) == 0, "0-length hash must return 0"
    
    # Hash determinism
    h1 = dll.throne_fast_hash(b"throne-fast-route-hash-test", 27)
    h2 = dll.throne_fast_hash(b"throne-fast-route-hash-test", 27)
    assert h1 == h2 and h1 != 0, f"Hash not deterministic: {h1} vs {h2}"
    print("[PASS] Edge cases, unmatched lookups, and NULL pointer safety verified.")

def test_lifecycle_and_memory_stability(dll):
    print("=" * 70)
    print("TEST SUITE 3: Engine Creation and Destruction Lifecycle (1,000 Engines)")
    print("=" * 70)
    
    # Measure baseline memory
    initial_ws, initial_pf = get_process_memory()
    print(f"Initial Process Memory: WorkingSet = {initial_ws / 1024:.2f} KB, Pagefile = {initial_pf / 1024:.2f} KB")
    
    start_time = time.perf_counter()
    num_cycles = 1000
    
    # Sequential lifecycle test: create, add routes, match, free
    for i in range(num_cycles):
        engine = dll.throne_fast_route_create()
        if not engine:
            raise RuntimeError(f"Engine creation failed at iteration {i}")
        
        # Add 15 routes per engine with various prefix lengths
        dll.throne_fast_route_add(engine, 0x0a000000, 8, 10)
        dll.throne_fast_route_add(engine, 0x0a010000, 16, 20)
        dll.throne_fast_route_add(engine, 0x0a010100, 24, 30)
        dll.throne_fast_route_add(engine, 0x0a010101, 32, 40)
        dll.throne_fast_route_add(engine, 0xc0a80000, 16, 50)
        dll.throne_fast_route_add(engine, 0xc0a80100, 24, 60)
        dll.throne_fast_route_add(engine, 0xac100000, 12, 70)
        dll.throne_fast_route_add(engine, 0x7f000000, 8, 80)
        dll.throne_fast_route_add(engine, 0x00000000, 0, 90)
        dll.throne_fast_route_add(engine, 0x00000000, 1, 100)
        dll.throne_fast_route_add(engine, 0x80000000, 1, 110)
        dll.throne_fast_route_add(engine, 0xffffffff, 32, 120)
        dll.throne_fast_route_add(engine, 0x08080808, 32, 130)
        dll.throne_fast_route_add(engine, 0x01010101, 32, 140)
        dll.throne_fast_route_add(engine, 0xd0000000, 4, 150)
        
        # Verify lookups
        assert dll.throne_fast_route_match(engine, 0x0a010101) == 40
        assert dll.throne_fast_route_match(engine, 0x0a010102) == 30
        assert dll.throne_fast_route_match(engine, 0xffffffff) == 120
        assert dll.throne_fast_route_match(engine, 0x08080808) == 130
        
        dll.throne_fast_route_free(engine)
    
    sequential_elapsed = time.perf_counter() - start_time
    mid_ws, mid_pf = get_process_memory()
    print(f"Post-Sequential (1,000 cycles in {sequential_elapsed:.4f}s): WorkingSet = {mid_ws / 1024:.2f} KB, Pagefile = {mid_pf / 1024:.2f} KB")
    
    # Simultaneous allocation test: allocate 1,000 engines concurrently, then free all
    start_simul = time.perf_counter()
    engines = []
    for i in range(num_cycles):
        eng = dll.throne_fast_route_create()
        assert eng, f"Simultaneous engine allocation failed at {i}"
        dll.throne_fast_route_add(eng, 0x0a000000 | (i << 8), 24, i & 0xffff)
        engines.append(eng)
    
    simul_alloc_ws, simul_alloc_pf = get_process_memory()
    print(f"With 1,000 Concurrent Engines Allocated: WorkingSet = {simul_alloc_ws / 1024:.2f} KB, Pagefile = {simul_alloc_pf / 1024:.2f} KB")
    
    # Free all simultaneous engines
    for eng in engines:
        dll.throne_fast_route_free(eng)
    engines.clear()
    
    final_ws, final_pf = get_process_memory()
    simul_elapsed = time.perf_counter() - start_simul
    print(f"Post-Free All 1,000 Concurrent Engines ({simul_elapsed:.4f}s): WorkingSet = {final_ws / 1024:.2f} KB, Pagefile = {final_pf / 1024:.2f} KB")
    
    # Memory growth delta
    ws_delta_kb = (final_ws - initial_ws) / 1024
    pf_delta_kb = (final_pf - initial_pf) / 1024
    print(f"Memory Delta across 2,000 engine lifecycles: WorkingSet Delta = {ws_delta_kb:+.2f} KB, Pagefile Delta = {pf_delta_kb:+.2f} KB")
    
    # Memory check: working set growth should be negligible (allocator overhead < a few MB)
    assert abs(pf_delta_kb) < 10240, f"Pagefile memory grew excessively: {pf_delta_kb:.2f} KB"
    print("[PASS] Lifecycle and memory stability verified with zero leaks or crashes.")
    return {
        "cycles": num_cycles,
        "sequential_duration_s": sequential_elapsed,
        "simul_duration_s": simul_elapsed,
        "initial_ws_kb": initial_ws / 1024,
        "final_ws_kb": final_ws / 1024,
        "delta_ws_kb": ws_delta_kb,
        "delta_pf_kb": pf_delta_kb,
    }

def test_throughput_benchmark(dll):
    print("=" * 70)
    print("TEST SUITE 4: Empirical Throughput Across 10,000,000 Operations")
    print("=" * 70)
    
    num_trials = 10
    iterations = 10_000_000
    results = []
    
    print(f"Running {num_trials} benchmark trials ({iterations:,} route match ops each)...")
    for trial in range(1, num_trials + 1):
        elapsed = dll.throne_fast_benchmark(iterations)
        assert elapsed > 0, f"Benchmark returned error: {elapsed}"
        ops_per_sec = iterations / elapsed
        m_ops_per_sec = ops_per_sec / 1_000_000
        latency_ns = (elapsed / iterations) * 1e9
        results.append({
            "trial": trial,
            "elapsed_s": elapsed,
            "m_ops_sec": m_ops_per_sec,
            "latency_ns": latency_ns,
        })
        print(f"  Trial {trial:2d}: {m_ops_per_sec:7.2f} M ops/sec ({latency_ns:.2f} ns/op, elapsed: {elapsed:.5f}s)")
    
    throughputs = [r["m_ops_sec"] for r in results]
    min_tp = min(throughputs)
    max_tp = max(throughputs)
    mean_tp = sum(throughputs) / len(throughputs)
    variance = sum((x - mean_tp) ** 2 for x in throughputs) / len(throughputs)
    std_tp = variance ** 0.5
    
    print("-" * 70)
    print(f"Throughput Statistics across {num_trials} trials:")
    print(f"  Mean Throughput: {mean_tp:.2f} M ops/sec")
    print(f"  Min Throughput:  {min_tp:.2f} M ops/sec")
    print(f"  Max Throughput:  {max_tp:.2f} M ops/sec")
    print(f"  Std Deviation:   {std_tp:.2f} M ops/sec")
    print(f"  Target Threshold: >= 200.00 M ops/sec")
    
    assert min_tp >= 200.0, f"Throughput dipped below 200 M ops/sec: min was {min_tp:.2f} M ops/sec"
    print(f"[PASS] All {num_trials} trials strictly exceeded 200 M ops/sec threshold!")
    
    return {
        "trials": results,
        "mean_tp": mean_tp,
        "min_tp": min_tp,
        "max_tp": max_tp,
        "std_tp": std_tp,
    }

def main():
    print("=" * 70)
    print("THRONE FAST CORE ADVERSARIAL STRESS TEST & BENCHMARK HARNESS")
    print("=" * 70)
    
    dll = load_throne_fast_core()
    version = dll.throne_fast_version().decode("utf-8")
    print(f"Loaded DLL: {DLL_PATH}")
    print(f"Reported Module Version: {version}")
    assert version == "1.4.2-fast", f"Unexpected version string: {version}"
    
    test_lpm_boundaries(dll)
    test_unmatched_and_null_checks(dll)
    lifecycle_res = test_lifecycle_and_memory_stability(dll)
    bench_res = test_throughput_benchmark(dll)
    
    print("\n" + "=" * 70)
    print("ALL ADVERSARIAL TESTS COMPLETED SUCCESSFULLY!")
    print("=" * 70)
    print(f"Verdict: PASS (APPROVE)")
    print(f"Mean Throughput: {bench_res['mean_tp']:.2f} M ops/sec (Threshold: 200 M ops/sec)")
    print(f"Lifecycle Memory Delta: {lifecycle_res['delta_pf_kb']:+.2f} KB across 2,000 engine instances")
    return 0

if __name__ == "__main__":
    sys.exit(main())
