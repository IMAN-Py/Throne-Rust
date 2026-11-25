# Contributing to Throne-Rust

Contributions, bug reports, benchmark profiles, and pull requests are welcome!

## Development Prerequisites (Windows x64)

- **Rust** (`stable` toolchain via `rustup`)
- **Go** (`1.27+`, matching `core/go.mod`)
- **Visual Studio 2022 / BuildTools** (MSVC C++20, CMake 3.20+, Ninja)
- **Qt 6.12.x** (static or shared build with Core, Gui, Widgets, Network, Test) & **OpenSSL**
- **Protocol Buffers** (`protoc` + `protoc-gen-go` + `protoc-gen-go-grpc`)
- **Python 3** & **WiX Toolset v5** (`wix` + `WixToolset.UI.wixext`) for MSI packaging

## Verification Checklist Before Opening a PR

1. **Rust Acceleration Core (`throne_fast_core`)**:
   ```powershell
   cd throne_fast_core
   cargo test --release
   cargo build --release
   ```
2. **C++20 <-> Rust Differential & Benchmark Suite**:
   ```powershell
   cl.exe /nologo /std:c++20 /O2 /EHsc /Iinclude tests\cpp_fast_core_test.cpp /Fe:tests\bin\cpp_fast_core_test.exe /link throne_fast_core\target\release\throne_fast_core.dll.lib
   Copy-Item throne_fast_core\target\release\throne_fast_core.dll tests\bin\ -Force
   .\tests\bin\cpp_fast_core_test.exe
   ```
3. **Go Core Tests (`CGO_ENABLED=0`)**:
   ```powershell
   cd core\gen
   protoc -I . --go_out=. --go-grpc_out=. libcore.proto
   cd ..
   $env:CGO_ENABLED = "0"
   $env:THRONE_FAST_CORE_DLL = "$PWD\..\throne_fast_core\target\release\throne_fast_core.dll"
   go test -count=1 ./internal/fastcore ./internal/probe ./internal/rpc ./internal/scan
   ```
4. **Engine / UI Boundary Check**:
   ```powershell
   cmake -DROOT="$PWD" -P cmake/EngineBoundary.cmake
   ```

## Guidelines

- Keep `src/engine/` free of `QtWidgets` / UI headers (enforced by `cmake/EngineBoundary.cmake`).
- Any optimization added to `throne_fast_core` must preserve bit-for-bit parity with the baseline implementation and include differential tests.
- Never commit personal paths, local machine identifiers, or credentials.
