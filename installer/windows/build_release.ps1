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

Write-Host "Building AEGIS CLI..."
Push-Location (Join-Path $RepoRoot "cli")
cargo build --release
Pop-Location

Write-Host "Building AEGIS Rust engine..."
Push-Location (Join-Path $RepoRoot "engine-rust")
cargo build --release
Pop-Location

Write-Host "Building AEGIS web UI..."
Push-Location (Join-Path $RepoRoot "frontend")
npm ci
npm run build
Pop-Location

Write-Host "Packaging Windows runtime artifacts..."
& (Join-Path $ScriptDir "package_runtime.ps1") `
    -Version $Version `
    -ReleaseBaseUrl $ReleaseBaseUrl `
    -OllamaInstallerUrl $OllamaInstallerUrl `
    -OllamaInstallerSha256 $OllamaInstallerSha256 `
    -MinimumSupportedWindows $MinimumSupportedWindows `
    -DefaultModel $DefaultModel `
    -PythonStandaloneUrl $PythonStandaloneUrl `
    -PythonStandaloneSha256 $PythonStandaloneSha256 `
    -PythonZipPath $PythonZipPath
