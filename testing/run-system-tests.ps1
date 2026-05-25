$ErrorActionPreference = "Stop"

Push-Location "$PSScriptRoot\.."
try {
    $env:PYTHONPATH = (Resolve-Path "rag-python").Path

    python -m unittest testing.python.test_rag_api_system
    python .\testing\performance\benchmark_rag_api.py
}
finally {
    Pop-Location
}
