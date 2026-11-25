<div align="center">

<img src="res/public/Throne.png" width="128" alt="Throne-Rust logo">

# Throne-Rust

**High-performance Windows GUI proxy & VPN client (powered by `sing-box` + `Xray-core`), rebuilt with Qt 6.12.1 and a native Rust (`throne_fast_core.dll`) acceleration engine.**

[![Release](https://img.shields.io/github/v/release/IMAN-Py/Throne-Rust?style=flat-square&label=release&color=2F80ED)](https://github.com/IMAN-Py/Throne-Rust/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/IMAN-Py/Throne-Rust/total?style=flat-square&color=1F883D)](https://github.com/IMAN-Py/Throne-Rust/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/IMAN-Py/Throne-Rust/rust-core.yml?branch=main&style=flat-square&label=CI)](https://github.com/IMAN-Py/Throne-Rust/actions/workflows/rust-core.yml)
[![Stars](https://img.shields.io/github/stars/IMAN-Py/Throne-Rust?style=flat-square&color=E3B341)](https://github.com/IMAN-Py/Throne-Rust/stargazers)
[![License](https://img.shields.io/github/license/IMAN-Py/Throne-Rust?style=flat-square&color=6E7781)](LICENSE)
[![Rust Core](https://img.shields.io/badge/Rust-throne__fast__core%20ABI%202.1-DEA584?style=flat-square&logo=rust)](throne_fast_core/src/lib.rs)
[![Upstream](https://img.shields.io/badge/upstream-throneproj%2FThrone-1F6FEB?style=flat-square)](https://github.com/throneproj/Throne)

<p>
  <a href="https://github.com/IMAN-Py/Throne-Rust/releases/latest"><strong>Download Windows MSI</strong></a>
  &nbsp;·&nbsp;
  <a href="#why-throne-rust">Why Throne-Rust</a>
  &nbsp;·&nbsp;
  <a href="docs/BENCHMARKS.md">Benchmarks</a>
  &nbsp;·&nbsp;
  <a href="#supported-protocols--features">Protocols</a>
  &nbsp;·&nbsp;
  <a href="#building-from-source-windows-x64">Build from Source</a>
</p>

</div>

---

**Throne-Rust** tracks the latest upstream [throneproj/Throne](https://github.com/throneproj/Throne) codebase (`dev` @ `d1a2ca78`, Qt `6.12.1`, `sing-box v1.15.0-alpha.13`, `Xray-core 26.11.9`, Go `1.27.0`) and pairs both the C++20/Qt6 desktop client and the Go proxy core with a shared LTO-compiled Rust acceleration library (**`throne_fast_core.dll`**, ABI `2.1`) alongside algorithmic hot-path rewrites guided by **`fast-anything` CPU profiling**.

---

## Why Throne-Rust?

| Workload | Upstream / Scalar Baseline | Throne-Rust (`throne_fast_core.dll`) | Measured Gain |
|---|---:|---:|---:|
| **IPv4 LPM Routing** (500k lookups, 505 CIDRs) | `283.00 ns/lookup` | **`4.25 ns/lookup`** | **66.62x faster** |
| **4 KB Buffer Hashing** (800 MB xxHash64) | `1.11 GB/s` | **`16.82 GB/s`** | **15.09x faster** |
| **XDNS Wire TXT Reply Match** (Go zero-CGO -> Rust FFI) | `165.9 ns/op` | **`116.1 ns/op`** (0 allocs) | **1.43x faster** |
| **Scan Target Batch Permutation** (1,024 v4+v6 targets) | `27.18 ns/target` | **`22.94 ns/target`** (0 allocs) | **1.185x faster** |
| **Batch Outbound Probing** (`probe.runBatch`) | `2 ms × N` forced sleep | **0 ms forced sleep** | Eliminated |
| **UI Profile Table & SQLite Reads** | `O(N)` LRU + `QJsonObject` | **`O(1)` `std::list` LRU + `idx_profiles_gid`** | Zero-copy & prefetched |

---

## Windows Installation (MSI)

Download **`Throne-1.4.2-rust-x64-Setup.msi`** from [GitHub Releases](https://github.com/IMAN-Py/Throne-Rust/releases/latest), or install with automatic SHA-256 verification via PowerShell or [Scoop](https://scoop.sh/):

```powershell
# Verified one-line installer (downloads latest MSI + SHA256SUMS.txt, verifies hash, runs msiexec)
irm https://raw.githubusercontent.com/IMAN-Py/Throne-Rust/main/installer/install.ps1 | iex
```

```powershell
# Or install via Scoop manifest
scoop install https://raw.githubusercontent.com/IMAN-Py/Throne-Rust/main/scoop/throne-rust.json
```

### What the WiX v5 Installer Includes
- **`Throne.exe`** — Static Qt `6.12.1` C++20 desktop GUI linked directly to `throne_fast_core.dll.lib`
- **`ThroneCore.exe`** — `sing-box` + `Xray-core` engine (`CGO_ENABLED=0`) loading `throne_fast_core.dll` dynamically with automatic pure-Go fallback (also staged into elevated grant directories by `CoreElevation`)
- **`throne_fast_core.dll`** — Native Rust acceleration core (`opt-level = 3`, fat LTO, `strip = "symbols"`)
- **`libcronet.dll`**, **`updater.exe`**, and bundled **`dashboard/`** web UI assets
- Start Menu & Desktop shortcuts, per-machine installation, and clean `MajorUpgrade` support

---

## Supported Protocols & Features

- **Protocols**: `VLESS` (Reality / Vision / XHTTP / XDRIVE) · `VMess` · `Trojan` · `Hysteria` / `Hysteria2` · `TUIC` · `WireGuard` · `AmneziaWG` · `AnyTLS` · `ShadowTLS` · `Shadowsocks` · `NaïveProxy` · `TrustTunnel` · `Mieru` · `Snell` · `MASQUE` · `SSH` · `DNSTT / VayDNS` · `MasterDNS` · `OpenVPN` · `OpenConnect` · `SOCKS` · `HTTP(S)` · `Custom Outbound/Config` · `Outbound Chaining` · `Extra Core`
- **Routing & Network**: System Proxy & Windows TUN Mode, Split Routing Rules, GeoIP/GeoSite Rule Sets, Custom DNS / FakeIP / XDNS, Auto-Selector & Latency/Speed Testing, Subscription Groups, Built-in Cloudflare/WARP/Resolver IP Scanner, Live Connections & Traffic Monitor.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                             Throne.exe (C++20 / MSVC / Qt 6.12.1)       │
│  • O(1) std::list LRU + windowed batch prefetch in ProfilesTableModel   │
│  • Zero-copy SQLite column hydration + idx_profiles_gid index           │
│  • O(N) ProfileFilter, IpListParse compact CIDR FFI & IpListsRepo hash  │
│  • Sequence-ordered non-blocking UpdateDataView w/ xxHash64 render skip │
│  • CoreElevation staging for throne_fast_core.dll & GroupUpdater SHA256 │
└───────────────────────────────┬─────────────────────────┬───────────────┘
                                │ static .dll.lib         │ gRPC / IPC
                                ▼                         ▼
                   ┌─────────────────────┐    ┌───────────────────────────┐
                   │ throne_fast_core.dll│◄───│   ThroneCore.exe (Go 1.27)│
                   │ (Rust ABI 2.1, LTO) │    │   CGO_ENABLED=0 loader    │
                   │ • xxHash64 / SHA256 │    │ • FastBatch scan targets  │
                   │ • 4-bit Arena Trie  │    │ • XDNS TXT wire matcher   │
                   │ • Batch Scan Mixer  │    │ • Ping-pong rule cache    │
                   │ • Zero-alloc DNS    │    │ • No 2ms probe sleep      │
                   │ • Compact CIDR/Geo  │    │ • LastIndexByte basename  │
                   └─────────────────────┘    └───────────────────────────┘
```

See [docs/BENCHMARKS.md](docs/BENCHMARKS.md) for profiling traces, differential test coverage, and exact benchmark methodology.

---

## Building from Source (Windows x64)

<details>
<summary><strong>Click to expand prerequisites and step-by-step build commands</strong></summary>

### Prerequisites

- **Visual Studio 2022 / BuildTools** (MSVC x64 C++20, CMake 3.20+, Ninja)
- **Rust toolchain** (`cargo` / `rustc` stable)
- **Go 1.27+** (or Go 1.24+ with `GOTOOLCHAIN=auto`)
- **Static Qt 6.12.x** (`CMAKE_PREFIX_PATH=<qt>/lib/cmake`) & **OpenSSL** (`OPENSSL_ROOT_DIR`)
- **Protocol Buffers** (`protoc` + `protoc-gen-go` + `protoc-gen-go-grpc`)
- **Python 3** & **WiX Toolset v5** (`wix` + `WixToolset.UI.wixext`) for building the `.msi`

### 1. Build & Test Rust Core (`throne_fast_core.dll`)

```powershell
cd throne_fast_core
cargo test --release
cargo build --release
```

### 2. Run C++20 <-> Rust Differential & Benchmark Suite

```powershell
cl.exe /nologo /std:c++20 /O2 /EHsc /Iinclude tests\cpp_fast_core_test.cpp /Fe:cpp_fast_core_test.exe /link throne_fast_core\target\release\throne_fast_core.dll.lib
Copy-Item throne_fast_core\target\release\throne_fast_core.dll . -Force
.\cpp_fast_core_test.exe
```

### 3. Build & Test Go Core (`ThroneCore.exe`)

```powershell
cd core\gen
protoc -I . --go_out=. --go-grpc_out=. libcore.proto
cd ..
$env:CGO_ENABLED = "0"
$env:THRONE_FAST_CORE_DLL = "$PWD\..\throne_fast_core\target\release\throne_fast_core.dll"
go test -count=1 ./internal/fastcore ./internal/probe ./internal/rpc ./internal/scan
$tags = "with_clash_api,with_quic,with_wireguard,with_utls,with_dhcp,with_tailscale,with_openvpn,with_openconnect,badlinkname,tfogo_checklinkname0,with_purego,with_naive_outbound"
go build -o ..\build\ThroneCore.exe -trimpath -ldflags "-w -s -checklinkname=0" -tags $tags .
```

### 4. Build C++20 / Qt6 GUI (`Throne.exe`) & WiX v5 MSI Installer

```powershell
cmake -B build -S . -G Ninja -DCMAKE_BUILD_TYPE=Release -DENABLE_THRONE_FAST_CORE=ON -DINPUT_VERSION="1.4.2-rust"
cmake --build build --parallel

.\installer\build-msi.ps1 -ThroneExe .\build\Throne.exe -ThroneCoreExe .\build\ThroneCore.exe -RustDll .\build\throne_fast_core.dll
```

</details>

---

## Contributing & Security

- Contributions are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md) for the build and verification checklist.
- For security advisories and checksum verification, see [SECURITY.md](SECURITY.md).
- If **Throne-Rust** is useful to you, consider giving it a **Star** on GitHub to help others discover it!

## Credits & License

- **Upstream Project**: [throneproj/Throne](https://github.com/throneproj/Throne) (`GPL-3.0`)
- **Cores & Libraries**: [SagerNet/sing-box](https://github.com/SagerNet/sing-box), [XTLS/Xray-core](https://github.com/xtls/xray-core), [Qv2ray](https://github.com/Qv2ray/Qv2ray), [Qt](https://www.qt.io/), [simple-protobuf](https://github.com/tonda-kriz/simple-protobuf), [fkYAML](https://github.com/fktn-k/fkYAML), [quirc](https://github.com/dlbeer/quirc), [srombauts/sqlitecpp](https://github.com/srombauts/sqlitecpp)
- **Throne-Rust Maintenance & MSI Packaging**: [IMAN-Py/Throne-Rust](https://github.com/IMAN-Py/Throne-Rust)
