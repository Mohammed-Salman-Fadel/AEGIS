import logging
import os

logger = logging.getLogger(__name__)

class EmbeddingService:
    def __init__(self, model_name: str):
        from app.core.config import EMBEDDING_MODEL_PATH
        self.model_name = model_name
        self.model_path = EMBEDDING_MODEL_PATH
        self._model = None
        self.backend = "sbert"

        try:
            from sentence_transformers import SentenceTransformer

            # Priority 1: Direct local path
            if os.path.exists(self.model_path):
                logger.info(f"Loading embedding model from local path: {self.model_path}")
                self._model = SentenceTransformer(self.model_path)
            else:
                # Priority 2: Hugging Face cache (offline mode if local_files_only=True)
                logger.info(f"Loading embedding model from HF cache: {self.model_name}")
                # We enforce loading from HF cache/internet. 
                self._model = SentenceTransformer(self.model_name, local_files_only=False)
                
            self.tokenizer = self._model.tokenizer
        except Exception as error:
            logger.error(
                "Could not load embedding model %s. RAG functionality will fail. Error: %s",
                self.model_name,
                error,
            )
            raise RuntimeError(f"Failed to load embedding model: {error}")
    
    def embed_documents(self, documents: list[str]) -> list[list[float]]:
        """
        Embeds a list of texts into vector representations.
        """
        if not documents:
            return []
        if self._model is None:
            raise RuntimeError("Embedding model is not loaded.")
        embeddings = self._model.encode(documents, convert_to_numpy=True)
        return embeddings.tolist()
        
    def embed_query(self, query: str) -> list[float]:
        """
        Embeds a single query string.
        """
        if self._model is None:
            raise RuntimeError("Embedding model is not loaded.")
        embedding = self._model.encode([query], convert_to_numpy=True)
        return embedding[0].tolist()
