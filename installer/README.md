# Throne WiX v5 MSI Packaging & Deployment Pipeline

This directory contains the production-grade **WiX v5** MSI packaging toolchain for **Throne** (and **Throne-Fast**). It builds a fully self-contained, enterprise-compliant Windows Installer package (`.msi`) featuring automated Windows Firewall rules, process lifecycle management, desktop and start menu shortcuts, and registry persistence.

---

## 1. Directory Structure

```text
installer/
├── Package.wxs             # Portable WiX v5 package definition using $(var.StageDir)
├── generate_wxs.py         # Automated stage scanner & deterministic WXS generator
├── build-msi.ps1           # Full build, extension check, and COM schema verification script
├── build-msi.bat           # Command-line wrapper for PowerShell build script
├── License.rtf             # GPLv3 RTF license agreement for installer UI
├── Throne.ico              # High-resolution multi-size application icon
├── README.md               # Packaging, automation, and enterprise deployment documentation
└── stage/                  # Target staging folder containing binary payloads
    ├── Throne.exe          # Main Qt/C++ GUI application
    ├── ThroneCore.exe      # sing-box Go proxy engine
    ├── libcronet.dll       # Chromium network stack library
    ├── throne_fast_core.dll# High-performance Rust route/filter acceleration core
    ├── updater.exe         # Auto-updater utility
    └── config/             # Web dashboards & runtime configuration assets
        ├── dashboard/      # Minimal default dashboard
        └── sb-dashboard/   # Full sing-box PWA dashboard with pre-compiled web fonts & assets
```

---

## 2. Prerequisites

1. **.NET SDK** (version 8.0, 9.0, or 10.0+):
   ```powershell
   dotnet --version
   ```
2. **WiX v5 CLI** (.NET Global Tool):
   ```powershell
   dotnet tool install --global wix
   wix --version
   ```
3. **Required WiX Extensions**:
   - `WixToolset.UI.wixext` (v5.0.0+)
   - `WixToolset.Util.wixext` (v5.0.0+)
   - `WixToolset.Firewall.wixext` (v5.0.0+)

> *Note:* `build-msi.ps1` automatically detects and installs any missing WiX extensions globally.

---

## 3. Staging Build Artifacts

Before compiling the MSI, populate `installer/stage/` with the compiled binaries:

1. **Compile Rust Core**:
   ```powershell
   cd throne_fast_core
   cargo build --release
   Copy-Item target\release\throne_fast_core.dll ..\installer\stage\
   ```
2. **Compile C++ Qt UI (`Throne.exe`) & sing-box Core (`ThroneCore.exe`)**:
   - Build via CMake / Qt Creator into standard output directories.
   - Copy `Throne.exe`, `ThroneCore.exe`, `libcronet.dll`, and `updater.exe` into `installer\stage\`.
3. **Copy Configuration Assets**:
   - Ensure `config\dashboard\` and `config\sb-dashboard\` are present in `installer\stage\`.

---

## 4. Building the MSI Package

### Option A: Using the Automated PowerShell Script (Recommended)

```powershell
# From repo root or installer/
cd installer
.\build-msi.ps1
```

Customizing output path or stage location:
```powershell
.\build-msi.ps1 -StageDir "..\dist\win-x64" -OutputFile "..\Throne-1.4.2-x64-Setup.msi"
```

### Option B: Using the Batch Wrapper

```cmd
cd installer
build-msi.bat
```

### Option C: Manual `wix build` Command

```powershell
wix build -arch x64 `
    -ext WixToolset.UI.wixext `
    -ext WixToolset.Util.wixext `
    -ext WixToolset.Firewall.wixext `
    -d "StageDir=stage" `
    -out "Throne-1.4.2-x64-Setup.msi" `
    Package.wxs
```

---

## 5. Regenerating `Package.wxs` (Optional)

If new files or directories are added to the application payload, run `generate_wxs.py` to regenerate the WiX definition with deterministic GUIDs:

```powershell
python generate_wxs.py --stage-dir stage --output Package.wxs --version 1.4.2.0
```

Key features of `generate_wxs.py`:
- **Deterministic GUIDs**: Uses UUIDv5 derived from the namespace `29950a94-3c8d-4043-9e00-36ab69f78042`. Upgrades will cleanly replace existing files without orphaned component GUIDs.
- **Portable Variables**: Emits `Source="$(var.StageDir)\..."` instead of hardcoded developer workstation paths.

---

## 6. Installer Architecture & Windows Features

### 6.1 Native Windows Firewall Rules (`Wix5FirewallException`)
During installation, the MSI automatically creates native inbound/outbound Windows Defender Firewall rules for both binaries so users are not interrupted by modal firewall dialogs:
- **Throne**: `Throne.exe` (Scope: `any`)
- **Throne Core (sing-box)**: `ThroneCore.exe` (Scope: `any`)

### 6.2 Process Lifecycle Management (`Wix4CloseApplication`)
When upgrading or uninstalling, the MSI queries running instances of:
- `Throne.exe`
- `ThroneCore.exe`
If found active, it prompts the user to close them or automatically delivers a shutdown signal (`CloseMessage="yes"`), preventing `ERROR_INSTALL_ALREADY_RUNNING` and avoiding reboots.

### 6.3 UpgradeCode & Versioning
- **UpgradeCode**: `{29950A94-3C8D-4043-9E00-36AB69F78042}`
  - Matches the upstream Inno Setup `AppId` (`{{29950A94-3C8D-4043-9E00-36AB69F78042}}`), ensuring seamless migration and upgrades.
- **MajorUpgrade Policy**: Configured with `AllowSameVersionUpgrades="yes"` to support rapid developer testing and CI iterations.

### 6.4 Registry Keys
The installer records the following values in `HKLM\Software\Throne`:
- `InstallPath` = `[INSTALLFOLDER]` (e.g. `C:\Program Files\Throne\`)
- `Version` = `1.4.2.0`
- `Executable` = `[INSTALLFOLDER]Throne.exe`

---

## 7. Silent Enterprise Deployment Guide

The generated MSI package fully adheres to standard Windows Installer command-line switches:

### 7.1 Silent Installation (Unattended)
```cmd
msiexec /i Throne-1.4.2-x64-Setup.msi /qn /norestart /l*v "C:\Logs\ThroneInstall.log"
```

### 7.2 Custom Installation Directory
```cmd
msiexec /i Throne-1.4.2-x64-Setup.msi INSTALLFOLDER="D:\Utilities\Throne" /qn
```

### 7.3 Silent Uninstallation
```cmd
msiexec /x Throne-1.4.2-x64-Setup.msi /qn /norestart /l*v "C:\Logs\ThroneUninstall.log"
```
Or via Product Code:
```cmd
msiexec /x {10105B69-BDDD-43E5-987E-49512B9A033B} /qn
```

---

## 8. Verification & Attestation

To inspect and verify an output MSI package:
```powershell
$installer = New-Object -ComObject WindowsInstaller.Installer
$db = $installer.OpenDatabase("Throne-1.4.2-x64-Setup.msi", 0)
$view = $db.OpenView("SELECT Name, Program FROM Wix5FirewallException")
$view.Execute()
while ($rec = $view.Fetch()) {
    Write-Host "$($rec.StringData(1)) -> $($rec.StringData(2))"
}
```
Expected output:
- `Throne -> [#ThroneExe]`
- `Throne Core (sing-box) -> [#ThroneCoreExe]`
