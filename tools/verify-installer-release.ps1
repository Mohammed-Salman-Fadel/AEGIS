param(
    [switch]$SkipBuild,
    [switch]$RequireSignature
)

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")
$InstallerPath = Join-Path $RepoRoot "installer\AEGIS-Windows-x64.exe"
$LandingRoot = Join-Path $RepoRoot "landing page"
$PublicDownloadPath = Join-Path $LandingRoot "public\downloads\AEGIS-Windows-x64.exe"
$DistDownloadPath = Join-Path $LandingRoot "dist\downloads\AEGIS-Windows-x64.exe"
$DistIndexPath = Join-Path $LandingRoot "dist\index.html"

function Assert-FileExists($Path, $Label) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "$Label was not found at `$Path`."
    }
}

function Get-Sha256($Path) {
    (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash
}

Assert-FileExists $InstallerPath "Installer binary"
Assert-FileExists $PublicDownloadPath "Landing page public download"

$installerHash = Get-Sha256 $InstallerPath
$publicHash = Get-Sha256 $PublicDownloadPath

if ($installerHash -ne $publicHash) {
    throw "The public download does not match the installer binary.`nInstaller: $installerHash`nPublic:    $publicHash"
}

if (-not $SkipBuild) {
    Push-Location $LandingRoot
    try {
        npm.cmd run build
    } finally {
        Pop-Location
    }
}

Assert-FileExists $DistIndexPath "Built landing page"
Assert-FileExists $DistDownloadPath "Built installer download"

$distHash = Get-Sha256 $DistDownloadPath
if ($installerHash -ne $distHash) {
    throw "The built download does not match the installer binary.`nInstaller: $installerHash`nDist:      $distHash"
}

$distFiles = Get-ChildItem -LiteralPath (Join-Path $LandingRoot "dist") -Recurse -File
$downloadReference = $distFiles | Select-String -SimpleMatch "/downloads/AEGIS-Windows-x64.exe" -List
if (-not $downloadReference) {
    throw "The built landing page does not reference /downloads/AEGIS-Windows-x64.exe."
}

$placeholderReference = Select-String -LiteralPath $DistIndexPath -Pattern "placeholder|aria-disabled=`"true`"|Download installer binary" -List
if ($placeholderReference) {
    throw "The built landing page still contains placeholder or disabled download copy."
}

$signature = Get-AuthenticodeSignature -LiteralPath $InstallerPath
if ($signature.Status -ne "Valid") {
    $message = "Installer Authenticode signature status: $($signature.Status). $($signature.StatusMessage)"
    if ($RequireSignature) {
        throw $message
    }

    Write-Warning $message
} else {
    Write-Host "Installer signature: Valid"
}

Write-Host "Installer release verification passed."
Write-Host "SHA256: $installerHash"
