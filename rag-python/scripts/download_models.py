import os
import sys
from pathlib import Path

# Add app directory to path
sys.path.append(str(Path(__file__).resolve().parent.parent))

def download_models():
    print("AEGIS Model Seeder")
    print("-------------------")
    
    # 1. Standard Embedding Model
    try:
        from sentence_transformers import SentenceTransformer
        from app.core.config import EMBEDDING_MODEL_NAME, EMBEDDING_MODEL_PATH
        
        print(f"Downloading SentenceTransformer model: {EMBEDDING_MODEL_NAME}...")
        model = SentenceTransformer(EMBEDDING_MODEL_NAME)
        
        print(f"Saving model to local path: {EMBEDDING_MODEL_PATH}")
        os.makedirs(os.path.dirname(EMBEDDING_MODEL_PATH), exist_ok=True)
        model.save(EMBEDDING_MODEL_PATH)
        print("[OK] Embedding model ready.")
    except Exception as e:
        print(f"[ERROR] Failed to download embedding model: {e}")

    # 2. Semble Models (Downloads potion-code-16M)
    try:
        from semble import SembleIndex
        
        # We trigger a first-time indexing of the project to force model downloads.
        # Semble indexes in-memory (~250ms), so we don't need to save the index to disk.
        project_root = str(Path(__file__).resolve().parent.parent.parent)
        print(f"Preparing Semble models using codebase at: {project_root}...")
        
        # This will trigger the download of 'minishlab/potion-code-16M'
        SembleIndex.from_path(project_root)
        
        print("[OK] Semble models ready.")
    except Exception as e:
        print(f"[ERROR] Failed to prepare Semble: {e}")

if __name__ == "__main__":
    download_models()
