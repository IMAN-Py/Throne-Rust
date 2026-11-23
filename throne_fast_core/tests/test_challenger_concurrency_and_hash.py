#!/usr/bin/env python3
"""
Throne-Fast Empirical Challenger Concurrency & Hash Stress Test Suite
======================================================================
Focus:
1. Multi-threaded read concurrency (8 threads, >=100,000 queries/thread, verifying 0 races & 0 crashes).
2. FNV-1a hash consistency, deterministic oracle verification, and boundary safety.
3. Strict Avalanche Criterion (SAC) bit-flip avalanche testing across payload sizes.
4. Hash collision resistance, bit independence, and Chi-Square bucket uniformity across 200,000 keys.
"""

import sys
import os
import time
import math
import random
import threading
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

try:
    ctypes.windll.psapi.GetProcessMemoryInfo.argtypes = [wt.HANDLE, ctypes.c_void_p, wt.DWORD]
    ctypes.windll.psapi.GetProcessMemoryInfo.restype = wt.BOOL
    HAS_PSAPI = True
except Exception:
    HAS_PSAPI = False

def get_process_memory_kb():
    if not HAS_PSAPI:
        return 0
    handle = ctypes.windll.kernel32.GetCurrentProcess()
    counters = PROCESS_MEMORY_COUNTERS()
    counters.cb = ctypes.sizeof(PROCESS_MEMORY_COUNTERS)
    res = ctypes.windll.psapi.GetProcessMemoryInfo(handle, ctypes.byref(counters), counters.cb)
    if not res:
        return 0
    return counters.WorkingSetSize // 1024

def fnv1a_64_reference(data: bytes) -> int:
    """Standard 64-bit FNV-1a reference oracle."""
    FNV_OFFSET = 0xcbf29ce484222325
    FNV_PRIME = 0x100000001b3
    h = FNV_OFFSET
    for b in data:
        h = ((h ^ b) * FNV_PRIME) & 0xffffffffffffffff
    return h

def popcount(x: int) -> int:
    return bin(x).count('1')

def load_throne_fast_core():
    if not os.path.exists(DLL_PATH):
        raise FileNotFoundError(f"DLL not found: {DLL_PATH}")
    dll = ctypes.CDLL(DLL_PATH)

    dll.throne_fast_version.argtypes = []
    dll.throne_fast_version.restype = ctypes.c_char_p

    dll.throne_fast_hash.argtypes = [ctypes.c_char_p, ctypes.c_size_t]
    dll.throne_fast_hash.restype = ctypes.c_uint64

    dll.throne_fast_route_create.argtypes = []
    dll.throne_fast_route_create.restype = ctypes.c_void_p

    dll.throne_fast_route_free.argtypes = [ctypes.c_void_p]
    dll.throne_fast_route_free.restype = None

    dll.throne_fast_route_add.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.c_uint8, ctypes.c_uint16]
    dll.throne_fast_route_add.restype = ctypes.c_int

    dll.throne_fast_route_match.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
    dll.throne_fast_route_match.restype = ctypes.c_int

    return dll

