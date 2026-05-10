from typing import List, Dict, Any
from ..core.lifecycle import state

class RetrievalService:
    def query(self, query_text: str, top_k: int) -> List[Dict[str, Any]]:
        if not query_text.strip():
            return []
            
        return state.vector_store.query(query_text, top_k)

retrieval_service = RetrievalService()
