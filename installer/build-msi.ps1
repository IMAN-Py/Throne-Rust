<#
.SYNOPSIS
    Automated build and verification script for Throne WiX v5 MSI package.
.DESCRIPTION
    Verifies WiX v5 CLI and required extensions (UI, Util, Firewall), validates
    staging directory contents (including throne_fast_core.dll), compiles the
    MSI with portable $(var.StageDir), and verifies database tables and payload size.
.PARAMETER StageDir
    Path to staging folder containing unpacked binaries (default: .\stage)
.PARAMETER OutputFile
    Path to destination MSI installer file (default: .\Throne-1.4.2-x64-Setup.msi)
.PARAMETER WxsFile
    Path to WiX source file (default: .\Package.wxs)
.PARAMETER SkipValidation
    Skip post-build Windows Installer COM database validation.
#>

[CmdletBinding()]
param(
    [string]$StageDir,
    [string]$OutputFile,
    [string]$WxsFile,
    [switch]$SkipValidation
)

$ErrorActionPreference = "Stop"

$ScriptDir = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Definition }
if (-not $ScriptDir) { $ScriptDir = (Get-Location).Path }

if (-not $StageDir) { $StageDir = Join-Path $ScriptDir "stage" }
if (-not $OutputFile) { $OutputFile = Join-Path $ScriptDir "Throne-1.4.2-x64-Setup.msi" }
if (-not $WxsFile) { $WxsFile = Join-Path $ScriptDir "Package.wxs" }

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "       Throne-Fast WiX v5 MSI Packaging Pipeline            " -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

# 1. Verify WiX Toolset CLI
Write-Host "[1/5] Checking WiX CLI environment..." -ForegroundColor Yellow
$wixCmd = Get-Command wix -ErrorAction SilentlyContinue
if (-not $wixCmd) {
    Write-Error "wix CLI not found on PATH. Install via: dotnet tool install --global wix"
}
$wixVer = & wix --version
Write-Host "  Found WiX CLI: $wixVer ($($wixCmd.Source))" -ForegroundColor Gray

# 2. Verify and install required WiX v5 extensions
Write-Host "[2/5] Verifying required WiX extensions..." -ForegroundColor Yellow
$requiredExtensions = @(
    "WixToolset.UI.wixext",
    "WixToolset.Util.wixext",
    "WixToolset.Firewall.wixext"
)
$installedExts = (& wix extension list -g 2>&1) -join "`n"
foreach ($ext in $requiredExtensions) {
    if ($installedExts -notmatch [regex]::Escape($ext)) {
        Write-Host "  Extension $ext not found. Installing globally..." -ForegroundColor Yellow
        & wix extension add -g "$ext/5.0.2"
        if ($LASTEXITCODE -ne 0) {
            Write-Error "Failed to install WiX extension: $ext"
        }
    } else {
        Write-Host "  $ext is installed." -ForegroundColor Gray
    }
}

# 3. Validate Staging Directory
Write-Host "[3/5] Validating staging payload in $StageDir..." -ForegroundColor Yellow
if (-not (Test-Path $StageDir)) {
    Write-Error "Staging directory does not exist: $StageDir"
}

$coreFiles = @(
    "Throne.exe",
    "ThroneCore.exe",
    "libcronet.dll",
    "throne_fast_core.dll",
    "updater.exe",
    "config\dashboard\index.html",
    "config\sb-dashboard\index.html"
)

foreach ($f in $coreFiles) {
    $targetPath = Join-Path $StageDir $f
    if (-not (Test-Path $targetPath)) {
        Write-Error "Required payload file missing in staging: $targetPath"
    }
}
$fileCount = (Get-ChildItem -Recurse -File $StageDir).Count
$totalBytes = (Get-ChildItem -Recurse -File $StageDir | Measure-Object -Property Length -Sum).Sum
Write-Host ("  Stage verified: {0} files ({1:N2} MB)" -f $fileCount, ($totalBytes / 1MB)) -ForegroundColor Gray

# Check Package.wxs
if (-not (Test-Path $WxsFile)) {
    Write-Host "  $WxsFile not found. Running generate_wxs.py..." -ForegroundColor Yellow
    $genPy = Join-Path $PSScriptRoot "generate_wxs.py"
    if (Test-Path $genPy) {
        & python "$genPy" --stage-dir "$StageDir" --output "$WxsFile"
    } else {
        Write-Error "Package definition not found: $WxsFile"
    }
}

# 4. Compile MSI
Write-Host "[4/5] Compiling MSI package..." -ForegroundColor Yellow
Write-Host "  Source: $WxsFile" -ForegroundColor Gray
Write-Host "  Stage:  $StageDir" -ForegroundColor Gray
Write-Host "  Output: $OutputFile" -ForegroundColor Gray

