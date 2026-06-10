import os
import pathlib
import shutil
import sys
import tempfile
import textwrap
import time
import types
from contextlib import contextmanager
from dataclasses import dataclass
from typing import Iterator


ROOT = pathlib.Path(__file__).resolve().parents[2]
RAG_ROOT = ROOT / "python-services"
if str(RAG_ROOT) not in sys.path:
    sys.path.insert(0, str(RAG_ROOT))

from app.core.lifecycle import state
from app.services.embedding import EmbeddingService
from app.services.vector_store import VectorStore


CORPUS_PARAGRAPH = textwrap.dedent(
    """
    AEGIS is a local-first assistant platform with a Rust engine, a Python retrieval subsystem,
    and multiple user interfaces. The engine coordinates sessions, prompt building, provider
    selection, and tool execution. The retrieval subsystem indexes local documents, chunks text,
    stores embeddings, and answers session-scoped queries. Privacy matters, so documents stay
    on the local machine and retrieval only returns material linked to the active session.
    """
).strip()


@dataclass
class HarnessContext:
    workspace: pathlib.Path
    corpus_path: pathlib.Path
    bootstrap_ms: float


def install_voice_stub() -> None:
    if "app.services.voice" in sys.modules:
        return

    stub = types.ModuleType("app.services.voice")

    class _StubVoiceService:
        keep_cached = True

        def unload_models(self) -> None:
            return None

        def transcribe(self, _audio_bytes: bytes) -> str:
            return "voice testing disabled"

        def synthesize(self, _text: str):
            return None

    stub.voice_service = _StubVoiceService()
    sys.modules["app.services.voice"] = stub


def get_test_app():
    install_voice_stub()
    from app.main import app

    return app


def _build_corpus_text(repetitions: int) -> str:
    sections = []
    for index in range(repetitions):
        sections.append(
            f"Section {index + 1}. {CORPUS_PARAGRAPH} "
            f"The CLI focuses on commands, the frontend focuses on chat, and the engine keeps state consistent. "
            f"Benchmark pass {index + 1} mentions indexing, retrieval, performance, latency, throughput, and cleanup."
        )
    return "\n\n".join(sections)


def write_corpus_file(workspace: pathlib.Path, repetitions: int = 180) -> pathlib.Path:
    workspace.mkdir(parents=True, exist_ok=True)
    corpus_path = workspace / "rag-system-corpus.txt"
    corpus_path.write_text(_build_corpus_text(repetitions), encoding="utf-8")
    return corpus_path


@contextmanager
def configured_rag_state(
    *,
    collection_name: str = "testing_collection",
    corpus_repetitions: int = 180,
) -> Iterator[HarnessContext]:
    original_env = {
        "AEGIS_RAG_EMBEDDING_BACKEND": os.environ.get("AEGIS_RAG_EMBEDDING_BACKEND"),
        "AEGIS_RAG_VECTOR_BACKEND": os.environ.get("AEGIS_RAG_VECTOR_BACKEND"),
    }
    original_state = {
        "is_initialized": state.is_initialized,
        "embedding_service": state.embedding_service,
        "vector_store": state.vector_store,
        "backend_name": state.backend_name,
    }

    workspace = pathlib.Path(tempfile.mkdtemp(prefix="aegis-testing-"))

    try:
        os.environ["AEGIS_RAG_EMBEDDING_BACKEND"] = "hash"
        os.environ["AEGIS_RAG_VECTOR_BACKEND"] = "json"

        start = time.perf_counter()
        embedding_service = EmbeddingService("local-hash-benchmark")
        vector_store = VectorStore(
            persist_directory=str(workspace / "data"),
            collection_name=collection_name,
            embedding_service=embedding_service,
        )
        bootstrap_ms = (time.perf_counter() - start) * 1000

        state.embedding_service = embedding_service
        state.vector_store = vector_store
        state.backend_name = embedding_service.backend
        state.is_initialized = True

        corpus_path = write_corpus_file(workspace, repetitions=corpus_repetitions)
        yield HarnessContext(
            workspace=workspace,
            corpus_path=corpus_path,
            bootstrap_ms=bootstrap_ms,
        )
    finally:
        state.is_initialized = original_state["is_initialized"]
        state.embedding_service = original_state["embedding_service"]
        state.vector_store = original_state["vector_store"]
        state.backend_name = original_state["backend_name"]

        for key, value in original_env.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value

        shutil.rmtree(workspace, ignore_errors=True)
