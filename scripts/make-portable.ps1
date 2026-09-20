# Builds MovieBox_<version>_x64_portable.zip from the release build output.
# Run after `npm run tauri build`.
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$rel = Join-Path $root "src-tauri\target\release"
$out = Join-Path $root "release"
$stage = Join-Path $out "MovieBox"
$version = (Get-Content (Join-Path $root "src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json).version

if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force -Path $stage, (Join-Path $stage "lib"), (Join-Path $stage "docs") | Out-Null

Copy-Item (Join-Path $rel "moviebox.exe") (Join-Path $stage "MovieBox.exe")
Copy-Item (Join-Path $root "src-tauri\lib\*.dll") (Join-Path $stage "lib")
Copy-Item (Join-Path $root "src-tauri\docs\*") (Join-Path $stage "docs")
# The sidecar that joins downloaded video and audio into one file.
Copy-Item (Join-Path $rel "ffmpeg.exe") (Join-Path $stage "ffmpeg.exe")

@"
MovieBox (portable)
===================

1. Unzip this folder anywhere (Desktop, USB stick...).
2. Double-click MovieBox.exe.
3. If Windows shows "Windows protected your PC", click "More info" and then "Run anyway".

Keep the "lib" folder and ffmpeg.exe next to MovieBox.exe - they are the video player
and the tool that joins finished downloads into one file.
MovieBox needs Microsoft Edge WebView2, which is already part of Windows 10 and 11.
If the app window stays blank, install WebView2 from https://go.microsoft.com/fwlink/p/?LinkId=2124703
or use the normal installer (MovieBox_${version}_x64-setup.exe), which installs it automatically.

See docs\Getting Started.html for a short guide.
"@ | Set-Content -Encoding UTF8 (Join-Path $stage "READ ME FIRST.txt")

$zip = Join-Path $out "MovieBox_${version}_x64_portable.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path $stage -DestinationPath $zip -CompressionLevel Optimal

$setup = Get-ChildItem (Join-Path $rel "bundle\nsis") -Filter "*.exe" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if ($setup) { Copy-Item $setup.FullName (Join-Path $out $setup.Name) -Force }

Get-ChildItem $out -File | Select-Object Name, @{n = "MB"; e = { [math]::Round($_.Length / 1MB, 1) } }
