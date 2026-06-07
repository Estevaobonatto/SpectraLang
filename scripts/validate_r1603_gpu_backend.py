#!/usr/bin/env python3
"""Validate R-1603 production GPU backend gates.

The default build must validate CPU fallback and diagnostics without native GPU
dependencies. The optional GPU build must compile and run the WGPU-backed test;
that test self-skips only when no adapter is available.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def run_step(name: str, args: list[str]) -> None:
    print(f"[R-1603] {name}: {' '.join(args)}")
    completed = subprocess.run(args, cwd=ROOT, text=True)
    if completed.returncode != 0:
        raise SystemExit(completed.returncode)


def main() -> int:
    run_step(
        "default CPU fallback and diagnostics",
        [
            "cargo",
            "test",
            "-p",
            "spectra-runtime",
            "tensor_runtime_r1603_default_cpu_fallback_and_diagnostics",
        ],
    )
    run_step(
        "optional WGPU backend diagnostics and backward coverage",
        [
            "cargo",
            "test",
            "-p",
            "spectra-runtime",
            "--features",
            "gpu",
            "tensor_runtime_r1603",
            "--",
            "--nocapture",
        ],
    )
    print("[R-1603] validation passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
