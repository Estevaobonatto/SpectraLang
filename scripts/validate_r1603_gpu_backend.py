#!/usr/bin/env python3
"""Validate R-1603 production GPU backend gates.

The default build must validate CPU fallback and diagnostics without native GPU
dependencies. The optional GPU build must compile and run the WGPU-backed test;
that test self-skips only when no adapter is available.
"""

from __future__ import annotations

import subprocess
import sys
import shutil
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CARGO = shutil.which("cargo") or str(Path.home() / ".cargo" / "bin" / "cargo.exe")


def run_step(name: str, args: list[str]) -> None:
    print(f"[R-1603] {name}: {' '.join(args)}")
    completed = subprocess.run(args, cwd=ROOT, text=True)
    if completed.returncode != 0:
        raise SystemExit(completed.returncode)


def main() -> int:
    run_step(
        "default CPU fallback and diagnostics",
        [
            CARGO,
            "test",
            "-p",
            "spectra-runtime",
            "tensor_runtime_r1603_default_cpu_fallback_and_diagnostics",
        ],
    )
    run_step(
        "optional WGPU backend diagnostics and backward coverage",
        [
            CARGO,
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
    # R-3023: typed GPU errors and per-kind stats counter.
    run_step(
        "typed GPU error counters (R-3023)",
        [
            CARGO,
            "test",
            "-p",
            "spectra-runtime",
            "--features",
            "gpu",
            "tensor_runtime_r3023_typed_gpu_errors_are_counted_per_kind",
            "--",
            "--nocapture",
        ],
    )
    # R-3021 / R-3051 / R-3052-minimal: real device upload, device buffer
    # pool reuse, and residency field. Tests self-skip on hosts without a
    # WGPU adapter.
    for name in (
        "tensor_runtime_r3021_real_upload_after_to_device",
        "tensor_runtime_r3051_pool_reuse_under_load",
        "tensor_runtime_r3051_pool_recycles_after_free",
        "tensor_runtime_r3052_device_resident_counter_tracks_to_device",
        "tensor_runtime_r3052_full_resident_matmul_matches_cpu",
        "tensor_runtime_r3052_full_resident_chain_stays_on_device",
        "tensor_runtime_r3052_full_resident_ml_linear_forward",
        "tensor_runtime_r3052_full_resident_relu_matches_cpu",
        "tensor_runtime_r3052_full_resident_binary_matches_cpu",
        "tensor_runtime_r3052_full_resident_releases_to_free_list",
        "tensor_runtime_r3052_full_resident_backward_accumulates_on_device",
        "tensor_runtime_r3052_full_resident_sgd_step_updates_device_param",
        "tensor_runtime_r3080_backward_kernels_match_cpu_within_tolerance",
    ):
        run_step(
            f"device upload + pool + residency + backward ({name})",
            [
                CARGO,
                "test",
                "-p",
                "spectra-runtime",
                "--features",
                "gpu",
                name,
                "--",
                "--nocapture",
            ],
        )
    print("[R-1603] validation passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
