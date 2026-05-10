# Windows Release Packaging

This folder owns the v1 Windows bootstrap packaging flow.

## Full Build

```powershell
.\installer\windows\build_release.ps1 `
  -Version "v0.1.0" `
  -ReleaseBaseUrl "https://github.com/<owner>/<repo>/releases/download/v0.1.0" `
  -OllamaInstallerUrl "<official-ollama-installer-url>" `
  -OllamaInstallerSha256 "<sha256>"
```

The script builds:

- `cli\target\release\aegis.exe`
- `engine-rust\target\release\aegis-engine.exe`
- `frontend\dist`
- `dist\aegis-bootstrap-windows-x64.exe`
- `dist\aegis-runtime-windows-x64.zip`
- `dist\installer-manifest.json`

## Package Only

Use `package_runtime.ps1` if the binaries and frontend already exist:

```powershell
.\installer\windows\package_runtime.ps1 `
  -Version "v0.1.0" `
  -ReleaseBaseUrl "https://github.com/<owner>/<repo>/releases/download/v0.1.0" `
  -OllamaInstallerUrl "<official-ollama-installer-url>" `
  -OllamaInstallerSha256 "<sha256>"
```

## Python Runtime

The runtime zip includes portable Python and an offline wheelhouse for `rag-python\requirements.txt`. You can provide either:

- `-PythonStandaloneUrl`, a downloadable portable Python archive.
- `-PythonStandaloneSha256`, the SHA-256 for that archive.
- `-PythonZipPath`, a local archive path for repeatable local packaging.

The installer creates `%LOCALAPPDATA%\AEGIS\runtime\rag-venv` from those bundled assets, so end users do not need Python installed.
