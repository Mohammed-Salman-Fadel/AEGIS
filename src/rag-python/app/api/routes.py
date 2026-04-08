from fastapi import APIRouter, HTTPException, Depends
from typing import Any
import os
import signal

from ..models.schemas import (
    StatusResponse, ErrorResponse, IndexRequest, IndexResponse, 
    QueryRequest, QueryResponse, StoreRequest
)
from ..core.lifecycle import state
from ..core.config import MAX_TOP_K
from ..services.indexing import indexing_service
from ..services.memory import memory_service
from ..services.retrieval import retrieval_service

# TODO: Currently using REST API for testing and demo. This entire interface will be migrated to IPC for near-instant latency with the Rust orchestrator.
router = APIRouter()

def check_initialized():
    """Dependency to ensure subsystem is initialized"""
    if not state.is_initialized:
        raise HTTPException(
            status_code=400, 
            detail="Service not initialized. Call /init first."
        )

@router.get("/health", response_model=StatusResponse)
def health_check():
    return {"status": "ok"}

@router.post("/init", response_model=StatusResponse)
def initialize_service():
    if state.initialize():
        return {"status": "initialized"}
    return {"status": "already_initialized"}

@router.post("/index", response_model=IndexResponse, dependencies=[Depends(check_initialized)])
def index_documents(request: IndexRequest):
    try:
        chunks_added = indexing_service.index_path(request.path)
        return {"status": "indexed", "chunks_added": chunks_added}
    except FileNotFoundError as e:
        # Avoid stack traces for missing files
        raise HTTPException(status_code=400, detail={"error": str(e)})
    except Exception as e:
        raise HTTPException(status_code=500, detail={"error": f"Indexing failed: {str(e)}"})

@router.post("/query", response_model=QueryResponse, dependencies=[Depends(check_initialized)])
def query_documents(request: QueryRequest):
    try:
        top_k = min(request.top_k, MAX_TOP_K)
        results = retrieval_service.query(request.query, top_k)
        return {"results": results}
    except Exception as e:
        raise HTTPException(status_code=500, detail={"error": f"Query failed: {str(e)}"})

# TODO: The memory /store endpoint is temporarily hidden for the demo.
# We will integrate this feature with the Rust engine in the next milestone.

@router.post("/shutdown", response_model=StatusResponse)
def shutdown_service():
    state.shutdown()
    
    # Schedule process term signal asynchronously so the HTTP response goes through
    def kill_process():
        os.kill(os.getpid(), signal.SIGTERM)
        
    import threading
    threading.Timer(1.0, kill_process).start()
    
    return {"status": "shutting_down"}
