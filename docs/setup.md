# AEGIS Setup Guide

## 🚀 Quick Start (5 Minutes)

1. **Prepare Environment**:
   ```powershell
   python -m venv .venv
   .\.venv\Scripts\activate
   pip install -r requirements.txt
   ```

2. **Start RAG Service**:
   ```powershell
   cd rag-python
   python -m uvicorn app.main:app --port 8000
   ```

3. **Start AEGIS Engine**:
   ```powershell
   cd engine
   $env:AEGIS_MODEL="llama3.2:3b"
   cargo run
   ```

---

## 🛠️ Detailed Configuration

### Subsystem Prerequisites

| Tool | Requirement | Note |
| :--- | :--- | :--- |
| **RAG** | Python 3.10+ | Uses ChromaDB (local) by default. |
| **Zotero** | [Zotero Desktop](https://zotero.org) | Must be running for local library access. |
| **Semble** | `AEGIS_SEMBLE_PATH` | Set this env var to index a specific codebase. |
| **Inference** | [Ollama](https://ollama.com) | Default provider. Ensure your model is pulled. |

### Environment Variables

| Variable | Default | Purpose |
| :--- | :--- | :--- |
| `AEGIS_MODEL` | `llama3.2:3b` | The LLM to use for generation. |
| `AEGIS_SEMBLE_PATH` | Project Root | The folder Semble will index for Coder Mode. |
| `ZOTERO_LOCAL` | `true` | Set to `false` to use Zotero Web API instead. |
| `RAG_DATA_DIR` | `./data` | Where ChromaDB and indices are stored. |

### Troubleshooting
- **Low Similarity?** Ensure `sentence-transformers` is installed to avoid the hash-fallback.
- **Decoding Error?** Ensure both Python and Rust services are on the latest version.
- **Zotero not found?** Ensure the Zotero Desktop app is open.

---

> [!IMPORTANT]
> **Legacy Environment Cleanup**: If you have an old `rag-python/rag-env` folder, please delete it. AEGIS now uses the unified `.venv` in the root directory for all Python services to ensure version consistency.
