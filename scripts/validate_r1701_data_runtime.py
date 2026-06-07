#!/usr/bin/env python3
"""Validate R-1701 dataset/dataframe runtime gates."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def run_step(name: str, args: list[str]) -> None:
    print(f"[R-1701] {name}: {' '.join(args)}")
    completed = subprocess.run(args, cwd=ROOT, text=True)
    if completed.returncode != 0:
        raise SystemExit(completed.returncode)


def main() -> int:
    run_step(
        "runtime file loaders, dataframe, transforms, splits",
        [
            "cargo",
            "test",
            "-p",
            "spectra-runtime",
            "ml_phase17_dataset_dataframe_file_loaders_transforms_and_splits",
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
            "tests/validation/92_ml_phase17_data_runtime.spectra",
        ],
    )
    run_step(
        "tabular AI example",
        [
            "cargo",
            "run",
            "-p",
            "spectra-cli",
            "--",
            "run",
            "examples/ai/tabular_dataset_training.spectra",
        ],
    )
    print("[R-1701] validation passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
