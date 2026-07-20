import time
from typing import List, Dict, Any
from ..core.lifecycle import state

class RetrievalService:
    def query(self, query_text: str, top_k: int, session_id: str | None, scope: str = "session") -> Dict[str, Any]:
        normalized_session = (session_id or "").strip()
        if not query_text.strip() or (scope != "workspace" and not normalized_session):
            return {
                "results": [],
                "metrics": {
                    "retrieval_time_ms": 0.0,
                    "avg_similarity": 0.0,
                    "chunk_count": 0,
                    "backend": state.backend_name
                }
            }
            
        start_time = time.time()
        results = state.vector_store.query(query_text, top_k, normalized_session, scope)
        duration_ms = (time.time() - start_time) * 1000
        
        avg_sim = 0.0
        if results:
            avg_sim = sum(r["score"] for r in results) / len(results)
            
        return {
            "results": results,
            "metrics": {
                "retrieval_time_ms": round(duration_ms, 2),
                "avg_similarity": round(avg_sim, 4),
                "chunk_count": len(results),
                "backend": state.backend_name
            }
        }

retrieval_service = RetrievalService()
