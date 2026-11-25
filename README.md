<div align="center">

<img src="res/public/Throne.png" width="128" alt="Throne logo">

# Throne-Rust

**High-performance Windows fork of [throneproj/Throne](https://github.com/throneproj/Throne), rebuilt with Qt 6.12.1 and a native Rust (`throne_fast_core.dll`) + `fast-anything` acceleration engine.**

[![Release](https://img.shields.io/github/v/release/IMAN-Py/Throne-Rust?style=flat-square&label=release&color=2F80ED)](https://github.com/IMAN-Py/Throne-Rust/releases/latest)
[![CI](https://img.shields.io/github/actions/workflow/status/IMAN-Py/Throne-Rust/rust-core.yml?branch=main&style=flat-square&label=CI)](https://github.com/IMAN-Py/Throne-Rust/actions/workflows/rust-core.yml)
[![License](https://img.shields.io/github/license/IMAN-Py/Throne-Rust?style=flat-square&color=6E7781)](LICENSE)
[![Upstream](https://img.shields.io/badge/upstream-throneproj%2FThrone-1F6FEB?style=flat-square)](https://github.com/throneproj/Throne)
[![Rust Core](https://img.shields.io/badge/Rust-throne__fast__core%20ABI%202.1-DEA584?style=flat-square&logo=rust)](throne_fast_core/src/lib.rs)

</div>

**Throne-Rust** tracks the latest upstream [throneproj/Throne](https://github.com/throneproj/Throne) codebase (`dev` @ `d1a2ca78`, Qt 6.12.1, sing-box `v1.15.0-alpha.13`, Xray-core `26.11.9`, Go `1.27.0`) and pairs both the C++20/Qt6 desktop client and the Go proxy core with a shared LTO-compiled Rust acceleration library (**`throne_fast_core.dll`**) alongside algorithmic hot-path rewrites guided by **`fast-anything` CPU profiling**.

---

## Windows Installation

Download **`Throne-1.4.2-rust-x64-Setup.msi`** from [GitHub Releases](https://github.com/IMAN-Py/Throne-Rust/releases/latest) and run the installer. It installs:

- `Throne.exe` (Qt 6.12.1 static standalone executable linked directly to `throne_fast_core.dll`)
- `ThroneCore.exe` (sing-box + Xray core built with `CGO_ENABLED=0`, loading `throne_fast_core.dll`)
- `throne_fast_core.dll` (native Rust acceleration library, `opt-level = 3`, fat LTO)
- `libcronet.dll`, `updater.exe`, and bundled `dashboard/` assets
- Start Menu & Desktop shortcuts + clean per-machine uninstall & `MajorUpgrade` metadata

---

## Architecture & What Was Accelerated

```
┌─────────────────────────────────────────────────────────────────────────┐
│                             Throne.exe (C++20 / MSVC / Qt 6.12.1)       │
│  • O(1) std::list LRU + batch prefetch in ProfilesTableModel            │
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

### Key Measured Results (Windows x64)

See [docs/BENCHMARKS.md](docs/BENCHMARKS.md) for methodology, commands, and differential verification suites.

| Workload | Baseline | Throne-Rust | Speedup |
|---|---:|---:|---:|
| **IPv4 LPM Routing** (500k lookups, 505 CIDRs) | `276.63 ns/lookup` | **`6.53 ns/lookup`** | **42.36x** |
| **4 KB Buffer Hashing** (800 MB xxHash64 vs scalar) | `1.10 GB/s` | **`17.07 GB/s`** | **15.54x** |
| **XDNS Wire TXT Reply Match** (Go zero-CGO -> Rust FFI) | `165.9 ns/op` | **`116.1 ns/op`** (0 allocs) | **1.43x** |
| **Scan Target Batch Permutation** (1,024 shuffled v4+v6 targets) | `27.18 ns/target` | **`22.94 ns/target`** (0 allocs) | **1.185x** |
| **Batch Probe Latency Stall** (`probe.runBatch`) | `2 ms × N` forced sleep | **0 ms forced sleep** | Eliminated |

---

## Building from Source (Windows x64)

### Prerequisites

- **Visual Studio 2022 / BuildTools** (MSVC x64 C++20, CMake, Ninja)
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
cd core
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

---

## Supported Protocols

SOCKS · HTTP(S) · Shadowsocks · Trojan · VMess · VLESS · TUIC · Hysteria · Hysteria2 · AnyTLS · Mieru · Snell · NaïveProxy · TrustTunnel · ShadowTLS · WireGuard · AmneziaWG · MASQUE · SSH · DNSTT / VayDNS · MasterDNS · Xray VLESS (XHTTP/XDRIVE) · OpenVPN/OpenConnect · Custom Outbound/Config (sing-box & Xray) · Outbound Chaining · Extra Core

## Credits & License

- **Upstream Project**: [throneproj/Throne](https://github.com/throneproj/Throne) (GPL-3.0)
- **Cores & Libraries**: [SagerNet/sing-box](https://github.com/SagerNet/sing-box), [XTLS/Xray-core](https://github.com/xtls/xray-core), [Qv2ray](https://github.com/Qv2ray/Qv2ray), [Qt](https://www.qt.io/), [simple-protobuf](https://github.com/tonda-kriz/simple-protobuf), [fkYAML](https://github.com/fktn-k/fkYAML), [quirc](https://github.com/dlbeer/quirc), [srombauts/sqlitecpp](https://github.com/srombauts/sqlitecpp)
- **Throne-Rust Maintenance & MSI Packaging**: [IMAN-Py/Throne-Rust](https://github.com/IMAN-Py/Throne-Rust)