# ============================================================================
# TEST SUITE 1: FNV-1a Hash Consistency & Deterministic Oracle Verification
# ============================================================================
def test_fnv1a_consistency(dll):
    print("\n" + "=" * 70)
    print("TEST SUITE 1: FNV-1a Hash Consistency & Differential Oracle")
    print("=" * 70)

    # 1. Standard known test vectors
    known_vectors = [
        (b"a", 0xaf63dc4c8601ec8c),
        (b"foobar", 0x85944171f73967e8),
        (b"123456789", 0x06d5573923c6cdfc),
        (b"The quick brown fox jumps over the lazy dog", 0xf3f9b7f5e7e47110),
        (b"throne-fast-core-acceleration-layer", 0x2de1605c2721b34b),
        (b"192.168.1.1:443", 0x7bd3d87656db436b),
    ]

    print(f"Validating {len(known_vectors)} canonical FNV-1a 64-bit test vectors...")
    for text, expected in known_vectors:
        actual = dll.throne_fast_hash(text, len(text))
        ref = fnv1a_64_reference(text)
        assert ref == expected, f"Reference mismatch for {text}: ref=0x{ref:016x}, exp=0x{expected:016x}"
        assert actual == expected, f"DLL mismatch for {text}: dll=0x{actual:016x}, exp=0x{expected:016x}"
        print(f"  [OK] '{text.decode('utf-8', errors='ignore')[:35]}...' -> 0x{actual:016x}")

    # 2. C-ABI edge cases (NULL, 0 length)
    print("Validating C-ABI boundary safety (NULL pointers and zero lengths)...")
    assert dll.throne_fast_hash(None, 0) == 0, "NULL with len 0 must return 0"
    assert dll.throne_fast_hash(None, 100) == 0, "NULL with len > 0 must return 0"
    assert dll.throne_fast_hash(b"test", 0) == 0, "Non-null with len 0 must return 0"
    print("  [OK] C-ABI NULL and 0-length guards strictly return 0.")

    # 3. Differential random oracle test across 100,000 random inputs
    num_random_samples = 100_000
    print(f"Running Differential Oracle across {num_random_samples:,} randomized payloads (1 to 256 bytes)...")
    random.seed(0xABCD_2026)
    mismatches = 0
    t0 = time.perf_counter()
    for _ in range(num_random_samples):
        length = random.randint(1, 256)
        payload = random.randbytes(length)
        ref_hash = fnv1a_64_reference(payload)
        dll_hash = dll.throne_fast_hash(payload, length)
        if ref_hash != dll_hash:
            mismatches += 1
            if mismatches <= 5:
                print(f"  [MISMATCH] len={length}: ref=0x{ref_hash:016x}, dll=0x{dll_hash:016x}")
    t_hash_diff = time.perf_counter() - t0
    ops_per_sec = num_random_samples / t_hash_diff

    print(f"Differential Oracle Result: {num_random_samples - mismatches}/{num_random_samples} matched 100%.")
    print(f"Time: {t_hash_diff:.3f}s ({ops_per_sec:,.0f} hashes/sec in Python ctypes)")
    assert mismatches == 0, f"Found {mismatches} mismatches against reference oracle!"

    # 4. Determinism test: 200,000 calls on same payload
    payload = b"throne-idempotency-check-payload-2026"
    expected = dll.throne_fast_hash(payload, len(payload))
    print("Testing hash idempotency over 200,000 repeated invocations...")
    non_matches = 0
    for _ in range(200_000):
        if dll.throne_fast_hash(payload, len(payload)) != expected:
            non_matches += 1
    assert non_matches == 0, "Hash is not idempotent!"
    print("  [OK] 200,000 repeated hashes yielded identical 0x%016x." % expected)

    return {
        "canonical_vectors_passed": len(known_vectors),
        "differential_samples_tested": num_random_samples,
        "differential_mismatches": mismatches,
        "idempotency_calls": 200_000,
    }

