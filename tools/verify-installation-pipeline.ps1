param(
    [switch]$SkipLandingBuild,
    [switch]$RequireInstallerSource
)

$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$InstallerPath = Join-Path $RepoRoot "installer\AEGIS-Windows-x64.exe"
$InstallerDir = Join-Path $RepoRoot "installer"
$LandingRoot = Join-Path $RepoRoot "landing page"
$CliRoot = Join-Path $RepoRoot "cli"
$ReleaseVerifier = Join-Path $RepoRoot "tools\verify-installer-release.ps1"

function Assert-FileExists($Path, $Label) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "$Label was not found at `$Path`."
    }
}

function Invoke-Step($Label, $ScriptBlock) {
    Write-Host ""
    Write-Host "==> $Label"
    & $ScriptBlock
}

Assert-FileExists $InstallerPath "Installer binary"
Assert-FileExists $ReleaseVerifier "Installer release verifier"

Invoke-Step "Verifying landing page installer handoff" {
    $args = @()
    if ($SkipLandingBuild) {
        $args += "-SkipBuild"
    }
    & powershell -ExecutionPolicy Bypass -File $ReleaseVerifier @args
}

Invoke-Step "Verifying CLI exposes aegis open" {
    Push-Location $CliRoot
    try {
        $previousErrorActionPreference = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        $help = & cargo run --quiet -- --help 2>&1
        $exitCode = $LASTEXITCODE
        $ErrorActionPreference = $previousErrorActionPreference

        if ($exitCode -ne 0) {
            throw "Could not run CLI help. Output:`n$help"
        }
        if (($help -join "`n") -notmatch "\bopen\b") {
            throw "CLI help does not expose the `aegis open` command."
        }
        Write-Host "CLI command available: aegis open"
    }
    finally {
        if ($previousErrorActionPreference) {
            $ErrorActionPreference = $previousErrorActionPreference
        }
        Pop-Location
    }
}

Invoke-Step "Checking installer source availability" {
    $sourcePatterns = @("*.nsi", "*.iss", "*.wxs", "*.wixproj", "package.json", "Cargo.toml")
    $sourceFiles = foreach ($pattern in $sourcePatterns) {
        Get-ChildItem -LiteralPath $InstallerDir -Filter $pattern -File -ErrorAction SilentlyContinue
    }

    if (-not $sourceFiles) {
        $message = "No installer source/build recipe was found in '$InstallerDir'. The binary handoff can be verified, but dependency-download behavior cannot be audited or rebuilt from this repository."
        if ($RequireInstallerSource) {
            throw $message
        }
        Write-Warning $message
        return
    }

    Write-Host "Installer source/build recipe found:"
    $sourceFiles | ForEach-Object { Write-Host " - $($_.FullName)" }
}

Write-Host ""
Write-Host "Installation pipeline verification completed."
