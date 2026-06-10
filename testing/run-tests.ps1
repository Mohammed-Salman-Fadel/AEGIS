$ErrorActionPreference = "Stop"

Push-Location "$PSScriptRoot\.."
try {
    $env:PYTHONPATH = (Resolve-Path "python-services").Path

    python -m unittest discover -s testing/python -p "test_*.py"

    Push-Location "cli"
    try {
        cargo test
    }
    finally {
        Pop-Location
    }

    Push-Location "engine"
    try {
        cargo test
    }
    finally {
        Pop-Location
    }
}
finally {
    Pop-Location
}
