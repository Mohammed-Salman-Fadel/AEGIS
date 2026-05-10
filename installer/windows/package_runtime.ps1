param(
    [string]$Version = "0.1.0",
    [string]$ReleaseBaseUrl = "",
    [string]$OllamaInstallerUrl = "",
    [string]$OllamaInstallerSha256 = "",
    [string]$MinimumSupportedWindows = "Windows 10 x64",
    [string]$DefaultModel = "llama3.2:3b",
    [string]$PythonStandaloneUrl = "",
    [string]$PythonStandaloneSha256 = "",
    [string]$PythonZipPath = ""
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Split-Path -Parent (Split-Path -Parent $ScriptDir)
$DistDir = Join-Path $RepoRoot "dist"
$RuntimeStage = Join-Path $DistDir "runtime"
$RuntimeZip = Join-Path $DistDir "aegis-runtime-windows-x64.zip"
$BootstrapExe = Join-Path $DistDir "aegis-bootstrap-windows-x64.exe"
$ManifestPath = Join-Path $DistDir "installer-manifest.json"

$CliExe = Join-Path $RepoRoot "cli\target\release\aegis.exe"
$EngineExe = Join-Path $RepoRoot "engine-rust\target\release\aegis-engine.exe"
$FrontendDist = Join-Path $RepoRoot "frontend\dist"
$RagSource = Join-Path $RepoRoot "rag-python"

if (-not (Test-Path $CliExe)) {
    throw "Missing CLI binary at $CliExe. Run installer\windows\build_release.ps1 or build the CLI first."
}

if (-not (Test-Path $EngineExe)) {
    throw "Missing engine binary at $EngineExe. Run installer\windows\build_release.ps1 or build the engine first."
}

if (-not (Test-Path (Join-Path $FrontendDist "index.html"))) {
    throw "Missing built frontend assets at $FrontendDist. Run npm run build in frontend first."
}

if (-not (Test-Path (Join-Path $RagSource "app\main.py"))) {
    throw "Missing Python RAG app at $RagSource."
}

if ([string]::IsNullOrWhiteSpace($ReleaseBaseUrl)) {
    $ReleaseBaseUrl = "https://github.com/aegis-project/AEGIS/releases/latest/download"
}

if ([string]::IsNullOrWhiteSpace($PythonStandaloneUrl) -and [string]::IsNullOrWhiteSpace($PythonZipPath)) {
    $PythonStandaloneUrl = "https://github.com/indygreg/python-build-standalone/releases/download/20240107/cpython-3.11.7%2B20240107-x86_64-pc-windows-msvc-shared-install_only.tar.gz"
    if ([string]::IsNullOrWhiteSpace($PythonStandaloneSha256)) {
        $PythonStandaloneSha256 = "67077e6fa918e4f4fd60ba169820b00be7c390c497bf9bc9cab2c255ea8e6f3e"
    }
}

New-Item -ItemType Directory -Force -Path $DistDir | Out-Null
if (Test-Path $RuntimeStage) {
    Remove-Item -Recurse -Force -LiteralPath $RuntimeStage
}

$RuntimeBin = Join-Path $RuntimeStage "bin"
$RuntimeUi = Join-Path $RuntimeStage "ui"
$RuntimeConfig = Join-Path $RuntimeStage "config"
$RuntimeRag = Join-Path $RuntimeStage "rag"
$RuntimePython = Join-Path $RuntimeStage "python"

New-Item -ItemType Directory -Force -Path $RuntimeBin, $RuntimeUi, $RuntimeConfig, $RuntimeRag, $RuntimePython | Out-Null

Copy-Item -LiteralPath $CliExe -Destination (Join-Path $RuntimeBin "aegis.exe") -Force
Copy-Item -LiteralPath $EngineExe -Destination (Join-Path $RuntimeBin "aegis-engine.exe") -Force
Copy-Item -Path (Join-Path $FrontendDist "*") -Destination $RuntimeUi -Recurse -Force
Copy-Item -Path (Join-Path $RagSource "app") -Destination $RuntimeRag -Recurse -Force
Copy-Item -LiteralPath (Join-Path $RagSource "requirements.txt") -Destination (Join-Path $RuntimeRag "requirements.txt") -Force

$PythonArchive = if ([string]::IsNullOrWhiteSpace($PythonZipPath)) {
    Join-Path $DistDir "python-standalone-runtime"
} else {
    $PythonZipPath
}

if ([string]::IsNullOrWhiteSpace($PythonZipPath)) {
    Write-Host "Downloading portable Python runtime..."
    Invoke-WebRequest -Uri $PythonStandaloneUrl -OutFile $PythonArchive
}

if (-not [string]::IsNullOrWhiteSpace($PythonStandaloneSha256)) {
    $ActualPythonSha = (Get-FileHash -LiteralPath $PythonArchive -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($ActualPythonSha -ne $PythonStandaloneSha256.ToLowerInvariant()) {
        throw "Portable Python SHA-256 mismatch. Expected $PythonStandaloneSha256, got $ActualPythonSha."
    }
}

$PythonExtract = Join-Path $DistDir "python-extract"
if (Test-Path $PythonExtract) {
    Remove-Item -Recurse -Force -LiteralPath $PythonExtract
}
New-Item -ItemType Directory -Force -Path $PythonExtract | Out-Null

if ($PythonArchive.EndsWith(".zip")) {
    Expand-Archive -LiteralPath $PythonArchive -DestinationPath $PythonExtract -Force
} else {
    tar -xf $PythonArchive -C $PythonExtract
}

$PythonExe = Get-ChildItem -Path $PythonExtract -Filter python.exe -Recurse | Select-Object -First 1
if (-not $PythonExe) {
    throw "Portable Python archive did not contain python.exe."
}

$PythonRoot = Split-Path -Parent $PythonExe.FullName
Copy-Item -Path (Join-Path $PythonRoot "*") -Destination $RuntimePython -Recurse -Force

$BundledPythonExe = Join-Path $RuntimePython "python.exe"
if (-not (Test-Path $BundledPythonExe)) {
    throw "Bundled Python was not copied to $BundledPythonExe."
}

& $BundledPythonExe -m ensurepip --upgrade

$Wheelhouse = Join-Path $RuntimeRag "wheels"
New-Item -ItemType Directory -Force -Path $Wheelhouse | Out-Null
& $BundledPythonExe -m pip download --dest $Wheelhouse -r (Join-Path $RuntimeRag "requirements.txt")
& $BundledPythonExe -m pip install -r (Join-Path $RuntimeRag "requirements.txt")

$EmbeddingModelDir = Join-Path $RuntimeRag "models\all-MiniLM-L6-v2"
New-Item -ItemType Directory -Force -Path $EmbeddingModelDir | Out-Null
$ModelDownloadScript = "from sentence_transformers import SentenceTransformer; SentenceTransformer('all-MiniLM-L6-v2').save(r'$($EmbeddingModelDir.Replace('\', '\\'))')"
& $BundledPythonExe -c $ModelDownloadScript

@"
AEGIS_INFERENCE_PROVIDER=ollama
AEGIS_OLLAMA_URL=http://127.0.0.1:11434
AEGIS_MODEL=$DefaultModel
AEGIS_ENGINE_HOST=127.0.0.1
AEGIS_ENGINE_PORT=8080
AEGIS_UI_DIR=ui
AEGIS_RAG_URL=http://127.0.0.1:8000
RAG_DATA_DIR=data\rag
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
    rag_host = "127.0.0.1"
    rag_port = 8000
    rag_url = "http://127.0.0.1:8000"
}

$Manifest | ConvertTo-Json | Set-Content -LiteralPath $ManifestPath -Encoding ASCII

Write-Host "Created:"
Write-Host "  $BootstrapExe"
Write-Host "  $RuntimeZip"
Write-Host "  $ManifestPath"
