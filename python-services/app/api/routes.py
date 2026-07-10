from fastapi import APIRouter, Depends, File, HTTPException, Request, Response, UploadFile
from typing import Any
import base64
import os
import signal

from ..models.schemas import (
    StatusResponse, ErrorResponse, IndexRequest, IndexResponse, 
    QueryRequest, QueryResponse, DeleteResponse, DeleteDocumentRequest
)
from ..core.lifecycle import state
from ..core.config import MAX_TOP_K
from ..services.indexing import indexing_service
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
    """Returns the current health status of the service."""
    return {"status": "ok"}

@router.post("/init", response_model=StatusResponse)
def initialize_service():
    """
    Initializes the local models and vector store. 
    Must be called before performing any indexing or queries.
    """
    if state.initialize():
        return {"status": "initialized"}
    return {"status": "already_initialized"}

@router.post("/index", response_model=IndexResponse, dependencies=[Depends(check_initialized)])
def index_documents(request: IndexRequest):
    """
    Ingests, chunks, and creates vector embeddings for a given document or folder.
    Associates the embeddings with a specific user session.
    """
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
    """
    Performs a semantic search against the vector store using the provided query.
    Returns the top_k most relevant document chunks.
    """
    try:
        top_k = min(request.top_k, MAX_TOP_K)
        return retrieval_service.query(request.query, top_k, request.session_id)
    except Exception as e:
        raise HTTPException(status_code=500, detail={"error": f"Query failed: {str(e)}"})

@router.post("/delete/{session_id}", response_model=DeleteResponse, dependencies=[Depends(check_initialized)])
def delete_documents(session_id: str):
    """
    Deletes all document embeddings associated with a specific session ID.
    Used for cleanup when a session is closed.
    """
    try:
        count = state.vector_store.delete_session_documents(session_id)
        return {"status": "deleted", "deleted_count": count}
    except Exception as e:
        raise HTTPException(status_code=500, detail={"error": f"Deletion failed: {str(e)}"})

@router.post("/delete-document", response_model=DeleteResponse, dependencies=[Depends(check_initialized)])
def delete_document(request: DeleteDocumentRequest):
    """
    Deletes a specific document (by source path) from a given session.
    """
    try:
        count = state.vector_store.delete_document(request.session_id, request.source)
        return {"status": "deleted", "deleted_count": count}
    except Exception as e:
        raise HTTPException(status_code=500, detail={"error": f"Document deletion failed: {str(e)}"})

@router.post("/shutdown", response_model=StatusResponse)
def shutdown_service():
    """
    Safely shuts down the vector store and terminates the Python process.
    """
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

@router.post("/ocr")
async def ocr_image(request: Request, file: UploadFile = File(None)):
    """Extracts text from an uploaded image using Tesseract OCR.
    Accepts either a file upload (multipart) or base64-encoded image in JSON body."""
    try:
        from PIL import Image
        import pytesseract
        import io

        if file:
            content = await file.read()
        else:
            try:
                body = await request.json()
            except Exception:
                body = {}

            image_b64 = body.get("image") if isinstance(body, dict) else None
            if image_b64:
                content = base64.b64decode(image_b64)
            else:
                raise HTTPException(status_code=400, detail="No image provided. Upload a file or send base64 data.")

        image = Image.open(io.BytesIO(content))
        text = pytesseract.image_to_string(image)
        return {"text": text}
    except ImportError as e:
        raise HTTPException(
            status_code=500,
            detail="OCR dependencies not installed. Run: pip install pytesseract Pillow"
        )
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))
