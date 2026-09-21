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
#   powershell -ExecutionPolicy Bypass -File scripts/publish-release.ps1 -Target (git rev-parse HEAD)

param(
    [string]$Notes = "",
    [string]$KeyPath = "$env:USERPROFILE\.tauri\moviebox_updater.key",
    # The commit (or branch) the release tag should point at. Without it GitHub tags the
    # default branch's head, which is the wrong code whenever the release was built from
    # a feature branch -- the v1.3.2 tag first landed on an old commit of main, not the code that shipped.
    [string]$Target = "",
    [switch]$SkipBuild,
    # Sign and write release\latest.json, then stop before anything reaches GitHub. The
    # auto-update workflow rehearses with this, using a throwaway key, on every dry run.
    [switch]$DryRun
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
        throw "MovieBox is running. Close it first - it holds libmpv-2.dll open and the build will fail."
    }
    Write-Host "Building (this takes a few minutes)..."
    npm run tauri build
    if ($LASTEXITCODE -ne 0) { throw "The build failed." }
    powershell -ExecutionPolicy Bypass -File "$root\scripts\make-portable.ps1"
}

# Signing is a separate step, and `createUpdaterArtifacts` is off in tauri.conf.json,
# on purpose: handing the key to `tauri build` through the environment makes it stop
# and ask for the key password, which nothing can answer in a non-interactive shell
# (PowerShell cannot hold an empty environment variable). The updater only needs the
# installer plus this .sig, which is exactly what these two steps produce.
$setupForSigning = "src-tauri\target\release\bundle\nsis\MovieBox_${version}_x64-setup.exe"
if (-not (Test-Path $setupForSigning)) { throw "No installer at $setupForSigning." }
# Through cmd on purpose: PowerShell drops an empty "" argument, and the signer needs
# one for -p because the key has no password.
cmd /c "npx tauri signer sign -f ""$KeyPath"" -p """" ""$setupForSigning"""
if ($LASTEXITCODE -ne 0) { throw "Signing the installer failed." }

$setup = "src-tauri\target\release\bundle\nsis\MovieBox_${version}_x64-setup.exe"
$sig = "$setup.sig"
foreach ($f in @($setup, $sig)) {
    if (-not (Test-Path $f)) { throw "Missing $f." }
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
# WriteAllText, not Set-Content: PowerShell 5.1 would add a UTF-8 BOM and the
# updater's JSON parser rejects the file outright.
[System.IO.File]::WriteAllText($latestPath, ($latest | ConvertTo-Json -Depth 5))
Write-Host "Wrote $latestPath"

if ($DryRun) {
    Write-Host "Dry run: signed $setup and wrote the feed, but nothing was uploaded."
    return
}

$assets = @($setup, $sig, $latestPath)
$portable = "release\MovieBox_${version}_x64_portable.zip"
if (Test-Path $portable) { $assets += $portable }

# cmd swallows gh's output cleanly; PowerShell 5.1 turns native stderr into an error.
cmd /c "gh release view $tag >nul 2>&1"
if ($LASTEXITCODE -eq 0) {
    Write-Host "Release $tag exists - replacing its files."
    gh release upload $tag $assets --clobber
} else {
    if ($Target) {
        gh release create $tag $assets --title "MovieBox $version" --notes $notesText --target $Target
    } else {
        gh release create $tag $assets --title "MovieBox $version" --notes $notesText
    }
}
if ($LASTEXITCODE -ne 0) { throw "Publishing to GitHub failed." }

Write-Host ""
Write-Host "Done. Installed copies of MovieBox will offer $version the next time they open."
Write-Host "Check the feed: https://github.com/vikasdigitalcreations/movies/releases/latest/download/latest.json"
