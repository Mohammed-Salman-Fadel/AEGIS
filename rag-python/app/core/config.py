# Config and constants
import os
from pathlib import Path

# Paths
BASE_DIR = Path(__file__).resolve().parent.parent.parent
DATA_DIR = os.getenv("RAG_DATA_DIR", str(BASE_DIR / "data"))

# Chroma Settings
CHROMA_COLLECTION_NAME = "aegis_collection"

# Embedding Model
LOCAL_EMBEDDING_MODEL = BASE_DIR / "models" / "all-MiniLM-L6-v2"
EMBEDDING_MODEL_NAME = os.getenv(
    "AEGIS_RAG_EMBEDDING_MODEL",
    str(LOCAL_EMBEDDING_MODEL) if LOCAL_EMBEDDING_MODEL.exists() else "all-MiniLM-L6-v2",
)

# Chunking Settings
CHUNK_SIZE_WORDS = 500
CHUNK_OVERLAP_WORDS = 100

# API Settings
MAX_TOP_K = 10