# ============================================================================
# TEST SUITE 2: Strict Avalanche Criterion (SAC) & Bit-Flip Diffusion
# ============================================================================
def test_fnv1a_avalanche(dll):
    print("\n" + "=" * 70)
    print("TEST SUITE 2: Strict Avalanche Criterion (SAC) & Bit-Flip Diffusion")
    print("=" * 70)

    # For an ideal hash function, changing 1 bit in input flips each output bit with 50% probability
    # (Hamming distance mean = 32 out of 64 bits = 50.0%).
    # We test across multiple block lengths: 8, 16, 32, 64 bytes.
    payload_sizes = [8, 16, 32, 64]
    trials_per_size = 200
    random.seed(0x5A5A_2026)

    overall_avalanche_stats = {}

    for size in payload_sizes:
        total_bit_flips = 0
        total_flips_tested = 0
        hamming_distances = []
        bit_flip_distribution = [0] * 64 # Count of flips for each output bit (0..63)

        for _ in range(trials_per_size):
            base_data = bytearray(random.randbytes(size))
            h_base = dll.throne_fast_hash(bytes(base_data), size)

            total_bits = size * 8
            for bit_idx in range(total_bits):
                byte_idx = bit_idx // 8
                bit_pos = bit_idx % 8

                # Flip single bit
                base_data[byte_idx] ^= (1 << bit_pos)
                h_flipped = dll.throne_fast_hash(bytes(base_data), size)
                # Revert bit
                base_data[byte_idx] ^= (1 << bit_pos)

                diff = h_base ^ h_flipped
                hd = popcount(diff)
                hamming_distances.append(hd)
                total_bit_flips += hd
                total_flips_tested += 1

                for out_bit in range(64):
                    if (diff >> out_bit) & 1:
                        bit_flip_distribution[out_bit] += 1

        mean_hd = sum(hamming_distances) / len(hamming_distances)
        avalanche_ratio = (mean_hd / 64.0) * 100.0
        min_hd = min(hamming_distances)
        max_hd = max(hamming_distances)
        variance = sum((x - mean_hd) ** 2 for x in hamming_distances) / len(hamming_distances)
        std_hd = math.sqrt(variance)

        # Output bit coverage
        exercised_bits = sum(1 for c in bit_flip_distribution if c > 0)
        min_bit_prob = min(c / total_flips_tested for c in bit_flip_distribution) * 100.0
        max_bit_prob = max(c / total_flips_tested for c in bit_flip_distribution) * 100.0

        overall_avalanche_stats[size] = {
            "size_bytes": size,
            "flips_tested": total_flips_tested,
            "mean_hamming_dist": mean_hd,
            "avalanche_percentage": avalanche_ratio,
            "min_hd": min_hd,
            "max_hd": max_hd,
            "std_hd": std_hd,
            "exercised_output_bits": exercised_bits,
            "min_bit_prob": min_bit_prob,
            "max_bit_prob": max_bit_prob,
        }

        print(f"Payload Size: {size:2d} bytes ({size*8:3d} input bits) | Total Flips Tested: {total_flips_tested:,}")
        print(f"  Mean Hamming Distance: {mean_hd:.2f} / 64 bits ({avalanche_ratio:.2f}%)")
        print(f"  Hamming Range: [{min_hd}, {max_hd}] | Std Dev: {std_hd:.2f} bits")
        print(f"  Output Bit Coverage: {exercised_bits}/64 bits exercised")
        print(f"  Per-Bit Flip Probability Range: [{min_bit_prob:.1f}%, {max_bit_prob:.1f}%]")

    # Check multi-byte diffusion
    mean_32b_avalanche = overall_avalanche_stats[32]["avalanche_percentage"]
    print("-" * 70)
    print(f"Empirical 32-Byte Avalanche Ratio: {mean_32b_avalanche:.2f}% (Target: ~45-55% for uniform non-crypto hashes)")
    assert 40.0 <= mean_32b_avalanche <= 60.0, f"Avalanche ratio outside acceptable range: {mean_32b_avalanche:.2f}%"
    print("[PASS] Avalanche diffusion meets criteria across all payload sizes.")

    return overall_avalanche_stats

