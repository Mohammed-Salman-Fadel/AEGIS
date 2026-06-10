import os
import pathlib
import sys
import unittest
from unittest.mock import patch


ROOT = pathlib.Path(__file__).resolve().parents[2]
RAG_ROOT = ROOT / "python-services"
if str(RAG_ROOT) not in sys.path:
    sys.path.insert(0, str(RAG_ROOT))

from app.services.embedding import EmbeddingService
from app.utils.text_splitter import split_text_by_words


class RagPipelineIntegrationTests(unittest.TestCase):
    def test_chunking_and_embedding_keep_parallel_output_shapes(self) -> None:
        text = "alpha beta gamma delta epsilon zeta eta theta"
        chunks = split_text_by_words(text, chunk_size=4, chunk_overlap=1)

        with patch.dict(os.environ, {"AEGIS_RAG_EMBEDDING_BACKEND": "hash"}, clear=False):
            service = EmbeddingService("unused-model")
            vectors = service.embed_documents(chunks)

        self.assertEqual(
            chunks,
            [
                "alpha beta gamma delta",
                "delta epsilon zeta eta",
                "eta theta",
            ],
        )
        self.assertEqual(len(vectors), len(chunks))
        self.assertTrue(all(len(vector) == 384 for vector in vectors))

    def test_chunk_vectors_match_query_vectors_for_identical_text(self) -> None:
        chunks = split_text_by_words("echo echo echo echo", chunk_size=2, chunk_overlap=0)

        with patch.dict(os.environ, {"AEGIS_RAG_EMBEDDING_BACKEND": "hash"}, clear=False):
            service = EmbeddingService("unused-model")
            vectors = service.embed_documents(chunks)
            query_vector = service.embed_query(chunks[0])

        self.assertEqual(chunks, ["echo echo", "echo echo"])
        self.assertEqual(vectors[0], vectors[1])
        self.assertEqual(vectors[0], query_vector)


if __name__ == "__main__":
    unittest.main()
