from fastapi import APIRouter, HTTPException, Depends, UploadFile, File, Response
from typing import Any
import os
import signal

from ..models.schemas import (
    StatusResponse, ErrorResponse, IndexRequest, IndexResponse, 
    QueryRequest, QueryResponse, StoreRequest, DeleteResponse, DeleteDocumentRequest
)
from ..core.lifecycle import state
from ..core.config import MAX_TOP_K
from ..services.indexing import indexing_service
from ..services.memory import memory_service
from ..services.retrieval import retrieval_service
from ..services.voice import voice_service

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
        chunks_added = indexing_service.index_path(request.path, request.session_id)
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
        return retrieval_service.query(request.query, top_k, request.session_id)
    except Exception as e:
        raise HTTPException(status_code=500, detail={"error": f"Query failed: {str(e)}"})

@router.post("/delete/{session_id}", response_model=DeleteResponse, dependencies=[Depends(check_initialized)])
def delete_documents(session_id: str):
    try:
        count = state.vector_store.delete_session_documents(session_id)
        return {"status": "deleted", "deleted_count": count}
    except Exception as e:
        raise HTTPException(status_code=500, detail={"error": f"Deletion failed: {str(e)}"})

@router.post("/delete-document", response_model=DeleteResponse, dependencies=[Depends(check_initialized)])
def delete_document(request: DeleteDocumentRequest):
    try:
        count = state.vector_store.delete_document(request.session_id, request.source)
        return {"status": "deleted", "deleted_count": count}
    except Exception as e:
        raise HTTPException(status_code=500, detail={"error": f"Document deletion failed: {str(e)}"})

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

@router.post("/transcribe")
async def transcribe_audio(file: UploadFile = File(...)):
    """Transcribes an uploaded audio file to text"""
    try:
        content = await file.read()
        text = voice_service.transcribe(content)
        return {"text": text}
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

@router.get("/synthesize")
async def synthesize_voice(text: str):
    """Synthesizes text to speech and returns a WAV file"""
    try:
        audio_data = voice_service.synthesize(text)
        if not audio_data:
            raise HTTPException(status_code=500, detail="Synthesis failed")
        return Response(content=audio_data, media_type="audio/wav")
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

@router.post("/voice/config")
def configure_voice(keep_cached: bool):
    """Configures whether voice models remain cached in memory"""
    try:
        voice_service.keep_cached = keep_cached
        if not keep_cached:
            voice_service.unload_models()
        return {"status": "ok", "keep_cached": keep_cached}
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))