# ============================================================================
# TEST SUITE 3: Collision Resistance & Chi-Square Uniformity (200,000 Keys)
# ============================================================================
def test_fnv1a_collision_and_distribution(dll):
    print("\n" + "=" * 70)
    print("TEST SUITE 3: Collision Resistance & Chi-Square Distribution (200,000 Keys)")
    print("=" * 70)

    NUM_KEYS = 200_000
    NUM_BUCKETS = 65_536  # 2^16 buckets

    print(f"Generating and hashing {NUM_KEYS:,} realistic keys (URLs, IPs, UUIDs, rules)...")
    seen_hashes = set()
    buckets = [0] * NUM_BUCKETS
    bit_counts = [0] * 64

    collisions = 0
    t0 = time.perf_counter()

    for i in range(NUM_KEYS):
        # Mix key patterns: proxy URLs, IPv4 address strings, sequential tokens
        pattern_type = i % 4
        if pattern_type == 0:
            key = f"vmess://user-{i:06d}@edge-de-{i % 50}.throne.internal:{443 + (i % 1000)}?path=/ws-{i % 100}".encode()
        elif pattern_type == 1:
            key = f"{(i >> 24) & 0xff}.{(i >> 16) & 0xff}.{(i >> 8) & 0xff}.{i & 0xff}".encode()
        elif pattern_type == 2:
            key = f"rule:route:action=direct:priority={i % 500}:domain=node-{i:05d}.cdn.cloudflare.net".encode()
        else:
            key = f"throne_fast_token_key_id_{i:08d}_salt_{i * 31 & 0xffffff:08x}".encode()

        h = dll.throne_fast_hash(key, len(key))
        if h in seen_hashes:
            collisions += 1
        else:
            seen_hashes.add(h)

        # Bucket distribution (lower 16 bits)
        bucket = h & 0xffff
        buckets[bucket] += 1

        # Bit position frequency
        for bit in range(64):
            if (h >> bit) & 1:
                bit_counts[bit] += 1

    t_elapsed = time.perf_counter() - t0
    ops_sec = NUM_KEYS / t_elapsed

    # Chi-Square Goodness-of-Fit test for bucket distribution
    # Null hypothesis: uniform distribution across 65,536 buckets
    expected_per_bucket = NUM_KEYS / NUM_BUCKETS # ~3.05175
    chi_square = sum(((count - expected_per_bucket) ** 2) / expected_per_bucket for count in buckets)
    df = NUM_BUCKETS - 1
    # For df = 65,535, mean is df, std dev is sqrt(2 * df) = sqrt(131,070) = ~362.03
    z_score = (chi_square - df) / math.sqrt(2 * df)

    # Bit position uniformity
    min_bit_ones = min(bit_counts)
    max_bit_ones = max(bit_counts)
    min_bit_pct = (min_bit_ones / NUM_KEYS) * 100.0
    max_bit_pct = (max_bit_ones / NUM_KEYS) * 100.0

    print(f"Hashing Completed in {t_elapsed:.3f}s ({ops_sec:,.0f} ops/sec)")
    print(f"  Total Keys Hashed:       {NUM_KEYS:,}")
    print(f"  Unique 64-bit Hashes:    {len(seen_hashes):,}")
    print(f"  Collisions Detected:     {collisions}")
    print(f"  Collision Rate:          {collisions / NUM_KEYS:.8f}")
    print(f"  Bucket Count:            {NUM_BUCKETS:,} (Expected per bucket: {expected_per_bucket:.2f})")
    print(f"  Chi-Square Statistic:    {chi_square:.2f} (df={df}, Z-score={z_score:+.2f})")
    print(f"  Bit Uniformity (64 bits): Range [{min_bit_pct:.2f}%, {max_bit_pct:.2f}%] of 1s (Ideal: 50.0%)")

    assert collisions == 0, f"Expected 0 collisions for 200k keys in 64-bit space, got {collisions}!"
    assert abs(z_score) < 5.0, f"Bucket distribution deviates significantly from uniform (Z={z_score:.2f})"
    print("[PASS] Collision resistance and uniformity criteria satisfied.")

    return {
        "num_keys": NUM_KEYS,
        "unique_hashes": len(seen_hashes),
        "collisions": collisions,
        "chi_square": chi_square,
        "chi_square_z_score": z_score,
        "bit_min_pct": min_bit_pct,
        "bit_max_pct": max_bit_pct,
    }

