#!/usr/bin/env python3
"""Validate R-1903 model monitoring and drift detection gates."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def run_step(name: str, args: list[str]) -> None:
    print(f"[R-1903] {name}: {' '.join(args)}")
    completed = subprocess.run(args, cwd=ROOT, text=True)
    if completed.returncode != 0:
        raise SystemExit(completed.returncode)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def validate_export(path: Path) -> None:
    data = json.loads(path.read_text(encoding="utf-8"))
    require(data["schema"] == "spectra.serve.monitoring_export.v1", "bad monitoring export schema")
    snapshot = data["snapshot"]
    require(snapshot["schema"] == "spectra.serve.monitoring_snapshot.v1", "bad snapshot schema")
    require("model_version" in snapshot, "model version missing")
    require(snapshot["requests"] >= 2, "request metric missing")
    require("latency_p95_ms" in snapshot, "latency metric missing")
    require("error_rate" in snapshot, "error metric missing")
    require("throughput_per_second" in snapshot, "throughput metric missing")
    distribution = data["distribution"]
    require(distribution["schema"] == "spectra.serve.distribution_summary.v1", "bad distribution schema")
    require(distribution["inputs"]["count"] >= 2, "input distribution missing")
    drift = data["drift"]
    require(drift["schema"] == "spectra.serve.drift_check.v1", "bad drift schema")
    require(drift["drifted"] is True, "expected drift detection")
    audit = data["audit"]
    require(audit["schema"] == "spectra.serve.audit.v1", "bad audit schema")


def main() -> int:
    (ROOT / "target/ai-examples/monitoring").mkdir(parents=True, exist_ok=True)
    run_step(
        "runtime monitoring",
        [
            "cargo",
            "test",
            "-p",
            "spectra-runtime",
            "serve_host_calls_cover_monitoring_drift_and_export",
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
            "tests/validation/100_phase19_model_monitoring.spectra",
        ],
    )
    run_step(
        "AI monitoring example",
        [
            "cargo",
            "run",
            "-p",
            "spectra-cli",
            "--",
            "run",
            "examples/ai/model_monitoring_drift_detection.spectra",
        ],
    )
    validate_export(ROOT / "target/ai-examples/monitoring/model-monitoring.json")
    summary = ROOT / "target/ai-examples/monitoring/summary.txt"
    require(summary.exists(), "monitoring summary missing")
    require("drifted=true" in summary.read_text(encoding="utf-8"), "monitoring drift evidence missing")
    print("[R-1903] validation passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
