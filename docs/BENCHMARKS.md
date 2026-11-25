# Throne-Rust Performance & Profiling Benchmarks

All measurements below were captured natively on **Windows 11 (x86_64)** using `fast-anything`, `go test -bench`, `cargo test --release`, and MSVC (`/std:c++20 /O2`) linked against `throne_fast_core.dll` (`opt-level = 3`, `lto = "fat"`, `codegen-units = 1`).

---

## 1. Hotspot Discovery (`fast-anything v0.7.0`)

Profiling the upstream pure-Go scanner & connection/DNS paths (`fast-anything scan -- go run ...` / `go tool pprof`) identified the top CPU hotspots:

| Function | `fast-anything` Share | `pprof` Cum% | Root Cause | Remediation |
|---|---:|---:|---|---|
| `internal/scan.Targets.At` | 30.2% | 53.12% | Per-target binary search + 4-round mixer loop | Batch `throne_fast_scan_fill_batch` (`FastBatch`) |
| `internal/scan.mixer.mix` | 12.4% | 21.88% (flat) | Repeated ARX + odd-mul rounds in scalar loop | Unrolled Rust `Mixer::mix` over contiguous chunk |
| `internal/scan.targetRange.address` | 12.4% | 21.88% | BigEndian unpack/pack per host | Native `u32`/`u128` wrapping addition in Rust |
| `internal/scan.Targets.permute` | 10.6% | 18.75% | Rejection sampling loop per position | Inlined Rust loop + sequential range hint |
| `strings.genSplit` (`connMetaToProto`) | — | 12.50% | `strings.Split` allocates slice on every poll row | Zero-alloc `strings.LastIndexByte` slice indexing |
| `x/net/dns/dnsmessage` (`parseXDNSReply`) | — | 12.50%+ | Stateful Go parser overhead per DNS reply | Single-pass bounds-checked `throne_fast_dns_match_xdns_reply` |

---

## 2. Go Core Benchmarks (`CGO_ENABLED=0` via `golang.org/x/sys/windows`)

Run with:

```powershell
$env:CGO_ENABLED = "0"
$env:THRONE_FAST_CORE_DLL = "$PWD\..\throne_fast_core\target\release\throne_fast_core.dll"
go test -run=NONE -bench=. -benchmem -benchtime=1s -count=3 ./internal/scan ./internal/rpc
```

| Benchmark | Go Baseline (ns/op) | Rust `throne_fast_core.dll` (ns/op) | Allocs | Speedup |
|---|---:|---:|---:|---:|
| `ParseXDNSReply` (compressed + `0x20` TXT packet) | `165.6 / 167.7 / 164.3` (mean **165.9 ns**) | `116.4 / 117.6 / 114.3` (mean **116.1 ns**) | **0 B / 0 allocs** | **1.43x** (-30.0% latency) |
| `Targets` batch 1024 (mixed shuffled v4+v6 `/32`+multiport) | `27.00 / 27.25 / 27.29` (mean **27.18 ns**) | `22.93 / 23.09 / 22.79` (mean **22.94 ns**) | **0 B / 0 allocs** | **1.185x** (-15.6% per target incl. FFI + `netip.Addr`) |

### Concurrency & Latency Tail Improvements

- **`probe.runBatch` (`core/internal/probe/probe.go`)**: Removed the deterministic `time.Sleep(2 * time.Millisecond)` per outbound tag (`500` tags previously incurred **1,000 ms+** of pure sleep before work could even start; `150` tags verified in unit test under `< 200 ms`, completing in `< 5 ms` with stub outbounds) and added `select` on `ctx.Done()` while acquiring semaphore slots so user cancellation unblocks immediately.
- **`rpc.ruleMatchCache` (`core/internal/rpc/rulematch.go`)**: Switched from `make(map[uuid.UUID]string)` per poll to ping-pong swapping + Go 1.21+ `clear(m.seen)`, eliminating repeated map bucket allocations under active connection polling.
- **`rpc.connMetaToProto` (`core/internal/rpc/connections.go`)**: Switched basename extraction to `strings.LastIndexByte` and pre-sized `active`/`closed` protobuf response slices (`0` basename allocations per connection).

---

## 3. Native C++20 <-> Rust Benchmarks (`tests/cpp_fast_core_test.cpp`)

Compiled with MSVC (`/std:c++20 /O2 /EHsc`) against `throne_fast_core.dll.lib`:

