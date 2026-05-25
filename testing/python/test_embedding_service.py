import math
import os
import pathlib
import sys
import unittest
from unittest.mock import patch


ROOT = pathlib.Path(__file__).resolve().parents[2]
RAG_ROOT = ROOT / "rag-python"
if str(RAG_ROOT) not in sys.path:
    sys.path.insert(0, str(RAG_ROOT))

from app.services.embedding import EmbeddingService, _hash_embedding


class EmbeddingServiceTests(unittest.TestCase):
    def test_hash_embedding_is_deterministic_and_normalized(self) -> None:
        vector_a = _hash_embedding("Alpha beta beta")
        vector_b = _hash_embedding("Alpha beta beta")

        self.assertEqual(vector_a, vector_b)
        self.assertEqual(len(vector_a), 384)

        magnitude = math.sqrt(sum(value * value for value in vector_a))
        self.assertAlmostEqual(magnitude, 1.0, places=6)

    def test_hash_embedding_returns_zero_vector_for_empty_text(self) -> None:
        vector = _hash_embedding("")

        self.assertEqual(len(vector), 384)
        self.assertTrue(all(value == 0.0 for value in vector))

    def test_service_uses_hash_fallback_when_backend_is_disabled(self) -> None:
        with patch.dict(os.environ, {"AEGIS_RAG_EMBEDDING_BACKEND": "hash"}, clear=False):
            service = EmbeddingService("unused-model")
            query_vector = service.embed_query("local fallback works")
            document_vectors = service.embed_documents(["local fallback works"])

        self.assertEqual(service.backend, "hash-fallback")
        self.assertEqual(query_vector, _hash_embedding("local fallback works"))
        self.assertEqual(document_vectors, [query_vector])


if __name__ == "__main__":
    unittest.main()
