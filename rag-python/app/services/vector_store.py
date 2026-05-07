import json
import logging
import math
import os
import pathlib
import threading
from typing import Any, Dict, List, Optional
from .embedding import EmbeddingService

try:
    import chromadb
except Exception as import_error:  # pragma: no cover - depends on local Python environment
    chromadb = None
    CHROMA_IMPORT_ERROR = import_error
else:
    CHROMA_IMPORT_ERROR = None

logger = logging.getLogger(__name__)

class VectorStore:
    def __init__(self, persist_directory: str, collection_name: str, embedding_service: EmbeddingService):
        self.persist_directory = pathlib.Path(persist_directory)
        self.persist_directory.mkdir(parents=True, exist_ok=True)
        self.collection_name = collection_name
        self.embedding_service = embedding_service
        self._lock = threading.Lock()
        self._json_store_path = self.persist_directory / f"{self.collection_name}.json"
        self.client = None
        self.collection = None
        self._use_chroma = False
        backend = os.getenv("AEGIS_RAG_VECTOR_BACKEND", "json").strip().lower()

        if backend != "chroma":
            logger.info(
                "Using local JSON vector store. Set AEGIS_RAG_VECTOR_BACKEND=chroma to opt into ChromaDB."
            )
            return

        if chromadb is None:
            logger.warning(
                "ChromaDB could not be imported; using local JSON vector store fallback: %s",
                CHROMA_IMPORT_ERROR,
            )
            return

        try:
            self.client = chromadb.PersistentClient(path=str(self.persist_directory))
            self.collection = self.client.get_or_create_collection(
                name=self.collection_name,
                metadata={"hnsw:space": "cosine"}
            )
            self._use_chroma = True
        except Exception as error:
            logger.warning(
                "ChromaDB could not be initialized; using local JSON vector store fallback: %s",
                error,
            )

    def add_documents(self, documents: List[str], metadatas: List[Dict[str, Any]], ids: List[str]):
        """
        Add document chunks to the vector store.
        """
        if not documents:
            return
            
        embeddings = self.embedding_service.embed_documents(documents)

        if self._use_chroma:
            self.collection.upsert(
                embeddings=embeddings,
                documents=documents,
                metadatas=metadatas,
                ids=ids
            )
            return

        self._json_upsert(
            embeddings=embeddings,
            documents=documents,
            metadatas=metadatas,
            ids=ids
        )

    def add_memory(self, text: str, memory_id: str):
        """
        Add a single piece of text to memory.
        """
        embedding = self.embedding_service.embed_query(text)
        metadata = {
            "source": "user",
            "page": -1,  # -1 represents null/none in chromadb metadata (which expects int/float/str)
            "type": "memory"
        }
        
        if self._use_chroma:
            self.collection.upsert(
                embeddings=[embedding],
                documents=[text],
                metadatas=[metadata],
                ids=[memory_id]
            )
            return

        self._json_upsert(
            embeddings=[embedding],
            documents=[text],
            metadatas=[metadata],
            ids=[memory_id]
        )

    def query(self, query_text: str, n_results: int) -> List[Dict[str, Any]]:
        """
        Query the vector store.
        """
        embedding = self.embedding_service.embed_query(query_text)

        if self._use_chroma:
            results = self.collection.query(
                query_embeddings=[embedding],
                n_results=n_results,
                include=["documents", "metadatas"]
            )
            
            formatted_results = []
            if not results["documents"] or not results["documents"][0]:
                return formatted_results
                
            docs = results["documents"][0]
            metas = results["metadatas"][0]
            
            for doc, meta in zip(docs, metas):
                formatted_results.append(_format_result(doc, meta))
                
            return formatted_results

        return self._json_query(embedding, n_results)

    def _json_upsert(
        self,
        documents: List[str],
        metadatas: List[Dict[str, Any]],
        ids: List[str],
        embeddings: List[List[float]],
    ):
        with self._lock:
            records = {record["id"]: record for record in self._read_json_records()}

            for document, metadata, record_id, embedding in zip(documents, metadatas, ids, embeddings):
                records[record_id] = {
                    "id": record_id,
                    "document": document,
                    "metadata": metadata,
                    "embedding": embedding,
                }

            self._write_json_records(list(records.values()))

    def _json_query(self, query_embedding: List[float], n_results: int) -> List[Dict[str, Any]]:
        with self._lock:
            records = self._read_json_records()

        scored = []
        for record in records:
            score = _cosine_similarity(query_embedding, record.get("embedding", []))
            scored.append((score, record))

        scored.sort(key=lambda item: item[0], reverse=True)

        return [
            _format_result(record.get("document", ""), record.get("metadata", {}))
            for score, record in scored[:n_results]
            if score > 0
        ]

    def _read_json_records(self) -> List[Dict[str, Any]]:
        if not self._json_store_path.exists():
            return []

        try:
            with self._json_store_path.open("r", encoding="utf-8") as file:
                data = json.load(file)
        except Exception as error:
            logger.warning("Could not read JSON vector store; starting empty: %s", error)
            return []

        return data if isinstance(data, list) else []

    def _write_json_records(self, records: List[Dict[str, Any]]):
        temp_path = self._json_store_path.with_suffix(".tmp")
        with temp_path.open("w", encoding="utf-8") as file:
            json.dump(records, file)
        temp_path.replace(self._json_store_path)

def _cosine_similarity(left: List[float], right: List[float]) -> float:
    if not left or not right or len(left) != len(right):
        return 0.0

    dot = sum(a * b for a, b in zip(left, right))
    left_norm = math.sqrt(sum(value * value for value in left))
    right_norm = math.sqrt(sum(value * value for value in right))

    if left_norm == 0 or right_norm == 0:
        return 0.0

    return dot / (left_norm * right_norm)

def _format_result(document: str, meta: Dict[str, Any]) -> Dict[str, Any]:
    return {
        "text": document,
        "source": meta.get("source", ""),
        "page": None if meta.get("page") == -1 else meta.get("page"),
        "type": meta.get("type", "document")
    }
