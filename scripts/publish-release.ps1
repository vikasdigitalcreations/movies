# Builds MovieBox and publishes it as a GitHub release, so installed copies update themselves.
#
# The app checks this on every launch:
#   https://github.com/vikasdigitalcreations/movies/releases/latest/download/latest.json
# That URL has to be readable without a login, which is why the repo is public.
#
# Needs: the updater signing key (created once with `npx tauri signer generate`) and
# the GitHub CLI, logged in.
#
#   powershell -ExecutionPolicy Bypass -File scripts/publish-release.ps1
#   powershell -ExecutionPolicy Bypass -File scripts/publish-release.ps1 -Notes "What changed"

param(
    [string]$Notes = "",
    [string]$KeyPath = "$env:USERPROFILE\.tauri\moviebox_updater.key",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if (-not (Test-Path $KeyPath)) {
    throw "Updater signing key not found at $KeyPath. Without it, installed copies will refuse the update. Create one with: npx tauri signer generate -w `"$KeyPath`""
}

$conf = Get-Content "src-tauri\tauri.conf.json" -Raw | ConvertFrom-Json
$version = $conf.version
$tag = "v$version"
Write-Host "Publishing MovieBox $version"

if (-not $SkipBuild) {
    if (Get-Process -Name "MovieBox", "moviebox" -ErrorAction SilentlyContinue) {
        throw "MovieBox is running. Close it first — it holds libmpv-2.dll open and the build will fail."
    }
    $env:TAURI_SIGNING_PRIVATE_KEY_PATH = $KeyPath
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
    Write-Host "Building (this takes a few minutes)..."
    npm run tauri build
    if ($LASTEXITCODE -ne 0) { throw "The build failed." }
    powershell -ExecutionPolicy Bypass -File "$root\scripts\make-portable.ps1"
}

$setup = "src-tauri\target\release\bundle\nsis\MovieBox_${version}_x64-setup.exe"
$sig = "$setup.sig"
foreach ($f in @($setup, $sig)) {
    if (-not (Test-Path $f)) { throw "Missing $f. Was the build run with the signing key set?" }
}

$notesText = if ($Notes) { $Notes } else { "MovieBox $version" }
$latest = [ordered]@{
    version   = $version
    notes     = $notesText
    pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    platforms = [ordered]@{
        "windows-x86_64" = [ordered]@{
            signature = (Get-Content $sig -Raw).Trim()
            url       = "https://github.com/vikasdigitalcreations/movies/releases/download/$tag/MovieBox_${version}_x64-setup.exe"
        }
    }
}
$latestPath = Join-Path $root "release\latest.json"
New-Item -ItemType Directory -Force -Path (Split-Path $latestPath) | Out-Null
$latest | ConvertTo-Json -Depth 5 | Set-Content $latestPath -Encoding utf8
Write-Host "Wrote $latestPath"

$assets = @($setup, $sig, $latestPath)
$portable = "release\MovieBox_${version}_x64_portable.zip"
if (Test-Path $portable) { $assets += $portable }

if (gh release view $tag 2>$null) {
    Write-Host "Release $tag exists — replacing its files."
    gh release upload $tag $assets --clobber
} else {
    gh release create $tag $assets --title "MovieBox $version" --notes $notesText
}
if ($LASTEXITCODE -ne 0) { throw "Publishing to GitHub failed." }

Write-Host ""
Write-Host "Done. Installed copies of MovieBox will offer $version the next time they open."
Write-Host "Check the feed: https://github.com/vikasdigitalcreations/movies/releases/latest/download/latest.json"
