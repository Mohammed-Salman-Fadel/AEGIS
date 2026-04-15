from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse
from .api.routes import router
from .core.lifecycle import state

app = FastAPI(title="AEGIS Python RAG Subsystem", version="1.0.0")

app.include_router(router)

@app.exception_handler(Exception)
async def global_exception_handler(request: Request, exc: Exception):
    """
    Ensure all errors return consistent JSON formats as per requirements.
    """
    return JSONResponse(
        status_code=500,
        content={"error": str(exc)},
    )
    
@app.on_event("shutdown")
def shutdown_event():
    state.shutdown()

if __name__ == "__main__":
    import uvicorn
    uvicorn.run("app.main:app", host="127.0.0.1", port=8000, reload=False)
