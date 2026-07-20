from pydantic import BaseModel, Field
from typing import Optional, List, Any, Literal

# API Requests

class IndexRequest(BaseModel):
    path: str = Field(..., description="Absolute path to the file or directory to index")
    session_id: str = Field(..., description="AEGIS session that owns this indexed document")

class QueryRequest(BaseModel):
    query: str = Field(..., description="The query string")
    top_k: int = Field(3, description="Number of top results to return")
    session_id: Optional[str] = Field(None, description="AEGIS session to retrieve documents from")
    scope: Literal["session", "workspace"] = Field("session", description="Search one session or every locally indexed source")

class DeleteDocumentRequest(BaseModel):
    session_id: str = Field(..., description="AEGIS session that owns this indexed document")
    source: str = Field(..., description="Absolute source path of the indexed document")

# API Responses

class StatusResponse(BaseModel):
    status: str = Field(..., description="Status of the requested operation")

class ErrorResponse(BaseModel):
    error: str = Field(..., description="Error message describing the failure")

class IndexResponse(BaseModel):
    status: str = Field(..., description="Status of the index operation")
    chunks_added: int = Field(..., description="Number of new document chunks added to the index")

class SearchResult(BaseModel):
    text: str = Field(..., description="The content of the retrieved document chunk")
    source: str = Field(..., description="The source file path or origin of the chunk")
    page: Optional[int] = Field(None, description="Page number of the chunk, if applicable")
    type: str = Field(..., description="Type of the stored entry (e.g., 'document')")
    score: float = Field(..., description="Relevance score (cosine similarity) of the chunk")

class RagMetrics(BaseModel):
    retrieval_time_ms: float = Field(..., description="Time taken to retrieve results in milliseconds")
    avg_similarity: float = Field(..., description="Average similarity score of the top_k results")
    chunk_count: int = Field(..., description="Number of chunks successfully retrieved")
    backend: str = Field(..., description="The backend used for retrieval (e.g., 'chroma')")

class DeleteResponse(BaseModel):
    status: str = Field(..., description="Status of the delete operation")
    deleted_count: int = Field(..., description="Number of document chunks deleted")

class QueryResponse(BaseModel):
    results: List[SearchResult] = Field(..., description="List of the most relevant document chunks")
    metrics: RagMetrics = Field(..., description="Performance and quality metrics for the query")
