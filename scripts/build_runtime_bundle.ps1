param(
    [string]$Version = "0.1.0",
    [string]$ReleaseBaseUrl = "",
    [string]$OllamaInstallerUrl = "",
    [string]$OllamaInstallerSha256 = "",
    [string]$MinimumSupportedWindows = "Windows 10 x64",
    [string]$DefaultModel = "qwen3:4b"
)

$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$DistDir = Join-Path $RepoRoot "dist"
$RuntimeStage = Join-Path $DistDir "runtime"
$RuntimeZip = Join-Path $DistDir "aegis-runtime-windows-x64.zip"
$BootstrapExe = Join-Path $DistDir "aegis-bootstrap-windows-x64.exe"
$ManifestPath = Join-Path $DistDir "installer-manifest.json"

$CliExe = Join-Path $RepoRoot "cli\target\release\aegis.exe"
$EngineExe = Join-Path $RepoRoot "engine-rust\target\release\aegis-engine.exe"
$FrontendDist = Join-Path $RepoRoot "frontend\dist"

if (-not (Test-Path $CliExe)) {
    throw "Missing CLI binary at $CliExe. Build cli\target\release\aegis.exe first."
}

if (-not (Test-Path $EngineExe)) {
    throw "Missing engine binary at $EngineExe. Build engine-rust\target\release\aegis-engine.exe first."
}

if (-not (Test-Path (Join-Path $FrontendDist "index.html"))) {
    throw "Missing built frontend assets at $FrontendDist. Run the frontend release build first."
}

if ([string]::IsNullOrWhiteSpace($ReleaseBaseUrl)) {
    $ReleaseBaseUrl = "https://github.com/aegis-project/AEGIS/releases/latest/download"
}

New-Item -ItemType Directory -Force -Path $DistDir | Out-Null
if (Test-Path $RuntimeStage) {
    Remove-Item -Recurse -Force -LiteralPath $RuntimeStage
}

$RuntimeBin = Join-Path $RuntimeStage "bin"
$RuntimeUi = Join-Path $RuntimeStage "ui"
$RuntimeConfig = Join-Path $RuntimeStage "config"

New-Item -ItemType Directory -Force -Path $RuntimeBin, $RuntimeUi, $RuntimeConfig | Out-Null

Copy-Item -LiteralPath $CliExe -Destination (Join-Path $RuntimeBin "aegis.exe") -Force
Copy-Item -LiteralPath $EngineExe -Destination (Join-Path $RuntimeBin "aegis-engine.exe") -Force
Copy-Item -Path (Join-Path $FrontendDist "*") -Destination $RuntimeUi -Recurse -Force

@"
AEGIS_INFERENCE_PROVIDER=ollama
AEGIS_OLLAMA_URL=http://127.0.0.1:11434
AEGIS_MODEL=$DefaultModel
AEGIS_ENGINE_HOST=127.0.0.1
AEGIS_ENGINE_PORT=8080
"@ | Set-Content -LiteralPath (Join-Path $RuntimeConfig "default.env") -Encoding ASCII

Set-Content -LiteralPath (Join-Path $RuntimeStage "version.txt") -Value $Version -Encoding ASCII

if (Test-Path $RuntimeZip) {
    Remove-Item -Force -LiteralPath $RuntimeZip
}

Compress-Archive -Path (Join-Path $RuntimeStage "*") -DestinationPath $RuntimeZip -Force
Copy-Item -LiteralPath $CliExe -Destination $BootstrapExe -Force

$RuntimeSha = (Get-FileHash -LiteralPath $RuntimeZip -Algorithm SHA256).Hash.ToLowerInvariant()

$Manifest = [ordered]@{
    version = $Version
    runtime_url = "$ReleaseBaseUrl/aegis-runtime-windows-x64.zip"
    runtime_sha256 = $RuntimeSha
    ollama_installer_url = $OllamaInstallerUrl
    ollama_installer_sha256 = $OllamaInstallerSha256
    default_model = $DefaultModel
    engine_host = "127.0.0.1"
    engine_port = 8080
    ui_url = "http://localhost:8080"
    minimum_supported_windows = $MinimumSupportedWindows
}

$Manifest | ConvertTo-Json | Set-Content -LiteralPath $ManifestPath -Encoding ASCII

Write-Host "Created:"
Write-Host "  $BootstrapExe"
Write-Host "  $RuntimeZip"
Write-Host "  $ManifestPath"
