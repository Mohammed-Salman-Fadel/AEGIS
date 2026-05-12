from pydantic import BaseModel, Field
from typing import Optional, List, Any

# API Requests

class IndexRequest(BaseModel):
    path: str = Field(..., description="Absolute path to the file or directory to index")
    session_id: str = Field(..., description="AEGIS session that owns this indexed document")

class QueryRequest(BaseModel):
    query: str = Field(..., description="The query string")
    top_k: int = Field(3, description="Number of top results to return")
    session_id: str = Field(..., description="AEGIS session to retrieve documents from")

class StoreRequest(BaseModel):
    text: str = Field(..., description="The text to store in memory")

# API Responses

class StatusResponse(BaseModel):
    status: str

class ErrorResponse(BaseModel):
    error: str

class IndexResponse(BaseModel):
    status: str
    chunks_added: int

class SearchResult(BaseModel):
    text: str
    source: str
    page: Optional[int] = None
    type: str
    score: float

class RagMetrics(BaseModel):
    retrieval_time_ms: float
    avg_similarity: float
    chunk_count: int
    backend: str

class DeleteResponse(BaseModel):
    status: str
    deleted_count: int

class QueryResponse(BaseModel):
    results: List[SearchResult]
    metrics: RagMetrics