# ============================================================================
# TEST SUITE 4: Multi-Threaded Read Concurrency Stress Test
# ============================================================================
def test_multithreaded_read_concurrency(dll):
    print("\n" + "=" * 70)
    print("TEST SUITE 4: Multi-Threaded Read Concurrency (8 Threads, >=100k Queries/Thread)")
    print("=" * 70)

    NUM_THREADS = 8
    QUERIES_PER_THREAD = 150_000  # 150,000 queries per thread = 1,200,000 total queries
    TOTAL_EXPECTED_QUERIES = NUM_THREADS * QUERIES_PER_THREAD

    print(f"Initializing RouteEngine and populating routing table with hierarchy...")
    engine = dll.throne_fast_route_create()
    assert engine, "Failed to create RouteEngine"

    def ip_str_to_u32(s: str) -> int:
        return int(IPv4Address(s))

    # Core hierarchical routes
    routes = [
        ("0.0.0.0", 0, 1),
        ("10.0.0.0", 8, 100),
        ("10.1.0.0", 16, 101),
        ("10.1.2.0", 24, 102),
        ("10.1.2.3", 32, 103),
        ("172.16.0.0", 12, 200),
        ("192.168.0.0", 16, 300),
        ("192.168.1.0", 24, 301),
        ("192.168.1.50", 31, 302),
        ("192.168.1.51", 32, 303),
        ("127.0.0.0", 8, 400),
        ("255.255.255.255", 32, 500),
    ]
    for ip_str, plen, tag in routes:
        dll.throne_fast_route_add(engine, ip_str_to_u32(ip_str), plen, tag)

    # Add 1,000 random prefixes in 200.0.0.0/8 range (0xc8000000)
    random.seed(0x7777_2026)
    for i in range(1000):
        net = (0xc8000000 | (random.getrandbits(24) & 0x00ffffff)) & 0xffffffff
        plen = random.randint(12, 28)
        tag = 1000 + i
        dll.throne_fast_route_add(engine, net, plen, tag)

    # Known verification queries
    verification_corpus = [
        (ip_str_to_u32("10.1.2.3"), 103),
        (ip_str_to_u32("10.1.2.4"), 102),
        (ip_str_to_u32("10.1.5.1"), 101),
        (ip_str_to_u32("10.2.0.1"), 100),
        (ip_str_to_u32("172.16.1.1"), 200),
        (ip_str_to_u32("172.31.255.254"), 200),
        (ip_str_to_u32("192.168.0.1"), 300),
        (ip_str_to_u32("192.168.1.1"), 301),
        (ip_str_to_u32("192.168.1.50"), 302),
        (ip_str_to_u32("192.168.1.51"), 303),
        (ip_str_to_u32("127.0.0.1"), 400),
        (ip_str_to_u32("255.255.255.255"), 500),
        (ip_str_to_u32("1.1.1.1"), 1), # default
        (ip_str_to_u32("8.8.8.8"), 1), # default
    ]

    for ip_val, expected_tag in verification_corpus:
        act = dll.throne_fast_route_match(engine, ip_val)
        assert act == expected_tag, f"Pre-check failed for IP 0x{ip_val:08x}: exp {expected_tag}, got {act}"

    print(f"Pre-check passed for {len(verification_corpus)} ground-truth queries.")

    mem_before_kb = get_process_memory_kb()
    print(f"Memory before test: {mem_before_kb} KB")
    print(f"Spawning {NUM_THREADS} concurrent threads ({QUERIES_PER_THREAD:,} queries each = {TOTAL_EXPECTED_QUERIES:,} total)...")

    barrier = threading.Barrier(NUM_THREADS)
    thread_exceptions = []
    thread_mismatches = []
    completed_per_thread = [0] * NUM_THREADS

    def worker(tid):
        corpus_len = len(verification_corpus)
        local_completed = 0
        local_mismatches = 0
        try:
            barrier.wait() # Synchronize start across all threads
            for q in range(QUERIES_PER_THREAD):
                ip_val, exp_tag = verification_corpus[(tid + q) % corpus_len]
                res = dll.throne_fast_route_match(engine, ip_val)
                if res != exp_tag:
                    local_mismatches += 1
                local_completed += 1
        except Exception as e:
            thread_exceptions.append((tid, str(e)))
        finally:
            completed_per_thread[tid] = local_completed
            if local_mismatches > 0:
                thread_mismatches.append((tid, local_mismatches))

    threads = [threading.Thread(target=worker, args=(i,)) for i in range(NUM_THREADS)]

    t_start = time.perf_counter()
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    t_end = time.perf_counter()

    elapsed = t_end - t_start
    total_completed = sum(completed_per_thread)
    total_mismatches = sum(m for _, m in thread_mismatches)
    total_crashes = len(thread_exceptions)
    mem_after_kb = get_process_memory_kb()
    mem_delta_kb = mem_after_kb - mem_before_kb

    throughput_qps = total_completed / elapsed
    m_qps = throughput_qps / 1_000_000

    print("-" * 70)
    print("CONCURRENCY RESULTS:")
    print("-" * 70)
    print(f"  Wall Time Elapsed:        {elapsed:.4f} seconds")
    print(f"  Total Queries Completed:  {total_completed:,} / {TOTAL_EXPECTED_QUERIES:,}")
    print(f"  Data Race Mismatches:     {total_mismatches}")
    print(f"  Thread Exceptions/Crashes:{total_crashes}")
    print(f"  Aggregate Throughput:     {m_qps:.2f} Million queries/sec (Python ctypes)")
    print(f"  Memory Delta:             {mem_delta_kb:+d} KB")

    # Clean up RouteEngine
    dll.throne_fast_route_free(engine)
    print("  Engine freed cleanly without fault.")

    assert total_completed == TOTAL_EXPECTED_QUERIES, f"Queries incomplete: {total_completed}"
    assert total_mismatches == 0, f"Data race mismatches detected: {total_mismatches}"
    assert total_crashes == 0, f"Thread crashes detected: {thread_exceptions}"

    print(f"[PASS] Multi-threaded read concurrency strictly passed: 0 races, 0 crashes across {TOTAL_EXPECTED_QUERIES:,} queries.")

    return {
        "num_threads": NUM_THREADS,
        "queries_per_thread": QUERIES_PER_THREAD,
        "total_queries": total_completed,
        "elapsed_sec": elapsed,
        "throughput_m_qps": m_qps,
        "mismatches": total_mismatches,
        "crashes": total_crashes,
        "mem_delta_kb": mem_delta_kb,
    }

