import uuid
from ..core.lifecycle import state

class MemoryService:
    def store_memory(self, text: str) -> None:
        if not text.strip():
            raise ValueError("Memory text cannot be empty.")
            
        memory_id = f"mem::call::{uuid.uuid4().hex}"
        state.vector_store.add_memory(text, memory_id)

memory_service = MemoryService()
