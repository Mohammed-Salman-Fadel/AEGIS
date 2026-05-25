import pathlib
import sys
import unittest

from fastapi.testclient import TestClient


TEST_ROOT = pathlib.Path(__file__).resolve().parent
if str(TEST_ROOT) not in sys.path:
    sys.path.insert(0, str(TEST_ROOT))

from rag_test_harness import configured_rag_state, get_test_app


class RagApiSystemTests(unittest.TestCase):
    def test_end_to_end_rag_api_cycle(self) -> None:
        app = get_test_app()
        with configured_rag_state(collection_name="system_test_collection") as harness:
            with TestClient(app) as client:
                health_response = client.get("/health")
                self.assertEqual(health_response.status_code, 200)
                self.assertEqual(health_response.json()["status"], "ok")

                index_response = client.post(
                    "/index",
                    json={
                        "path": str(harness.corpus_path),
                        "session_id": "system-test-session",
                    },
                )
                self.assertEqual(index_response.status_code, 200)
                index_payload = index_response.json()
                self.assertEqual(index_payload["status"], "indexed")
                self.assertGreaterEqual(index_payload["chunks_added"], 10)

                query_response = client.post(
                    "/query",
                    json={
                        "query": "How does AEGIS keep retrieval private and session scoped?",
                        "top_k": 3,
                        "session_id": "system-test-session",
                    },
                )
                self.assertEqual(query_response.status_code, 200)
                query_payload = query_response.json()
                self.assertGreaterEqual(query_payload["metrics"]["chunk_count"], 1)
                self.assertGreater(query_payload["metrics"]["avg_similarity"], 0.0)
                self.assertTrue(
                    any(
                        "session" in result["text"].lower()
                        or "privacy" in result["text"].lower()
                        for result in query_payload["results"]
                    )
                )

                delete_response = client.post("/delete/system-test-session")
                self.assertEqual(delete_response.status_code, 200)
                self.assertEqual(delete_response.json()["status"], "deleted")
                self.assertGreaterEqual(delete_response.json()["deleted_count"], 1)

                empty_query_response = client.post(
                    "/query",
                    json={
                        "query": "privacy and session scoped retrieval",
                        "top_k": 3,
                        "session_id": "system-test-session",
                    },
                )
                self.assertEqual(empty_query_response.status_code, 200)
                self.assertEqual(empty_query_response.json()["results"], [])


if __name__ == "__main__":
    unittest.main()
