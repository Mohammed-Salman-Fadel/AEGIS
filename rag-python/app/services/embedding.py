import logging
import hashlib
import math
import os
import re

logger = logging.getLogger(__name__)

class EmbeddingService:
    def __init__(self, model_name: str):
        self.model_name = model_name
        self._model = None
        backend = os.getenv("AEGIS_RAG_EMBEDDING_BACKEND", "hash").strip().lower()

        if backend not in {"sentence-transformers", "sentence_transformers", "sbert"}:
            logger.info(
                "Using local hash embeddings. Set AEGIS_RAG_EMBEDDING_BACKEND=sentence-transformers to opt into SentenceTransformers."
            )
            return

        try:
            from sentence_transformers import SentenceTransformer

            self._model = SentenceTransformer(self.model_name, local_files_only=True)
        except Exception as error:
            logger.warning(
                "Could not load local embedding model %s; using local hash embeddings: %s",
                self.model_name,
                error,
            )
    
    def embed_documents(self, documents: list[str]) -> list[list[float]]:
        """
        Embeds a list of texts into vector representations.
        """
        if not documents:
            return []
        if self._model is None:
            return [_hash_embedding(document) for document in documents]
        embeddings = self._model.encode(documents, convert_to_numpy=True)
        return embeddings.tolist()
        
    def embed_query(self, query: str) -> list[float]:
        """
        Embeds a single query string.
        """
        if self._model is None:
            return _hash_embedding(query)
        embedding = self._model.encode([query], convert_to_numpy=True)
        return embedding[0].tolist()

def _hash_embedding(text: str, dimensions: int = 384) -> list[float]:
    """
    Small deterministic fallback embedding.

    This is not as semantically rich as SentenceTransformers, but it keeps local
    document retrieval functional when the ML stack is missing or incompatible.
    """
    vector = [0.0] * dimensions
    tokens = re.findall(r"[a-zA-Z0-9_]+", text.lower())

    for token in tokens:
        digest = hashlib.blake2b(token.encode("utf-8"), digest_size=8).digest()
        bucket = int.from_bytes(digest[:4], "little") % dimensions
        sign = 1.0 if digest[4] % 2 == 0 else -1.0
        vector[bucket] += sign

    magnitude = math.sqrt(sum(value * value for value in vector))
    if magnitude == 0:
        return vector

    return [value / magnitude for value in vector]
