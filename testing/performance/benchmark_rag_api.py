import json
import pathlib
import statistics
import sys
import time
from typing import Any

from fastapi.testclient import TestClient


ROOT = pathlib.Path(__file__).resolve().parents[2]
PYTHON_TEST_ROOT = ROOT / "testing" / "python"
if str(PYTHON_TEST_ROOT) not in sys.path:
    sys.path.insert(0, str(PYTHON_TEST_ROOT))

from rag_test_harness import configured_rag_state, get_test_app


HEALTH_SAMPLES = 40
CYCLE_SAMPLES = 6
QUERY_TEXTS = [
    "How does AEGIS handle document indexing?",
    "What keeps the system local and private?",
    "Which components manage sessions and prompts?",
    "How does retrieval stay linked to the active session?",
    "What parts of AEGIS focus on frontend and CLI usage?",
]


def _duration_ms(operation) -> tuple[float, Any]:
    start = time.perf_counter()
    result = operation()
    duration_ms = (time.perf_counter() - start) * 1000
    return duration_ms, result


def _summary(values: list[float]) -> dict[str, float]:
    if not values:
        return {"mean_ms": 0.0, "median_ms": 0.0, "min_ms": 0.0, "max_ms": 0.0, "p95_ms": 0.0}

    if len(values) == 1:
        p95 = values[0]
    else:
        p95 = statistics.quantiles(values, n=20, method="inclusive")[18]

    return {
        "mean_ms": round(statistics.mean(values), 2),
        "median_ms": round(statistics.median(values), 2),
        "min_ms": round(min(values), 2),
        "max_ms": round(max(values), 2),
        "p95_ms": round(p95, 2),
    }


def main() -> int:
    app = get_test_app()
    health_durations: list[float] = []
    bootstrap_durations: list[float] = []
    index_durations: list[float] = []
    delete_durations: list[float] = []
    cycle_durations: list[float] = []
    chunks_per_second: list[float] = []
    query_durations: list[float] = []
    query_result_counts: list[int] = []
    query_similarities: list[float] = []
    chunks_added_values: list[int] = []

    for cycle_index in range(CYCLE_SAMPLES):
        with configured_rag_state(
            collection_name=f"benchmark_collection_{cycle_index}",
            corpus_repetitions=220,
        ) as harness:
            bootstrap_durations.append(harness.bootstrap_ms)

            with TestClient(app) as client:
                if cycle_index == 0:
                    for _ in range(HEALTH_SAMPLES):
                        duration_ms, response = _duration_ms(lambda: client.get("/health"))
                        response.raise_for_status()
                        health_durations.append(duration_ms)

                session_id = f"benchmark-session-{cycle_index}"
                cycle_start = time.perf_counter()

                index_duration_ms, index_response = _duration_ms(
                    lambda: client.post(
                        "/index",
                        json={
                            "path": str(harness.corpus_path),
                            "session_id": session_id,
                        },
                    )
                )
                index_response.raise_for_status()
                index_payload = index_response.json()
                chunks_added = index_payload["chunks_added"]
                chunks_added_values.append(chunks_added)
                index_durations.append(index_duration_ms)
                chunks_per_second.append(chunks_added / (index_duration_ms / 1000))

                for query_text in QUERY_TEXTS:
                    query_duration_ms, query_response = _duration_ms(
                        lambda q=query_text: client.post(
                            "/query",
                            json={
                                "query": q,
                                "top_k": 3,
                                "session_id": session_id,
                            },
                        )
                    )
                    query_response.raise_for_status()
                    query_payload = query_response.json()

                    query_durations.append(query_duration_ms)
                    query_result_counts.append(query_payload["metrics"]["chunk_count"])
                    query_similarities.append(query_payload["metrics"]["avg_similarity"])

                delete_duration_ms, delete_response = _duration_ms(
                    lambda: client.post(f"/delete/{session_id}")
                )
                delete_response.raise_for_status()
                delete_durations.append(delete_duration_ms)
                cycle_durations.append((time.perf_counter() - cycle_start) * 1000)

    result = {
        "benchmark": "rag_api_local_hash_json",
        "sample_counts": {
            "health": len(health_durations),
            "cycles": len(cycle_durations),
            "queries": len(query_durations),
        },
        "corpus": {
            "average_chunks_indexed": round(statistics.mean(chunks_added_values), 2),
            "query_set_size": len(QUERY_TEXTS),
            "backend": "hash embeddings + json vector store",
            "transport": "FastAPI TestClient (in-process HTTP)",
        },
        "health": _summary(health_durations),
        "bootstrap": _summary(bootstrap_durations),
        "index": {
            **_summary(index_durations),
            "mean_chunks_per_second": round(statistics.mean(chunks_per_second), 2),
        },
        "query": {
            **_summary(query_durations),
            "average_results_returned": round(statistics.mean(query_result_counts), 2),
            "average_similarity": round(statistics.mean(query_similarities), 4),
        },
        "delete": _summary(delete_durations),
        "full_cycle": _summary(cycle_durations),
    }

    results_dir = ROOT / "testing" / "results"
    results_dir.mkdir(parents=True, exist_ok=True)
    results_path = results_dir / "rag-api-benchmark-latest.json"
    results_path.write_text(json.dumps(result, indent=2), encoding="utf-8")

    print(json.dumps(result, indent=2))
    print(f"\nSaved benchmark results to: {results_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
