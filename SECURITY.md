# Security Policy

## Supported Versions

Only the latest release of **Throne-Rust** (`v1.4.2-rust.2` and newer on `main`) receives security fixes and installer updates.

## Verifying Downloads

Always download installers from [GitHub Releases](https://github.com/IMAN-Py/Throne-Rust/releases/latest) and verify the SHA-256 checksum against the accompanying `SHA256SUMS.txt` asset before running:

```powershell
Get-FileHash .\Throne-1.4.2-rust-x64-Setup.msi -Algorithm SHA256
```

## Reporting a Vulnerability

If you discover a security issue in the Rust FFI boundary (`throne_fast_core`), MSI packaging, or Windows privilege elevation:

1. Please **do not** disclose exploit details in public issues before a fix is available.
2. Use GitHub's **Private Vulnerability Reporting** (under the repository's **Security** tab) or open a minimal report requesting a private follow-up.
3. For issues originating in upstream proxy cores, please also check [throneproj/Throne](https://github.com/throneproj/Throne), [SagerNet/sing-box](https://github.com/SagerNet/sing-box), or [XTLS/Xray-core](https://github.com/XTLS/xray-core).
