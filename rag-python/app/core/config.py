# Config and constants
import os
from pathlib import Path

# Paths
BASE_DIR = Path(__file__).resolve().parent.parent.parent
DATA_DIR = os.getenv("RAG_DATA_DIR", str(BASE_DIR / "data"))

# Chroma Settings
CHROMA_COLLECTION_NAME = "aegis_collection"

# Embedding Model
EMBEDDING_MODEL_NAME = "all-MiniLM-L6-v2"

# Chunking Settings
CHUNK_SIZE_WORDS = 500
CHUNK_OVERLAP_WORDS = 100

# API Settings
MAX_TOP_K = 10
