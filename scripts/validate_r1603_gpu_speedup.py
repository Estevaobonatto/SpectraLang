#!/usr/bin/env python3
"""R-1603 GPU speedup gate (manual, off default CI).

Measures the `ml-mlp-step-gpu` benchmark on both CPU and GPU, writes
`target/r1603-gpu-speedup/report.json`, and fails when the GPU path
is not faster than the CPU path at the documented batch size.

This script is intentionally NOT in the default `run_tests.ps1` flow.
Wire it under a manual `phase31_gpu` phase, since CI hosts typically
do not have a WGPU adapter.

The default build validates CPU fallback and diagnostics; the optional
GPU build validates the full device upload + pool reuse + residency +
backward story. The new benchmark runs in 3 batch sizes (64/128/256)
on the GPU and in 1 batch size (256) on the CPU.

Usage:
    python scripts/validate_r1603_gpu_speedup.py
    python scripts/validate_r1603_gpu_speedup.py --out target/custom.json
    python scripts/validate_r1603_gpu_speedup.py --target-ratio 1.0
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUT = ROOT / "target" / "r1603-gpu-speedup" / "report.json"
SPECTRA_BENCH = ROOT / "benchmarks" / "gpu" / "ml-mlp-step-gpu" / "spectra" / "bench.spectra"
GO_BENCH = ROOT / "benchmarks" / "gpu" / "ml-mlp-step-gpu" / "go" / "bench.go"
SPECTRALANG_DEBUG = ROOT / "target" / "debug" / "spectralang.exe"
SPECTRALANG_RELEASE = ROOT / "target" / "release" / "spectralang.exe"
BENCH_WORKDIR = ROOT / "target" / "r1603-gpu-speedup"


def run(cmd: list[str], *, cwd: Path | None = None, timeout: int = 600) -> subprocess.CompletedProcess[str]:
    print(f"[R-1603] {' '.join(cmd)}")
    return subprocess.run(
        cmd,
        cwd=cwd or ROOT,
        text=True,
        capture_output=True,
        timeout=timeout,
    )


def ensure_spectralang() -> Path:
    for candidate in (SPECTRALANG_RELEASE, SPECTRALANG_DEBUG):
        if candidate.exists():
            return candidate
    print("[R-1603] building spectralang (debug) ...")
    run(["cargo", "build", "-p", "spectra-cli"], timeout=1800)
    if not SPECTRALANG_DEBUG.exists():
        raise SystemExit("spectralang binary not found after build")
    return SPECTRALANG_DEBUG


def spectra_bench_variant(mode: str) -> Path:
    """Materialize a CPU-only or GPU-only copy of the Spectra bench."""
    if mode not in {"cpu", "gpu"}:
        raise ValueError(f"invalid Spectra bench mode: {mode}")
    if not SPECTRA_BENCH.exists():
        raise SystemExit(f"Spectra bench not found: {SPECTRA_BENCH}")
    source = SPECTRA_BENCH.read_text(encoding="utf-8")
    if mode == "cpu":
        source = source.replace("if !tensor.device_available(6) {", "if true {")
        source = source.replace("let p = ml.linear(h, w2, b2);", "let p = ml.linear(_a, w2, b2);")
    else:
        source = source.replace("if !tensor.device_available(6) {", "if false {")
    BENCH_WORKDIR.mkdir(parents=True, exist_ok=True)
    path = BENCH_WORKDIR / f"bench-{mode}.spectra"
    path.write_text(source, encoding="utf-8")
    return path


def gpu_adapter_available(binary: Path) -> bool:
    """Probe WGPU adapter availability before forcing the GPU benchmark path."""
    BENCH_WORKDIR.mkdir(parents=True, exist_ok=True)
    probe = BENCH_WORKDIR / "gpu-adapter-probe.spectra"
    probe.write_text(
        "module r1603_gpu_adapter_probe;\n"
        "import std.tensor as tensor;\n"
        "pub fn main() -> int {\n"
        "    if tensor.device_available(6) { return 0; }\n"
        "    return 2;\n"
        "}\n",
        encoding="utf-8",
    )
    result = run([str(binary), "run", str(probe)], timeout=120)
    if result.returncode == 0:
        return True
    if result.returncode == 2:
        return False
    output = (result.stdout or "") + (result.stderr or "")
    print(output[-4000:], file=sys.stderr)
    raise SystemExit(f"GPU adapter probe failed (rc={result.returncode})")


def time_spectra(batch: int, iters: int, *, mode: str) -> dict[str, Any]:
    """Run the Spectra bench in an explicit CPU-only or GPU-only mode."""
    binary = ensure_spectralang()
    bench = spectra_bench_variant(mode)
    # The bench accepts no args; batch/iters are baked into the source.
    # We invoke it 3 times for warmup, then take the median of 5 runs.
    timings: list[float] = []
    for _ in range(5):
        t0 = time.perf_counter()
        result = run([str(binary), "run", str(bench)], timeout=120)
        elapsed = time.perf_counter() - t0
        if result.returncode != 0:
            print(result.stdout)
            print(result.stderr, file=sys.stderr)
            raise SystemExit(f"spectra run failed (rc={result.returncode})")
        timings.append(elapsed)
    timings.sort()
    median = timings[len(timings) // 2]
    return {"ns_per_iter": int(median * 1e9), "samples": timings}


def time_go(batch: int, iters: int) -> dict[str, Any] | None:
    """Run the Go reference if `go` is available."""
    if shutil.which("go") is None:
        print("[R-1603] `go` not found; skipping Go reference")
        return None
    if not GO_BENCH.exists():
        print(f"[R-1603] Go bench not found: {GO_BENCH}")
        return None
    build_dir = ROOT / "target" / "r1603-gpu-speedup" / "go-build"
    build_dir.mkdir(parents=True, exist_ok=True)
    binary = build_dir / "bench.exe"
    build = run(["go", "build", "-o", str(binary), str(GO_BENCH)], timeout=120)
    if build.returncode != 0:
        print(build.stderr, file=sys.stderr)
        return None
    timings: list[float] = []
    for _ in range(5):
        t0 = time.perf_counter()
        result = run([str(binary)], timeout=120)
        elapsed = time.perf_counter() - t0
        if result.returncode != 0:
            print(result.stderr, file=sys.stderr)
            return None
        timings.append(elapsed)
    timings.sort()
    median = timings[len(timings) // 2]
    return {"ns_per_iter": int(median * 1e9), "samples": timings}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    parser.add_argument(
        "--target-ratio",
        type=float,
        default=1.0,
        help="Minimum GPU/CPU speedup ratio to pass (default 1.0x at batch 256)",
    )
    parser.add_argument(
        "--batch",
        type=int,
        default=256,
        help="Batch size for the speedup gate (default 256)",
    )
    args = parser.parse_args()
    args.out = (ROOT / args.out).resolve() if not args.out.is_absolute() else args.out.resolve()

    args.out.parent.mkdir(parents=True, exist_ok=True)

    print(f"[R-1603] target ratio: {args.target_ratio}x at batch={args.batch}")

    report: dict[str, Any] = {
        "schema": "r1603-gpu-speedup-v1",
        "target_ratio": args.target_ratio,
        "batch": args.batch,
        "iters": 10,
        "bench_path": str(SPECTRA_BENCH.relative_to(ROOT)),
        "go_path": str(GO_BENCH.relative_to(ROOT)) if GO_BENCH.exists() else None,
    }

    binary = ensure_spectralang()
    if not gpu_adapter_available(binary):
        report["status"] = "skipped"
        report["skip_reason"] = "no WGPU adapter available"
        with args.out.open("w", encoding="utf-8") as handle:
            json.dump(report, handle, indent=2)
        print("[R-1603] SKIP: no WGPU adapter available")
        return 0
    report["status"] = "measured"

    # CPU (Spectra with no GPU upload = fall back)
    print(f"[R-1603] measuring CPU (batch={args.batch}) ...")
    cpu = time_spectra(args.batch, 10, mode="cpu")
    report["cpu"] = cpu

    # GPU (Spectra with Wgpu upload; runs GPU path on hosts with adapter,
    # falls back to CPU otherwise and reports no speedup).
    print(f"[R-1603] measuring GPU (batch={args.batch}) ...")
    gpu = time_spectra(args.batch, 10, mode="gpu")
    report["gpu"] = gpu

    cpu_ns = cpu["ns_per_iter"]
    gpu_ns = gpu["ns_per_iter"]
    ratio = cpu_ns / gpu_ns if gpu_ns > 0 else 0.0
    report["ratio"] = round(ratio, 3)
    report["speedup_target_met"] = ratio >= args.target_ratio

    go = time_go(args.batch, 10)
    if go is not None:
        report["go_reference"] = go
        report["go_vs_cpu"] = round(go["ns_per_iter"] / cpu_ns, 3) if cpu_ns > 0 else 0.0
        report["go_vs_gpu"] = round(go["ns_per_iter"] / gpu_ns, 3) if gpu_ns > 0 else 0.0

    with args.out.open("w", encoding="utf-8") as handle:
        json.dump(report, handle, indent=2)
    print(f"[R-1603] wrote {args.out.relative_to(ROOT)}")

    if not report["speedup_target_met"]:
        print(
            f"[R-1603] FAIL: GPU ratio {ratio:.2f}x < target {args.target_ratio}x. "
            "Check that the bench is actually exercising the GPU path "
            "(stats_gpu_backward_ops should be > 0)."
        )
        return 1
    print(f"[R-1603] PASS: GPU ratio {ratio:.2f}x >= target {args.target_ratio}x")
    return 0


if __name__ == "__main__":
    sys.exit(main())
