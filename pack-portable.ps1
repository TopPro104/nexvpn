#requires -Version 5.1
<#
Packs a portable Windows build of NexVPN.
Run AFTER `npm run tauri build` has produced src-tauri/target/release/nexvpn.exe.

Output: dist-portable/NexVPN_<version>_portable_x64.zip
#>

$ErrorActionPreference = "Stop"
Set-Location -Path $PSScriptRoot

$pkg = Get-Content "package.json" -Raw | ConvertFrom-Json
$version = $pkg.version
$releaseDir = "src-tauri/target/release"
$exe = Join-Path $releaseDir "nexvpn.exe"

if (-not (Test-Path $exe)) {
    Write-Host "nexvpn.exe not found at $exe — run 'npm run tauri build' first." -ForegroundColor Red
    exit 1
}

$stageRoot = "dist-portable"
$stageName = "NexVPN_${version}_portable_x64"
$stage = Join-Path $stageRoot $stageName
$zipPath = Join-Path $stageRoot "$stageName.zip"

if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
if (Test-Path $zipPath) { Remove-Item $zipPath -Force }
New-Item -ItemType Directory -Path $stage -Force | Out-Null

# Main binary + sidecars (sing-box, xray) + wintun (Windows TUN driver)
$files = @("nexvpn.exe", "sing-box.exe", "xray.exe", "wintun.dll")
$fallbackDir = "src-tauri/binaries"
foreach ($f in $files) {
    $src = Join-Path $releaseDir $f
    if (-not (Test-Path $src)) {
        $alt = Join-Path $fallbackDir $f
        if (Test-Path $alt) { $src = $alt }
    }
    if (-not (Test-Path $src)) {
        Write-Host "Missing: $f" -ForegroundColor Yellow
        continue
    }
    Copy-Item $src -Destination $stage
}

# Tiny readme
@"
NexVPN $version (portable)

Run nexvpn.exe directly — no installer needed.
Settings are stored in %APPDATA%\nexvpn (same as the installed build).
TUN mode requires running as Administrator.
"@ | Out-File -FilePath (Join-Path $stage "README.txt") -Encoding utf8

Compress-Archive -Path "$stage/*" -DestinationPath $zipPath -CompressionLevel Optimal
Write-Host ""
Write-Host "Built portable archive:" -ForegroundColor Green
Write-Host "  $zipPath"
$size = (Get-Item $zipPath).Length / 1MB
Write-Host ("  {0:N2} MB" -f $size)
