import pathlib
import hashlib
import uuid
import logging
import fitz  # PyMuPDF
from typing import Tuple, List

from ..core.config import CHUNK_SIZE_WORDS, CHUNK_OVERLAP_WORDS
from ..utils.text_splitter import split_text_by_words
from ..core.lifecycle import state

logger = logging.getLogger(__name__)

class IndexingService:
    def __init__(self):
        # Very simple cache to prevent re-indexing in the same session, 
        # or we could rely purely on generated ID deterministic hashing.
        pass
        
    def _generate_chunk_id(self, source: str, chunk_index: int) -> str:
        unique_string = f"{source}::{chunk_index}"
        return hashlib.md5(unique_string.encode()).hexdigest()

    def process_txt(self, file_path: pathlib.Path) -> List[dict]:
        chunks = []
        try:
            text = file_path.read_text(encoding='utf-8')
        except UnicodeDecodeError:
            text = file_path.read_bytes().decode('utf-8', errors='replace')
        except Exception as e:
            logger.error(f"Error processing text file {file_path}: {e}")
            return chunks

        try:
            if not text.strip():
                return chunks
                
            text_chunks = split_text_by_words(text, CHUNK_SIZE_WORDS, CHUNK_OVERLAP_WORDS)
            source = str(file_path.resolve())
            
            for i, chunk_text in enumerate(text_chunks):
                chunks.append({
                    "id": self._generate_chunk_id(source, i),
                    "text": chunk_text,
                    "metadata": {
                        "source": source,
                        "page": -1,  # represents null
                        "type": "document"
                    }
                })
        except Exception as e:
            logger.error(f"Error processing text file {file_path}: {e}")
        return chunks

    def process_pdf(self, file_path: pathlib.Path) -> List[dict]:
        chunks = []
        try:
            doc = fitz.open(file_path)
            source = str(file_path.resolve())
            
            chunk_global_index = 0
            for page_num in range(len(doc)):
                page = doc.load_page(page_num)
                text = page.get_text("text")
                if not text.strip():
                    continue
                    
                text_chunks = split_text_by_words(text, CHUNK_SIZE_WORDS, CHUNK_OVERLAP_WORDS)
                for chunk_text in text_chunks:
                    chunks.append({
                        "id": self._generate_chunk_id(source, chunk_global_index),
                        "text": chunk_text,
                        "metadata": {
                            "source": source,
                            "page": page_num + 1,
                            "type": "document"
                        }
                    })
                    chunk_global_index += 1
        except Exception as e:
            logger.error(f"Error processing PDF file {file_path}: {e}")
        return chunks

    def index_path(self, path_str: str) -> int:
        path = pathlib.Path(path_str).resolve()
        
        if not path.exists():
            raise FileNotFoundError(f"Path does not exist: {path}")

        files_to_process = []
        if path.is_file():
            if path.suffix.lower() in [".txt", ".pdf"]:
                files_to_process.append(path)
        elif path.is_dir():
            for f in path.rglob("*"):
                if f.is_file() and f.suffix.lower() in [".txt", ".pdf"]:
                    files_to_process.append(f)

        total_chunks = 0
        documents = []
        metadatas = []
        ids = []

        for f in files_to_process:
            if f.suffix.lower() == ".txt":
                file_chunks = self.process_txt(f)
            else:
                file_chunks = self.process_pdf(f)

            for chunk in file_chunks:
                documents.append(chunk["text"])
                metadatas.append(chunk["metadata"])
                ids.append(chunk["id"])
                
        if documents:
            # Chunking the insert to prevent large batch payloads 
            batch_size = 500
            for i in range(0, len(documents), batch_size):
                state.vector_store.add_documents(
                    documents=documents[i:i+batch_size],
                    metadatas=metadatas[i:i+batch_size],
                    ids=ids[i:i+batch_size]
                )
                total_chunks += len(documents[i:i+batch_size])

        return total_chunks

indexing_service = IndexingService()
