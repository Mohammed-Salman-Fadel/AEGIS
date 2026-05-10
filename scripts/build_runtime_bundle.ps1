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

$RepoRoot = Split-Path -Parent $PSScriptRoot
$PackageScript = Join-Path $RepoRoot "installer\windows\package_runtime.ps1"

& $PackageScript `
    -Version $Version `
    -ReleaseBaseUrl $ReleaseBaseUrl `
    -OllamaInstallerUrl $OllamaInstallerUrl `
    -OllamaInstallerSha256 $OllamaInstallerSha256 `
    -MinimumSupportedWindows $MinimumSupportedWindows `
    -DefaultModel $DefaultModel `
    -PythonStandaloneUrl $PythonStandaloneUrl `
    -PythonStandaloneSha256 $PythonStandaloneSha256 `
    -PythonZipPath $PythonZipPath
