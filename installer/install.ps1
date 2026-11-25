param(
    [string]$Repo = "IMAN-Py/Throne-Rust",
    [string]$MsiName = "Throne-1.4.2-rust-x64-Setup.msi",
    [switch]$Silent
)

$ErrorActionPreference = "Stop"

$workDir = Join-Path $env:TEMP "Throne-Rust-Setup"
New-Item -ItemType Directory -Force $workDir | Out-Null

$msiUrl = "https://github.com/$Repo/releases/latest/download/$MsiName"
$sumsUrl = "https://github.com/$Repo/releases/latest/download/SHA256SUMS.txt"
$msiPath = Join-Path $workDir $MsiName
$sumsPath = Join-Path $workDir "SHA256SUMS.txt"

Write-Host "Downloading $MsiName and SHA256SUMS.txt from $Repo..."
Invoke-WebRequest -Uri $sumsUrl -OutFile $sumsPath -UseBasicParsing
Invoke-WebRequest -Uri $msiUrl -OutFile $msiPath -UseBasicParsing

$expectedLine = Get-Content $sumsPath | Where-Object { $_ -match [regex]::Escape($MsiName) } | Select-Object -First 1
if (-not $expectedLine) {
    throw "Could not find SHA-256 entry for $MsiName in SHA256SUMS.txt"
}
$expectedHash = ($expectedLine -split '\s+')[0].Trim().ToLower()
$actualHash = (Get-FileHash -Path $msiPath -Algorithm SHA256).Hash.ToLower()

if ($actualHash -ne $expectedHash) {
    Remove-Item -Force $msiPath -ErrorAction SilentlyContinue
    throw "SHA-256 verification FAILED! Expected $expectedHash, got $actualHash"
}

Write-Host "SHA-256 verified: $actualHash"
$msiArgs = if ($Silent) { @("/i", "`"$msiPath`"", "/qn", "/norestart") } else { @("/i", "`"$msiPath`"") }
Start-Process msiexec.exe -ArgumentList $msiArgs -Wait