| Workload | C++ Scalar Reference | Rust `throne_fast_core.dll` | Throughput / Latency | Speedup |
|---|---:|---:|---:|---:|
| **IPv4 LPM Routing** (500,000 lookups, 505 CIDRs `/0`..`/32`) | `138.31 ms` (`276.63 ns/lookup`) | `3.27 ms` (`6.53 ns/lookup`) | ~153M lookups/sec | **42.36x** |
| **4 KB Buffer Hashing** (200,000 buffers = 800 MB) | `694.80 ms` (`1.10 GB/s`) | `44.70 ms` (`17.07 GB/s`) | **17.07 GB/s** | **15.54x** |

---

## 4. C++20 / Qt 6.12.1 / SQLite Optimizations

- **`ProfilesTableModel` (`src/ui/utils/ProfilesTableModel.cpp`)**:
  - Replaced $O(N)$ `QList<int>::move()` LRU promotion with $O(1)$ `std::list<int>::splice` + iterator hash map `m_lruPos`.
  - Enlarged model profile cache from `100` to `512` entries and added `[row-8, row+32]` windowed `GetProfileBatch` prefetching on cache misses.
- **`ProfilesRepo` (`src/database/ProfilesRepo.cpp`)**:
  - Added `CREATE INDEX IF NOT EXISTS idx_profiles_gid ON profiles(gid)` for fast group-filtered reads.
  - Hydrates `Profile` directly from raw SQLite columns (`QByteArray::fromRawData` + `QString::fromUtf8`), avoiding an intermediate `QJsonObject` construction and redundant UTF-8 conversions.
- **`ProfileFilter` (`src/database/entities/Profile.cpp`)**:
  - Replaced $O(N^2)$ `QList::removeAll` / `QList::contains` and $O(\log N)$ `QMap<QString>` in `Uniq`, `Common`, `OnlyInSrc`, `OnlyInSrc_ByPointer`, and `ChangedByIdentity` with Rust `throne_fast_xxhash64` hash tables, collision equality check, tombstone compaction, `std::deque`, and pointer `QSet`.
- **`AutoSelectorPlan` (`src/configs/AutoSelectorPlan.cpp`)**:
  - Pre-warms `ProfilesRepo` via `GetProfileBatch` before `sortByLatency` and `eligibleMembers` loops instead of issuing $N+1$ individual SQLite queries.
- **`ConnectionLister` (`src/stats/connectionLister/connectionLister.cpp`)**:
  - Interns repeated connection strings (`network`, `protocol`, `outbound`, `process`, `processPath`, `rule`, `ruleItem`, `source`, `domain`) per poll via Rust `throne_fast_xxhash64` and reuses decoded active rows for traffic accounting.
- **`GroupUpdater` (`src/configs/sub/GroupUpdater.cpp`)**:
  - Computes 32-byte SHA-256 subscription identity/content keys using `throne_fast_sha256_raw`.
- **`MainWindow::UpdateDataView` (`src/ui/mainWindow/mainwindow_view.cpp`)**:
  - Switched from synchronous cross-thread blocking (`block = true`) to non-blocking `QPointer`-guarded updates with Rust `throne_fast_xxhash64` diff skipping before calling `setHtml` / `setToolTip`.

---

## 5. Differential Verification Summary

Every accelerated path has an explicit automated differential test asserting exact parity against the original baseline:

- `TestFastBatchDifferentialEquivalence` (`core/internal/scan/targets_test.go`): Mixed shuffled IPv4 + IPv6 `/32` + multiport + sampled `/16` specs verified bit-for-bit against Go `Targets.At`.
- `TestParseXDNSReplyDifferential` (`core/internal/scan/dns_test.go`): 21 wire DNS cases (compressed labels, `0x20` randomized case, 10-hop/11-hop compression chains, literal dot label rejection, 254/255-byte names, NXDOMAIN without questions, truncation, self-referential pointer loop guards) verified identically against Go `dnsmessage.Parser`.
- `cpp_fast_core_test.cpp` (`tests/cpp_fast_core_test.cpp`): 10,000 random + corner IPv4 probes over 505 overlapping CIDRs verified 100% identically against a linear longest-prefix oracle, plus xxHash64/SHA-256 NIST vectors, RFC 5952 & compact CIDR masking (`0xFF` all 8 feature flags), range cover, GeoIP, and domain routing tests.
- `qt_fast_integration_test.cpp` (`tests/qt_fast_integration_test.cpp`): 6/6 QtTest integration checks verifying compact CIDR FFI parity with `Scanner::NormalizeCidr`, IP list range parsing & deduplication, `ProfileFilter` correctness, `ProfilesRepo` batch hydration & `idx_profiles_gid` index, `IpListsRepo` deduplication, and `DataViewHtmlGenerator` sequence ordering.
- `cargo test --release` (`throne_fast_core`): 16/16 unit tests passing.
