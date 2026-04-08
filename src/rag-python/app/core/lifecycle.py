import threading
import logging
import os
from .config import DATA_DIR, CHROMA_COLLECTION_NAME, EMBEDDING_MODEL_NAME
from ..services.vector_store import VectorStore
from ..services.embedding import EmbeddingService

logger = logging.getLogger(__name__)

class LifecycleState:
    def __init__(self):
        self.is_initialized = False
        self.lock = threading.Lock()
        
        self.embedding_service: EmbeddingService | None = None
        self.vector_store: VectorStore | None = None

    def initialize(self):
        with self.lock:
            if self.is_initialized:
                return False
            
            logger.info("Initializing RAG Subsystem...")
            
            # Ensure data directory exists
            os.makedirs(DATA_DIR, exist_ok=True)

            # Load Embedding model
            logger.info(f"Loading Embedding Model: {EMBEDDING_MODEL_NAME}")
            self.embedding_service = EmbeddingService(EMBEDDING_MODEL_NAME)
            
            # Initialize Vector Store
            logger.info("Initializing Vector Store...")
            self.vector_store = VectorStore(
                persist_directory=DATA_DIR,
                collection_name=CHROMA_COLLECTION_NAME,
                embedding_service=self.embedding_service
            )

            self.is_initialized = True
            logger.info("RAG Subsystem Initialized successfully.")
            return True

    def shutdown(self):
        with self.lock:
            if not self.is_initialized:
                return False
                
            logger.info("Shutting down RAG Subsystem...")
            self.vector_store = None
            self.embedding_service = None
            self.is_initialized = False
            return True

# Singleton instance
state = LifecycleState()