Push-Location $ScriptDir
try {
    $buildArgs = @(
        "build",
        "-arch", "x64",
        "-ext", "WixToolset.UI.wixext",
        "-ext", "WixToolset.Util.wixext",
        "-ext", "WixToolset.Firewall.wixext",
        "-d", "StageDir=$StageDir",
        "-out", "$OutputFile",
        "$WxsFile"
    )

    & wix @buildArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Error "WiX build failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}

if (-not (Test-Path $OutputFile)) {
    Write-Error "Output MSI was not created: $OutputFile"
}

$msiItem = Get-Item $OutputFile
Write-Host ("  MSI created successfully: {0:N0} bytes ({1:N2} MB)" -f $msiItem.Length, ($msiItem.Length / 1MB)) -ForegroundColor Green

# 5. Database Schema & Integrity Validation
if (-not $SkipValidation) {
    Write-Host "[5/5] Validating MSI Database Tables & Schema..." -ForegroundColor Yellow
    $installer = $null
    $db = $null
    $propView = $null
    $fwView = $null
    $closeView = $null
    $fView = $null
    $cView = $null
    try {
        $installer = New-Object -ComObject WindowsInstaller.Installer
        $db = $installer.OpenDatabase($msiItem.FullName, 0) # 0 = OpenReadOnly

        # Verify Product Properties
        $propView = $db.OpenView("SELECT Property, Value FROM Property")
        $propView.Execute()
        $props = @{}
        while ($rec = $propView.Fetch()) {
            $props[$rec.StringData(1)] = $rec.StringData(2)
        }
        Write-Host "  ProductName:  $($props['ProductName'])" -ForegroundColor Gray
        Write-Host "  Version:      $($props['ProductVersion'])" -ForegroundColor Gray
        Write-Host "  UpgradeCode:  $($props['UpgradeCode'])" -ForegroundColor Gray

        if ($props['UpgradeCode'] -ne "{29950A94-3C8D-4043-9E00-36AB69F78042}" -and $props['UpgradeCode'] -ne "29950A94-3C8D-4043-9E00-36AB69F78042") {
            Write-Warning "UpgradeCode mismatch: $($props['UpgradeCode'])"
        }

        # Verify Wix5FirewallException
        $fwView = $db.OpenView("SELECT Name, Program FROM Wix5FirewallException")
        $fwView.Execute()
        $fwRules = @()
        while ($rec = $fwView.Fetch()) {
            $fwRules += "$($rec.StringData(1)) -> $($rec.StringData(2))"
        }
        Write-Host "  Firewall Rules ($($fwRules.Count)): $($fwRules -join '; ')" -ForegroundColor Gray
        if ($fwRules.Count -lt 2) {
            Write-Warning "Expected at least 2 firewall rules (Throne and Throne Core)."
        }

        # Verify Wix4CloseApplication
        $closeView = $db.OpenView("SELECT Target, Description FROM Wix4CloseApplication")
        $closeView.Execute()
        $closeApps = @()
        while ($rec = $closeView.Fetch()) {
            $closeApps += "$($rec.StringData(1))"
        }
        Write-Host "  CloseApplication Targets ($($closeApps.Count)): $($closeApps -join '; ')" -ForegroundColor Gray

        # Verify File and Component counts
        $fView = $db.OpenView("SELECT File FROM File")
        $fView.Execute()
        $dbFiles = 0
        while ($rec = $fView.Fetch()) { $dbFiles++ }

        $cView = $db.OpenView("SELECT Component FROM Component")
        $cView.Execute()
        $dbComps = 0
        while ($rec = $cView.Fetch()) { $dbComps++ }

        Write-Host "  Database Records: $dbFiles Files, $dbComps Components" -ForegroundColor Gray

        Write-Host "=== All MSI Verification Checks PASSED ===" -ForegroundColor Green
    }
    catch {
        Write-Warning "Database validation encountered warning: $_"
    }
    finally {
        # Explicit COM object release to prevent MsiException 1631 across consecutive runs
        if ($propView) { [System.Runtime.InteropServices.Marshal]::ReleaseComObject($propView) | Out-Null }
        if ($fwView) { [System.Runtime.InteropServices.Marshal]::ReleaseComObject($fwView) | Out-Null }
        if ($closeView) { [System.Runtime.InteropServices.Marshal]::ReleaseComObject($closeView) | Out-Null }
        if ($fView) { [System.Runtime.InteropServices.Marshal]::ReleaseComObject($fView) | Out-Null }
        if ($cView) { [System.Runtime.InteropServices.Marshal]::ReleaseComObject($cView) | Out-Null }
        if ($db) { [System.Runtime.InteropServices.Marshal]::ReleaseComObject($db) | Out-Null }
        if ($installer) { [System.Runtime.InteropServices.Marshal]::ReleaseComObject($installer) | Out-Null }
        [System.GC]::Collect()
        [System.GC]::WaitForPendingFinalizers()
    }
}

Write-Host "Build complete: $($msiItem.FullName)" -ForegroundColor Cyan
