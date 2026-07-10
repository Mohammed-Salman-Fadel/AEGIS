# AEGIS Landing Page

This is the standalone public landing website for AEGIS. It is separate from the main AEGIS web app
in `frontend`.

## Development

```powershell
npm.cmd install
npm.cmd run dev -- --host 127.0.0.1 --port 5174
```

## Production Build

```powershell
npm.cmd run build
```

The build runs `../tools/sync-installer-download.ps1` first, so the public download is copied from
`../installer/AEGIS-Windows-x64.exe` before Vite emits `dist`.

Before publishing a build, verify the installer pipeline:

```powershell
npm.cmd run verify:release
```

Deploy the generated `dist` directory to any static host or CDN. In production, the site root is `/`,
the Windows binary is served from `/downloads/AEGIS-Windows-x64.exe`, and documentation files are
served from `/docs`.

For public releases, sign `../installer/AEGIS-Windows-x64.exe` with an Authenticode certificate before
building, then run:

```powershell
powershell -ExecutionPolicy Bypass -File ..\tools\verify-installer-release.ps1 -RequireSignature
```
