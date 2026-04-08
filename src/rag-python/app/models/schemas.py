from pydantic import BaseModel, Field
from typing import Optional, List, Any

# API Requests

class IndexRequest(BaseModel):
    path: str = Field(..., description="Absolute path to the file or directory to index")

class QueryRequest(BaseModel):
    query: str = Field(..., description="The query string")
    top_k: int = Field(3, description="Number of top results to return")

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

class QueryResponse(BaseModel):
    results: List[SearchResult]
