# Downloads the ffmpeg binary MovieBox bundles as a Tauri sidecar.
#
# MovieBox's servers only hand out DASH streams (audio and video arrive as separate
# tracks), so a download has to be muxed back into one file. ffmpeg does that.
#
# The LGPL build is used deliberately: it leaves out the GPL-only parts, which is what
# lets us ship it beside a closed app. Source: https://github.com/BtbN/FFmpeg-Builds
#
# Run once after cloning:  powershell -ExecutionPolicy Bypass -File scripts/fetch-ffmpeg.ps1

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$binDir = Join-Path $root "src-tauri\bin"
$target = Join-Path $binDir "ffmpeg-x86_64-pc-windows-msvc.exe"
$url = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-lgpl.zip"

if (Test-Path $target) {
    $mb = [math]::Round((Get-Item $target).Length / 1MB, 1)
    Write-Host "ffmpeg is already here ($mb MB): $target"
    Write-Host "Delete it and re-run this script to refresh it."
    exit 0
}

New-Item -ItemType Directory -Force -Path $binDir | Out-Null
$tmp = Join-Path $env:TEMP "moviebox-ffmpeg"
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
$zip = Join-Path $tmp "ffmpeg.zip"

Write-Host "Downloading ffmpeg (static LGPL build, ~40 MB)..."
Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing

Write-Host "Extracting..."
$extract = Join-Path $tmp "x"
if (Test-Path $extract) { Remove-Item -Recurse -Force $extract }
Expand-Archive -Path $zip -DestinationPath $extract -Force

$exe = Get-ChildItem -Path $extract -Recurse -Filter "ffmpeg.exe" | Select-Object -First 1
if (-not $exe) { throw "ffmpeg.exe was not found inside the archive" }
Copy-Item $exe.FullName $target -Force

# The static build is one self-contained exe: nothing else has to ship with it.

Remove-Item -Recurse -Force $tmp
$mb = [math]::Round((Get-Item $target).Length / 1MB, 1)
Write-Host "ffmpeg ready ($mb MB): $target"
Write-Host "It is bundled as a Tauri sidecar, so it installs beside MovieBox.exe."
