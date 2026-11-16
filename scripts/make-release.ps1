# Builds latest.json + checksums.txt next to the NSIS installer, ready to upload to a
# GitHub release. Run after `pnpm tauri build` (which needs TAURI_SIGNING_PRIVATE_KEY set).
# Usage:  powershell -File scripts\make-release.ps1
param(
    [string]$BundleDir = "target\release\bundle\nsis",
    [string]$OutDir = "target\release\bundle\release",
    [string]$Repo = "hpmediaoffifical/hp-screen"
)
$ErrorActionPreference = "Stop"

$setup = Get-ChildItem "$BundleDir\*-setup.exe" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $setup) { throw "No *-setup.exe in $BundleDir. Build first: pnpm tauri build" }
$sig = "$($setup.FullName).sig"
if (-not (Test-Path $sig)) { throw "Missing $sig - the updater signature. Set TAURI_SIGNING_PRIVATE_KEY when building." }

$conf = Get-Content "src-tauri\tauri.conf.json" -Raw | ConvertFrom-Json
$version = $conf.version
$tag = "v$version"
$signature = (Get-Content $sig -Raw).Trim()
$now = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
$url = "https://github.com/$Repo/releases/download/$tag/$([Uri]::EscapeDataString($setup.Name))"

$release = [ordered]@{
    version  = $version
    notes    = "HP Screen $version"
    pub_date = $now
    platforms = [ordered]@{
        "windows-x86_64" = [ordered]@{
            signature = $signature
            url       = $url
        }
    }
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText("$PWD\$OutDir\latest.json", ($release | ConvertTo-Json -Depth 5), $utf8NoBom)
$hash = (Get-FileHash $setup.FullName -Algorithm SHA256).Hash
[System.IO.File]::WriteAllText("$PWD\$OutDir\checksums.txt", "SHA-256 ($($setup.Name)): $hash", $utf8NoBom)
Write-Host "Wrote $OutDir\latest.json and checksums.txt"
Write-Host "  version:  $version"
Write-Host "  url:      $url"
Write-Host "  sha256:   $hash"
Write-Host "Upload to the GitHub release '$tag' along with the setup exe."
