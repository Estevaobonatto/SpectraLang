#!/usr/bin/env python3
"""Validate R-1802 transformer and LLM runtime primitive gates."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def run_step(name: str, args: list[str]) -> None:
    print(f"[R-1802] {name}: {' '.join(args)}")
    completed = subprocess.run(args, cwd=ROOT, text=True)
    if completed.returncode != 0:
        raise SystemExit(completed.returncode)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def main() -> int:
    (ROOT / "target/ai-examples").mkdir(parents=True, exist_ok=True)
    run_step(
        "runtime transformer primitives",
        [
            "cargo",
            "test",
            "-p",
            "spectra-runtime",
            "ml_phase18_transformer_primitives_and_sampling",
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
            "tests/validation/96_ml_phase18_transformer_primitives.spectra",
        ],
    )
    run_step(
        "AI toy transformer example",
        [
            "cargo",
            "run",
            "-p",
            "spectra-cli",
            "--",
            "run",
            "examples/ai/toy_transformer_inference.spectra",
        ],
    )
    artifact = ROOT / "target/ai-examples/toy_transformer_inference.txt"
    require(artifact.exists(), "toy transformer artifact missing")
    text = artifact.read_text(encoding="utf-8")
    require("primitive_attention=1" in text, "attention primitive evidence missing")
    require("kv_cache_len=2" in text, "KV cache evidence missing")
    print("[R-1802] validation passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
