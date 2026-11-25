param(
    [string]$ThroneExe = "",
    [string]$ThroneCoreExe = "",
    [string]$RustDll = "",
    [string]$UpdaterExe = "",
    [string]$CronetDll = "",
    [string]$DashboardDir = "",
    [string]$Version = "1.4.2.1",
    [string]$OutMsi = ""
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot
if (-not $ThroneExe) { $ThroneExe = Join-Path $RepoRoot "build\Throne.exe" }
if (-not $ThroneCoreExe) { $ThroneCoreExe = Join-Path $RepoRoot "core\ThroneCore.exe" }
if (-not $RustDll) { $RustDll = Join-Path $RepoRoot "throne_fast_core\target\release\throne_fast_core.dll" }
if (-not $DashboardDir) { $DashboardDir = Join-Path $RepoRoot "res\dashboard" }
if (-not $OutMsi) { $OutMsi = Join-Path $PSScriptRoot "Throne-1.4.2-rust-x64-Setup.msi" }

if (-not $CronetDll) {
    $goPkg = Join-Path $env:USERPROFILE "go\pkg\mod\github.com\sagernet\cronet-go\lib"
    if (Test-Path $goPkg) {
        $found = Get-ChildItem $goPkg -Recurse -Filter "libcronet.dll" | Where-Object { $_.FullName -like "*windows_amd64*" } | Select-Object -First 1
        if ($found) { $CronetDll = $found.FullName }
    }
}

foreach ($req in @(
    @{ Name = "Throne.exe"; Path = $ThroneExe },
    @{ Name = "ThroneCore.exe"; Path = $ThroneCoreExe },
    @{ Name = "throne_fast_core.dll"; Path = $RustDll },
    @{ Name = "libcronet.dll"; Path = $CronetDll }
)) {
    if (-not $req.Path -or -not (Test-Path $req.Path)) {
        throw "Required payload '$($req.Name)' not found at '$($req.Path)'"
    }
}

$StageDir = Join-Path $PSScriptRoot "stage"
if (Test-Path $StageDir) { Remove-Item -Recurse -Force $StageDir }
New-Item -ItemType Directory -Force $StageDir | Out-Null

Copy-Item $ThroneExe (Join-Path $StageDir "Throne.exe") -Force
Copy-Item $ThroneCoreExe (Join-Path $StageDir "ThroneCore.exe") -Force
Copy-Item $RustDll (Join-Path $StageDir "throne_fast_core.dll") -Force
Copy-Item $CronetDll (Join-Path $StageDir "libcronet.dll") -Force
if ($UpdaterExe -and (Test-Path $UpdaterExe)) {
    Copy-Item $UpdaterExe (Join-Path $StageDir "updater.exe") -Force
}

if ($DashboardDir -and (Test-Path $DashboardDir)) {
    $dashDest = Join-Path $StageDir "dashboard"
    New-Item -ItemType Directory -Force $dashDest | Out-Null
    Copy-Item "$DashboardDir\*" $dashDest -Recurse -Force
    Get-ChildItem $dashDest -Filter ".gitkeep" -Recurse | Remove-Item -Force
}

$WxsFile = Join-Path $PSScriptRoot "Package.gen.wxs"
$IconPath = Join-Path $RepoRoot "res\Throne.ico"
$LicensePath = Join-Path $PSScriptRoot "License.rtf"

python (Join-Path $PSScriptRoot "generate_wxs.py") `
    --stage $StageDir `
    --out $WxsFile `
    --version $Version `
    --icon $IconPath `
    --license $LicensePath
if ($LASTEXITCODE -ne 0) { throw "generate_wxs.py failed with exit $LASTEXITCODE" }

$wixCmd = Get-Command "wix.exe" -ErrorAction SilentlyContinue
if (-not $wixCmd) {
    $wixCandidate = Join-Path $env:USERPROFILE ".dotnet\tools\wix.exe"
    if (Test-Path $wixCandidate) { $wixCmd = $wixCandidate } else { throw "wix.exe not found" }
} else {
    $wixCmd = $wixCmd.Source
}

& $wixCmd extension list -g 2>&1 | Out-String | Out-Null
& $wixCmd build $WxsFile -ext WixToolset.UI.wixext -arch x64 -out $OutMsi
if ($LASTEXITCODE -ne 0) { throw "wix build failed with exit $LASTEXITCODE" }

$hash = (Get-FileHash $OutMsi -Algorithm SHA256).Hash
$sizeMB = [math]::Round((Get-Item $OutMsi).Length / 1MB, 2)
Write-Host "MSI built successfully: $OutMsi ($sizeMB MB)"
Write-Host "SHA256: $hash"
