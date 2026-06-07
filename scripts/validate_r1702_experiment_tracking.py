#!/usr/bin/env python3
"""Validate R-1702 experiment tracking and reproducibility gates."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def run_step(name: str, args: list[str]) -> None:
    print(f"[R-1702] {name}: {' '.join(args)}")
    completed = subprocess.run(args, cwd=ROOT, text=True)
    if completed.returncode != 0:
        raise SystemExit(completed.returncode)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def validate_manifest(path: Path) -> None:
    manifest = json.loads(path.read_text(encoding="utf-8"))
    require(manifest["schema"] == "spectra.ml.experiment.v1", "bad experiment schema")
    require(manifest["seed"] == 2026, "example seed was not recorded")
    require(manifest["metrics"], "metrics missing from manifest")
    require(manifest["artifacts"], "artifacts missing from manifest")
    require(manifest["lockfile"] is not None, "lockfile missing from manifest")
    require(manifest["model_output"] is not None, "model output missing from manifest")
    require("spectralang run" in manifest["reproduction_command"], "missing repro command")


def main() -> int:
    (ROOT / "target/ai-examples").mkdir(parents=True, exist_ok=True)
    run_step(
        "runtime manifest and compare API",
        [
            "cargo",
            "test",
            "-p",
            "spectra-runtime",
            "ml_phase17_experiment_tracking_manifests_compare_and_repro_command",
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
            "tests/validation/93_ml_phase17_experiment_tracking.spectra",
        ],
    )
    run_step(
        "AI experiment example",
        [
            "cargo",
            "run",
            "-p",
            "spectra-cli",
            "--",
            "run",
            "examples/ai/experiment_tracking_reproducibility.spectra",
        ],
    )
    validate_manifest(ROOT / "target/ai-examples/experiment-run/experiment-manifest.json")
    print("[R-1702] validation passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
