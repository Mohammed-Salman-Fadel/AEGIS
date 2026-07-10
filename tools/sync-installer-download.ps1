$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")
$InstallerPath = Join-Path $RepoRoot "installer\AEGIS-Windows-x64.exe"
$PublicDownloadsDir = Join-Path $RepoRoot "landing page\public\downloads"
$PublicDownloadPath = Join-Path $PublicDownloadsDir "AEGIS-Windows-x64.exe"

if (-not (Test-Path -LiteralPath $InstallerPath -PathType Leaf)) {
    throw "Installer binary was not found at `$InstallerPath`."
}

New-Item -ItemType Directory -Force -Path $PublicDownloadsDir | Out-Null
Copy-Item -LiteralPath $InstallerPath -Destination $PublicDownloadPath -Force

$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $PublicDownloadPath).Hash
Write-Host "Synced installer download: $PublicDownloadPath"
Write-Host "SHA256: $hash"
