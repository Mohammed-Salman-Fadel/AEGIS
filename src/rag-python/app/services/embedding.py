import logging
from sentence_transformers import SentenceTransformer

logger = logging.getLogger(__name__)

class EmbeddingService:
    def __init__(self, model_name: str):
        self.model_name = model_name
        self._model = SentenceTransformer(self.model_name)
    
    def embed_documents(self, documents: list[str]) -> list[list[float]]:
        """
        Embeds a list of texts into vector representations.
        """
        if not documents:
            return []
        embeddings = self._model.encode(documents, convert_to_numpy=True)
        return embeddings.tolist()
        
    def embed_query(self, query: str) -> list[float]:
        """
        Embeds a single query string.
        """
        embedding = self._model.encode([query], convert_to_numpy=True)
        return embedding[0].tolist()
