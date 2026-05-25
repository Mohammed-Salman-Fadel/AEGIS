# Testing

This folder collects cross-project test coverage that does not naturally live inside a single crate.

- `python/`: runnable `unittest` suites for the RAG utilities.
- `run-tests.cmd`: Windows-friendly entrypoint for the Python suite plus the Rust crate tests.
- `run-tests.ps1`: underlying PowerShell runner.
- `run-system-tests.cmd`: API-level smoke test plus performance benchmark.

Rust unit tests that Cargo discovers automatically still live next to the code they verify:

- `cli/src/ui.rs`
- `engine/src/user_profile.rs`

## Test Types Added

- Unit tests for text chunking edge cases.
- Unit tests for deterministic fallback embeddings.
- Integration-style tests that exercise chunking plus embedding together.
- API-level system smoke tests for the local RAG workflow.
- Performance benchmarks that report average latency and throughput.
- Rust helper tests for response normalization and profile-note parsing.

## Run

```powershell
./testing/run-tests.cmd
./testing/run-system-tests.cmd
```

Or run the suites individually:

```powershell
powershell -ExecutionPolicy Bypass -File .\testing\run-tests.ps1
python -m unittest discover -s testing/python -p "test_*.py"
cargo test
```
