#!/usr/bin/env python3
"""Validate R-1803 tokenization, embeddings, vector index, and RAG gates."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def run_step(name: str, args: list[str]) -> None:
    print(f"[R-1803] {name}: {' '.join(args)}")
    completed = subprocess.run(args, cwd=ROOT, text=True)
    if completed.returncode != 0:
        raise SystemExit(completed.returncode)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def validate_index(path: Path) -> None:
    data = json.loads(path.read_text(encoding="utf-8"))
    require(data["schema"] == "spectra.ml.vector_index.v1", "bad vector index schema")
    require(data["dim"] == 16, "example vector dimension mismatch")
    ids = {entry["id"] for entry in data["entries"]}
    require("doc-rag" in ids and "doc-ml" in ids, "expected RAG entries missing")


def main() -> int:
    (ROOT / "target/ai-examples/rag").mkdir(parents=True, exist_ok=True)
    run_step(
        "runtime RAG toolkit",
        [
            "cargo",
            "test",
            "-p",
            "spectra-runtime",
            "ml_phase18_rag_tokenizer_vector_index_and_prompt_eval",
        ],
    )
    run_step(
        "public Spectra validation",
        [
            "cargo",
            "run",
            "-p",
            "spectra-cli",
            "--",
            "run",
            "tests/validation/97_ml_phase18_rag_toolkit.spectra",
        ],
    )
    run_step(
        "AI RAG example",
        [
            "cargo",
            "run",
            "-p",
            "spectra-cli",
            "--",
            "run",
            "examples/ai/rag_retrieval_pipeline.spectra",
        ],
    )
    validate_index(ROOT / "target/ai-examples/rag/vector-index.json")
    report = ROOT / "target/ai-examples/rag/report.txt"
    require(report.exists(), "RAG report missing")
    require("retrieved=doc-rag" in report.read_text(encoding="utf-8"), "RAG retrieval evidence missing")
    print("[R-1803] validation passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
