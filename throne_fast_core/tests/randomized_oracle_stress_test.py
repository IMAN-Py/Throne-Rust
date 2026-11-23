#!/usr/bin/env python3
"""
Randomized Differential Oracle & Concurrency Stress Test for throne_fast_core
Validates:
1. Randomized LPM route generator against an independent Python reference oracle across 5,000 queries.
2. Concurrent multi-threaded read stress testing across 8 threads.
"""

import sys
import os
import random
import threading
import ctypes

DLL_PATH = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "target", "release", "throne_fast_core.dll"))

class PythonLpmOracle:
    """Independent ground-truth oracle for Longest Prefix Matching."""
    def __init__(self):
        self.routes = {} # (network, prefix_len) -> tag

    def add_prefix(self, ip: int, prefix_len: int, tag: int):
        prefix_len = min(prefix_len, 32)
        mask = ((1 << 32) - 1) ^ ((1 << (32 - prefix_len)) - 1) if prefix_len < 32 else 0xffffffff
        if prefix_len == 0:
            mask = 0
        network = ip & mask
        self.routes[(network, prefix_len)] = tag

    def match_ip(self, ip: int) -> int:
        best_len = -1
        best_tag = -1
        for (network, prefix_len), tag in self.routes.items():
            mask = ((1 << 32) - 1) ^ ((1 << (32 - prefix_len)) - 1) if prefix_len < 32 else 0xffffffff
            if prefix_len == 0:
                mask = 0
            if (ip & mask) == network:
                if prefix_len > best_len:
                    best_len = prefix_len
                    best_tag = tag
        return best_tag

def run_differential_oracle_test(dll, num_prefixes=500, num_queries=5000):
    print("=" * 70)
    print(f"DIFFERENTIAL ORACLE TEST: {num_prefixes} Random Prefixes vs {num_queries} Queries")
    print("=" * 70)
    
    random.seed(0x1337_2026)
    oracle = PythonLpmOracle()
    engine = dll.throne_fast_route_create()
    assert engine
    
    generated_prefixes = []
    # Always include boundary prefixes
    boundary_prefixes = [
        (0x00000000, 0, 1),
        (0x00000000, 32, 2),
        (0xffffffff, 32, 3),
        (0x80000000, 1, 4),
        (0x7fffffff, 31, 5),
    ]
    for ip, plen, tag in boundary_prefixes:
        oracle.add_prefix(ip, plen, tag)
        dll.throne_fast_route_add(engine, ip, plen, tag)
        generated_prefixes.append((ip, plen, tag))

    for i in range(num_prefixes - len(boundary_prefixes)):
        ip = random.getrandbits(32)
        plen = random.randint(0, 32)
        tag = random.randint(1, 65535)
        # Mask out host bits in network insertion to test canonical and non-canonical IP inputs
        oracle.add_prefix(ip, plen, tag)
        dll.throne_fast_route_add(engine, ip, plen, tag)
        generated_prefixes.append((ip, plen, tag))

    print(f"Generated and populated {len(generated_prefixes)} prefixes into both engines.")

    # Generate query IPs
    query_ips = [
        0x00000000, 0xffffffff, 0x00000001, 0xfffffffe,
        0x7fffffff, 0x80000000, 0x0a010203, 0xc0a80101
    ]
    # Add random IPs
    for _ in range(num_queries - len(query_ips)):
        query_ips.append(random.getrandbits(32))

    mismatches = 0
    for idx, ip in enumerate(query_ips):
        expected = oracle.match_ip(ip)
        actual = dll.throne_fast_route_match(engine, ip)
        if expected != actual:
            print(f"[MISMATCH #{mismatches+1}] IP=0x{ip:08x}: Oracle={expected}, Rust={actual}")
            mismatches += 1
            if mismatches >= 10:
                print("Too many mismatches, aborting early.")
                break

    if mismatches == 0:
        print(f"[PASS] 100% Differential Match! All {len(query_ips)} queries matched Oracle identically.")
    else:
        print(f"[FAIL] Found {mismatches} mismatches!")
        dll.throne_fast_route_free(engine)
        return False

    # Concurrency test: 8 threads reading simultaneously
    print("\nStarting concurrent multi-threaded read test (8 threads, 50,000 reads each)...")
    errors = []
    def worker_thread(tid):
        try:
            for _ in range(50000):
                qip = query_ips[random.randint(0, len(query_ips) - 1)]
                res = dll.throne_fast_route_match(engine, qip)
                if res < 0:
                    errors.append(f"Thread {tid} got unexpected negative match")
        except Exception as e:
            errors.append(f"Thread {tid} exception: {e}")

    threads = [threading.Thread(target=worker_thread, args=(i,)) for i in range(8)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()

    if errors:
        print(f"[FAIL] Concurrent read test failed with {len(errors)} errors: {errors[:3]}")
        dll.throne_fast_route_free(engine)
        return False

    print("[PASS] Multi-threaded concurrent read stress test passed (400,000 total reads, 0 errors).")
    dll.throne_fast_route_free(engine)
    return True

def main():
    dll = ctypes.CDLL(DLL_PATH)
    dll.throne_fast_route_create.restype = ctypes.c_void_p
    dll.throne_fast_route_free.argtypes = [ctypes.c_void_p]
    dll.throne_fast_route_add.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.c_uint8, ctypes.c_uint16]
    dll.throne_fast_route_add.restype = ctypes.c_int
    dll.throne_fast_route_match.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
    dll.throne_fast_route_match.restype = ctypes.c_int

    success = run_differential_oracle_test(dll)
    return 0 if success else 1

if __name__ == "__main__":
    sys.exit(main())
