import chromadb
import logging
from typing import Any, Dict, List, Optional
from .embedding import EmbeddingService

logger = logging.getLogger(__name__)

class VectorStore:
    def __init__(self, persist_directory: str, collection_name: str, embedding_service: EmbeddingService):
        self.client = chromadb.PersistentClient(path=persist_directory)
        self.collection_name = collection_name
        self.embedding_service = embedding_service
        
        # Create or get collection
        self.collection = self.client.get_or_create_collection(
            name=self.collection_name,
            metadata={"hnsw:space": "cosine"}
        )

    def add_documents(self, documents: List[str], metadatas: List[Dict[str, Any]], ids: List[str]):
        """
        Add document chunks to the vector store.
        """
        if not documents:
            return
            
        embeddings = self.embedding_service.embed_documents(documents)
        
        self.collection.add(
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
        
        self.collection.add(
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
            formatted_results.append({
                "text": doc,
                "source": meta.get("source", ""),
                "page": None if meta.get("page") == -1 else meta.get("page"),
                "type": meta.get("type", "document")
            })
            
        return formatted_results
