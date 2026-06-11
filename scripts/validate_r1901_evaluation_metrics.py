#!/usr/bin/env python3
"""Validate R-1901 model evaluation metrics and report gates."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def run_step(name: str, args: list[str]) -> None:
    print(f"[R-1901] {name}: {' '.join(args)}")
    completed = subprocess.run(args, cwd=ROOT, text=True)
    if completed.returncode != 0:
        raise SystemExit(completed.returncode)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def validate_report(path: Path) -> None:
    data = json.loads(path.read_text(encoding="utf-8"))
    require(data["schema"] == "spectra.ml.evaluation_report.v1", "bad evaluation report schema")
    require(data["classification"]["kind"] == "classification", "classification metrics missing")
    require(data["classification"]["accuracy"] == 1.0, "classification accuracy mismatch")
    require("roc_auc_baseline" in data["classification"], "ROC-AUC baseline missing")
    require(data["regression"]["mse"] == 0.0, "regression MSE mismatch")
    require(data["regression"]["mae"] == 0.0, "regression MAE mismatch")
    require("ndcg_at_k" in data["ranking"], "ranking NDCG missing")
    require("perplexity" in data["generation"], "generation perplexity missing")
    require("latency_p95_ms" in data["serving"], "serving latency missing")
    require("throughput_per_second" in data["serving"], "serving throughput missing")
    human = Path(f"{path}.txt")
    require(human.exists(), "human-readable evaluation report missing")
    require("Spectra ML Evaluation Report" in human.read_text(encoding="utf-8"), "human report header missing")


def main() -> int:
    (ROOT / "target/ai-examples/evaluation").mkdir(parents=True, exist_ok=True)
    run_step(
        "runtime evaluation metrics",
        [
            "cargo",
            "test",
            "-p",
            "spectra-runtime",
            "ml_phase19_evaluation_metrics_and_report",
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
            "tests/validation/98_ml_phase19_evaluation_metrics.spectra",
        ],
    )
    run_step(
        "AI evaluation example",
        [
            "cargo",
            "run",
            "-p",
            "spectra-cli",
            "--",
            "run",
            "examples/ai/model_evaluation_report.spectra",
        ],
    )
    validate_report(ROOT / "target/ai-examples/evaluation/model-evaluation.json")
    summary = ROOT / "target/ai-examples/evaluation/summary.txt"
    require(summary.exists(), "evaluation example summary missing")
    require("status=pass" in summary.read_text(encoding="utf-8"), "evaluation example status missing")
    print("[R-1901] validation passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
