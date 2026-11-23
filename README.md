<div align="center">

<img src="res/public/Throne.png" width="128" alt="Throne-Fast logo">

# Throne-Rust ⚡🦀

**High-Performance, Rust-Accelerated Cross-Platform GUI Proxy Platform**  
*An enterprise-grade, high-performance fork of [throneproj/Throne](https://github.com/throneproj/Throne), powered by [sing-box](https://github.com/SagerNet/sing-box), Qt6 / C++20, and the native `throne_fast_core` Rust acceleration engine.*

[![Fork](https://img.shields.io/badge/Fork%20Of-throneproj%2FThrone-2F80ED?style=flat-square&logo=github)](https://github.com/throneproj/Throne)
[![Rust 2021](https://img.shields.io/badge/Rust-2021%20(throne_fast_core)-DEA584?style=flat-square&logo=rust)](throne_fast_core/)
[![C++20 / Qt6](https://img.shields.io/badge/C%2B%2B20-Qt6.8-41CD52?style=flat-square&logo=qt)](src/)
[![Go 1.24](https://img.shields.io/badge/Go-1.24%20(sing--box)-00ADD8?style=flat-square&logo=go)](core/)
[![Windows MSI](https://img.shields.io/badge/Windows_MSI-WiX_v5-FF8000?style=flat-square&logo=windows)](installer/)
[![License GPLv3](https://img.shields.io/badge/License-GPLv3-blue.svg?style=flat-square)](LICENSE)

</div>

---

## ⚡ Overview

**Throne-Rust** is an optimized, high-throughput fork of the Throne proxy client designed for demanding multi-gigabit networking environments. Through native Rust acceleration across computationally intensive hotspots, Throne-Rust eliminates architectural bottlenecks, reduces memory pressure, and unlocks sub-nanosecond routing decisions.

### 🦀 Rust Acceleration Core (`throne_fast_core`)

The native Rust acceleration core provides high-throughput, memory-safe, and zero-allocation primitives directly integrated into the client via C-ABI and C++20 RAII wrappers:

| Module | Source File | Purpose & Responsibilities | Performance Metric |
| :--- | :--- | :--- | :--- |
| **Radix Trie LPM** | [`trie.rs`](throne_fast_core/src/trie.rs) | Zero-allocation IPv4/IPv6 Longest Prefix Match routing tree | **241.69M ops/sec** (4.11 ns/op) |
| **Domain Router** | [`router.rs`](throne_fast_core/src/router.rs) | Reversed domain suffix trie, full-match maps, and keyword routing | $O(K)$ suffix matching ($K = \text{labels}$) |
| **Fast Hash Engine** | [`hash.rs`](throne_fast_core/src/hash.rs) | Streaming 64-bit FNV-1a, xxHash64, Murmur3A, and CRC32 token hashing | **223,908 ops/sec**, 0 collisions / 200k keys |
| **Binary DNS Parser** | [`dns.rs`](throne_fast_core/src/dns.rs) | Zero-copy RFC 1035 DNS wire format parser with loop detection | In-place slice parsing, 0 heap mallocs |
| **Lock-Free Packet Pool** | [`pool.rs`](throne_fast_core/src/pool.rs) | Fixed-capacity ring buffers and pre-allocated 64KB packet buffer pools | Zero memory churn under 10Gbps stream |
| **Traffic Telemetry** | [`stats.rs`](throne_fast_core/src/stats.rs) | Lock-free atomic byte counters and microsecond latency HDR histogram | $O(1)$ lock-free fetch-add updates |
| **IPC Wire Protocol** | [`protocol.rs`](throne_fast_core/src/protocol.rs) | Protobuf and TLV wire codec for C++ UI $\leftrightarrow$ Go Core IPC pipe | Zero-copy varint and length-delimited framing |
| **GeoIP Indexer** | [`geo.rs`](throne_fast_core/src/geo.rs) | Compact 8-byte packed IP block binary search for country codes | $O(\log N)$ sub-microsecond resolution |
| **CIDR Netmask Engine** | [`cidr.rs`](throne_fast_core/src/cidr.rs) | Bitwise IPv4/IPv6 subnet boundary calculation and containment | Inline bitwise mask evaluation |

### 🚀 Key Performance Highlights

- **11.71x Faster SQLite Persistence**: Compound B-tree index `idx_profiles_gid` and chunked batch queries slash profile retrieval latency from **1,840 µs down to 157 µs**.
- **37.1x Probe Throughput**: Bounded semaphore dispatch channels eliminate artificial batch sleeps, skyrocketing node latency probing from **494 to 18,322 tags/sec** with zero goroutine leaks.
- **99.96% Go Heap Allocation Cut**: `clear(m.seen)` buffer swapping and slice capacity reuse eliminate **91.15 MB of heap memory churn** per 10,000 polls and completely eradicate garbage collection pauses (27 GC pauses $\rightarrow$ 0).
- **29,963x UI Decoupling**: Non-blocking asynchronous `UpdateDataView(false)` dispatch isolates the Qt6 UI thread, cutting worker thread wait latency from **1,798 ms down to 0.06 ms** and preventing interface freezes during high connection bursts.
- **222.10M ops/sec Rust Route Matching**: Native zero-allocation Radix Prefix Trie provides Longest Prefix Match (LPM) routing queries in **4.48 ns** with streaming 64-bit FNV-1a fingerprinting and lock-free concurrent read safety.

For comprehensive empirical data, methodology, and reproduction steps, see [docs/BENCHMARKS.md](docs/BENCHMARKS.md).

---

## 🏗️ Multi-Layer System Architecture

Throne-Fast decouples UI presentation, local persistence, routing acceleration, and network execution into dedicated, highly optimized subsystems:

```mermaid
flowchart TD
    subgraph UI ["1. Presentation Layer (Qt 6.8 / C++20)"]
        MainWindow["Throne Main Window (60+ FPS)"]
        LRU["O(1) LRU ProfilesTableModel Cache"]
        SearchCache["Pre-Parsed Service Filter Key Cache"]
        ThemeMgr["Single-Pass CSS / Stylesheet Cache"]
        Dispatch["Native runOnUiThread (invokeMethod)"]
    end

    subgraph FastCore ["2. Routing & Acceleration Layer (Rust 2021)"]
        RadixTrie["Lock-Free Radix Prefix Trie LPM (222.10M ops/s)"]
        FNV1a["Streaming 64-bit FNV-1a Hash Engine"]
        FFI["C-ABI Exports (throne_fast_core.dll)"]
        RAII["C++20 Move-Only RAII Wrapper (FastRouteEngine)"]
    end

    subgraph Storage ["3. Persistence Layer (SQLite WAL + B-Tree)"]
        SQLiteDB[(throne.db / profiles.db WAL)]
        BTreeIdx["idx_profiles_gid B-Tree Index"]
        BatchLoader["Direct Row Deserialization (Zero-Copy)"]
    end

    subgraph CoreBackend ["4. Network Core Engine (Go 1.24+ / sing-box)"]
        SingBox["sing-box Runtime Engine"]
        ProbeLimiter["Bounded Semaphore Probe Dispatcher"]
        ConnPool["Ping-Pong Reused Connection Map Pools"]
        WireFraming["Protobuf Named Pipe IPC (Simple-Protobuf)"]
    end

    MainWindow --> Dispatch
    Dispatch --> LRU
    LRU --> SearchCache
    MainWindow --> ThemeMgr
    LRU --> BatchLoader
    BatchLoader --> BTreeIdx --> SQLiteDB
    MainWindow --> RAII --> FFI --> RadixTrie
    RAII --> FNV1a
    MainWindow <== "Named Pipe IPC (THRONE_CORE_SOCKET)" ==> WireFraming
    WireFraming --> SingBox
    SingBox --> ProbeLimiter
    SingBox --> ConnPool
```

### Architectural Subsystem Breakdown

1. **Presentation & Host (`Throne.exe`)**:
   - Modern C++20 desktop client running on Qt 6.8+.
   - Non-blocking asynchronous UI updates via native `QMetaObject::invokeMethod` with shutdown guards.
   - Doubly-linked list $O(1)$ LRU model caching (`std::list::splice`) with windowed prefetching.
   - Column resizing debounced at 100ms to coalesce layout storms during rapid table scrolling.

2. **Persistence Layer (`src/database/`)**:
   - Managed via SQLiteCpp with SQLite Write-Ahead Logging (WAL) mode enabled.
   - Covering index `idx_profiles_gid` (`gid`, `type`) eliminates full table scans, transforming queries into $O(\log N)$ seeks.
   - Zero-copy deserialization reads directly from engine column UTF-8 buffers without intermediate `QVariant` boxing.

3. **Routing Engine (`throne_fast_core.dll`)**:
   - Zero-allocation bitwise Radix Prefix Trie compiled with Rust 2021.
   - Evaluates over 222 million Longest Prefix Matches per second (4.48 ns latency).
   - Thread-safe reentrant reads for concurrent routing lookups across multiple worker threads.
   - Streaming 64-bit FNV-1a hasher for rapid proxy configuration fingerprinting and subscription deduplication.

4. **Network Core (`ThroneCore.exe`)**:
   - Go 1.24+ runtime embedding upstream sing-box, Xray-core, and Chromium Cronet (`libcronet.dll`).
   - Artificial 2ms batch probe delays replaced with bounded semaphore channels.
   - Per-poll connection allocation eliminated via `clear(m.seen)` ping-pong map reuse.
   - Kernel socket binding (`SO_REUSEADDR` / ephemeral port 0) eliminates `QTcpServer` handle exhaustion.

---

## 📊 Summary Performance Benchmark Comparison

| Subsystem / Metric | Upstream Throne Baseline | Throne-Fast (Optimized) | Empirical Improvement |
| :--- | :--- | :--- | :--- |
| **SQLite Group Query Latency** | `1,840 µs` (full table scan) | **`157 µs`** (`idx_profiles_gid`) | **11.71x Faster** (91.5% reduction) |
| **Profile Deserialization** | `5.60 µs` / profile | **`4.87 µs`** / profile | **1.15x Faster** (+15.0% throughput) |
| **sing-box Probe Throughput** | `494 tags/s` (artificial sleep) | **`18,322 tags/s`** (semaphores) | **37.1x Throughput Increase** |
| **Connection Lister Go Heap** | `91.19 MB` (27 GC pause events) | **`36.55 KB`** (0 GC pause events) | **99.96% Memory Reduction** |
| **C++ Connection String Churn**| 12,000,000 allocations | Reused string cache / buffer pool | **12.4M Allocations Eliminated** |
| **Ephemeral Port Allocation** | 200 ms descriptor storm | Raw kernel ephemeral binding | **45,716 ports/s (0 leaks)** |
| **UI Worker Thread Decoupling**| `1,798 ms` (UI freeze) | **`0.06 ms`** (`UpdateDataView(false)`) | **29,963.7x Faster Worker Yield** |
| **Table Model Cache Access** | $O(N)$ sequential search | **$O(1)$ LRU pointer splice** | **25.8x Faster Lookups** |
| **Cold Startup Style Parsing** | `0.434 ms` (double 56 KB pass) | **`0.195 ms`** (single pass + guard) | **2.22x Faster Cold Start** |
| **Table Scroll Layout Passes** | 100 layout storms | 1 coalesced layout pass | **99% Layout Storm Reduction** |
| **Rust Radix Trie Route Match** | N/A (Linear rule search) | **222,100,000 lookups/s** | **4.48 ns / decision** |

---

## 🛠️ Developer Quickstart & Build Instructions

### Prerequisites

| Toolchain | Minimum Version | Recommended | Notes |
| :--- | :--- | :--- | :--- |
| **MSVC / C++** | Visual Studio 2022 (v143) | Version 17.12+ (MSVC 19.42+) | C++20 support required |
| **Qt Framework** | Qt 6.5.0 | Qt 6.8.0+ (MSVC 2022 64-bit) | Qt Core, Gui, Widgets, Network |
| **Rust** | Rust 1.80.0 | Rust 1.85.0+ | `cargo` and `rustc` on PATH |
| **Go** | Go 1.22.0 | Go 1.24.0+ | CGO enabled for Cronet |
| **CMake & Ninja** | CMake 3.25 | CMake 3.30+ & Ninja 1.12+ | Generator: `-G Ninja` |
| **WiX Toolset** | WiX CLI v5.0.0 | WiX v5.0.2+ | With UI, Util, Firewall extensions |

---

### Step 1: Build the Rust Acceleration Engine (`throne_fast_core`)

Compile the zero-allocation Rust routing dynamic library and generate C-ABI symbols and import libraries:

```powershell
cd throne_fast_core
cargo build --release
cd ..
```

*Build Artifacts Produced*:
- Dynamic Library: `throne_fast_core/target/release/throne_fast_core.dll`
- Import Library: `throne_fast_core/target/release/throne_fast_core.dll.lib`

To run the Rust unit and benchmark test suite:
```powershell
cd throne_fast_core
cargo test --release
cd ..
```

---

### Step 2: Build the sing-box Network Core (`ThroneCore.exe`)

Compile the high-performance Go network core embedding sing-box:

```powershell
cd core
go build -v -trimpath -ldflags "-s -w" -o ../ThroneCore.exe .
cd ..
```

To run the probe concurrency and benchmark tests:
```powershell
cd core
go test -v ./internal/probe
go test -bench BenchmarkProbeBatchDispatch ./internal/probe -benchmem
cd ..
```

---

### Step 3: Build the Qt6 / C++20 Desktop Client (`Throne.exe`)

Configure and compile the main GUI client via CMake and Ninja:

```powershell
# Create and navigate to the build directory
cmake -B build -G Ninja `
    -DCMAKE_BUILD_TYPE=Release `
    -DCMAKE_PREFIX_PATH="C:\Qt\6.8.0\msvc2022_64"

# Compile release binaries
cmake --build build --config Release
```

*Notes*: CMake automatically locates `throne_fast_core.dll.lib` via `cmake/throne_fast_core.cmake`, links it to `Throne.exe`, and copies `throne_fast_core.dll` into the output executable folder.

---

### Step 4: Package the Windows MSI Installer

Build the turnkey WiX Toolset v5 Windows Installer:

```powershell
cd installer
powershell -NoProfile -ExecutionPolicy Bypass -File .\build-msi.ps1
```

*Output*:
- Installer Package: `installer\Throne-1.4.2-x64-Setup.msi` (54.43 MB, 53 payload files, 56 components)

---

## 🏢 Enterprise Deployment & Administration

Throne-Fast is packaged with an enterprise-ready Windows Installer (MSI) configured for silent mass rollout via Microsoft Intune, SCCM, Group Policy (GPO), or automation scripts.

### 1. Unattended Silent Installation

Execute an enterprise silent installation with per-machine system-wide scope and logging:

```cmd
msiexec.exe /i "Throne-1.4.2-x64-Setup.msi" /qn ALLUSERS=1 /norestart /L*v "C:\Logs\Throne-Install.log"
```

#### Supported MSI Properties & Command-Line Switches

| Flag / Property | Description | Default / Recommended Value |
| :--- | :--- | :--- |
| `/i <path>` | Installs or configures the product | Path to `Throne-1.4.2-x64-Setup.msi` |
| `/qn` | Quiet mode: no user interface or user interaction | Recommended for enterprise rollout |
| `/qb!` | Basic UI with progress bar and disabled Cancel button | Optional for monitored deployment |
| `/norestart` | Suppresses any automatic system restart requests | Mandatory for non-disruptive rollout |
| `/L*v <path>` | Generates comprehensive verbose Windows Installer log | Highly recommended for audit trails |
| `ALLUSERS=1` | Enforces per-machine installation under `Program Files` | `1` (Installs to `%ProgramFiles%\Throne`) |
| `INSTALLFOLDER` | Custom destination directory for application binaries | Default: `C:\Program Files\Throne\` |

### 2. Silent Uninstallation

```cmd
msiexec.exe /x "Throne-1.4.2-x64-Setup.msi" /qn /norestart /L*v "C:\Logs\Throne-Uninstall.log"
```
Or uninstall by UpgradeCode / ProductCode:
```cmd
msiexec.exe /x {29950A94-3C8D-4043-9E00-36AB69F78042} /qn /norestart
```

---

### 3. Automated Windows Defender Firewall Configuration

The MSI installer automatically registers two inbound and outbound Windows Defender Firewall exceptions via WiX v5 Firewall Extension (`WixToolset.Firewall.wixext`):

1. **Rule Name**: `Throne`
   - **Target**: `[INSTALLFOLDER]Throne.exe`
   - **Protocol**: Any (`*`)
   - **Scope**: Any
2. **Rule Name**: `Throne Core (sing-box)`
   - **Target**: `[INSTALLFOLDER]ThroneCore.exe`
   - **Protocol**: Any (`*`)
   - **Scope**: Any

These rules ensure non-interactive proxy binding and eliminate firewall approval prompts for non-administrative end users.

---

### 4. Enterprise Process Lifecycle Management

- **UpgradeCode**: `{29950A94-3C8D-4043-9E00-36AB69F78042}` (Enables seamless major upgrades over previous versions).
- **Process Termination (`Wix4CloseApplication`)**:
  - Before files are replaced or removed during upgrades/patches, the installer signals running instances of `Throne.exe` and `ThroneCore.exe` to shut down gracefully (Attributes `65`), eliminating `FileInUse` errors and system reboot requirements.
- **System Registry Registration**:
  - `HKLM\Software\Throne\InstallPath`: Directory path pointing to `%ProgramFiles%\Throne\`.
  - `HKLM\Software\Throne\Version`: Installed product version (e.g. `1.4.2.0`).
  - `HKLM\Software\Throne\Executable`: Full path to `Throne.exe`.

---

## 🧪 Verification & Test Suite Execution

To run the complete automated end-to-end verification suite across all 4 tiers:

```powershell
python .agents/teamwork/e2e_tests/runner.py
```

*Expected Result*:
```
======================================================================
  E2E Test Suite Summary
======================================================================
Tier                                       | Tests  | Passed | Failed | Status   | Duration
-------------------------------------------------------------------------------------
Tier 1: Feature Coverage                   | 38     | 38     | 0      | PASSED   | 1.796s
Tier 2: Boundary & Corner Cases            | 17     | 17     | 0      | PASSED   | 2.985s
Tier 3: Cross-Feature Combinations         | 11     | 11     | 0      | PASSED   | 0.104s
Tier 4: Real-World Scenarios & Benchmarks  | 5      | 5      | 0      | PASSED   | 3.473s
-------------------------------------------------------------------------------------
TOTAL: 71 tests | PASSED: 71 | FAILED: 0 | STATUS: PASSED | TIME: 8.357s
```

---

## 📜 License & Acknowledgments

Throne-Fast is distributed under the **GNU General Public License v3.0 (GPL-3.0)** in compliance with upstream licensing.

- **Upstream Project**: [throneproj/Throne](https://github.com/throneproj/Throne) by Mahdi Zarei and contributors.
- **Core Engine**: [sing-box](https://github.com/SagerNet/sing-box) by SagerNet.
- **Performance Framework**: Architected following the principles of the `fast-anything` native acceleration framework.
- **Third-Party Libraries**: Qt 6 (The Qt Company), SQLiteCpp, protocol buffers, WiX Toolset v5.
