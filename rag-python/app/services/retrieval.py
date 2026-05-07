from typing import List, Dict, Any
from ..core.lifecycle import state

class RetrievalService:
    def query(self, query_text: str, top_k: int, session_id: str) -> List[Dict[str, Any]]:
        if not query_text.strip() or not session_id.strip():
            return []
            
        return state.vector_store.query(query_text, top_k, session_id.strip())

retrieval_service = RetrievalService()