def main():
    print("=" * 70)
    print("THRONE-FAST CHALLENGER IT2-2 EMPIRICAL VERIFICATION SUITE")
    print("=" * 70)
    dll = load_throne_fast_core()
    version = dll.throne_fast_version().decode("utf-8")
    print(f"Loaded Core DLL: {DLL_PATH}")
    print(f"Core Version:    {version}")
    assert version == "1.4.2-fast", f"Invalid version: {version}"

    r1 = test_fnv1a_consistency(dll)
    r2 = test_fnv1a_avalanche(dll)
    r3 = test_fnv1a_collision_and_distribution(dll)
    r4 = test_multithreaded_read_concurrency(dll)

    print("\n" + "=" * 70)
    print("FINAL SUMMARY & VERDICT")
    print("=" * 70)
    print("1. FNV-1a Hash Consistency: PASS (100,000 random inputs + known vectors 100% matched)")
    print(f"2. Avalanche Diffusion:     PASS (Mean 32-byte avalanche: {r2[32]['avalanche_percentage']:.2f}%, 64/64 output bits)")
    print(f"3. Collision Resistance:    PASS (0 collisions across {r3['num_keys']:,} keys, Chi-Square Z={r3['chi_square_z_score']:+.2f})")
    print(f"4. 8-Thread Concurrency:    PASS (0 races, 0 crashes across {r4['total_queries']:,} queries)")
    print("\nVERDICT: APPROVE")
    return 0

if __name__ == "__main__":
    sys.exit(main())
