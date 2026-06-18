#!/usr/bin/env python3
"""Run and validate the R-2006 tensor/std performance refresh evidence."""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import shutil
import subprocess
import sys


REQUIRED_CATEGORIES = {
    "materialization",
    "elementwise_chains",
    "reductions",
    "matmul",
    "autodiff",
    "buffer_reuse",
}

BENCHMARK_SCHEMA = "spectra.r2006.performance_refresh.v1"
BASELINE_SCHEMA = "spectra.r2006.performance_baseline.v1"


def resolve_cargo() -> str:
    explicit = os.environ.get("CARGO")
    if explicit:
        return explicit
    cargo = shutil.which("cargo")
    if cargo:
        return cargo
    home = pathlib.Path.home()
    candidates = [
        home / ".cargo" / "bin" / "cargo.exe",
        home / ".cargo" / "bin" / "cargo",
    ]
    for candidate in candidates:
        if candidate.exists():
            return str(candidate)
    raise FileNotFoundError("cargo was not found in PATH, CARGO, or ~/.cargo/bin")


def extract_json(stdout: str) -> dict:
    start = stdout.find("{")
    end = stdout.rfind("}")
    if start < 0 or end < start:
        raise ValueError("benchmark did not emit a JSON object")
    return json.loads(stdout[start : end + 1])


def load_json(path: pathlib.Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def benchmark_items(report: dict) -> dict[str, dict]:
    return {str(item.get("id")): item for item in report.get("benchmarks", [])}


def validate_report_shape(report: dict, label: str, failures: list[str]) -> dict[str, dict]:
    if report.get("schema") != BENCHMARK_SCHEMA:
        failures.append(f"{label}: unexpected schema {report.get('schema')}")
    if report.get("profile") != "release":
        failures.append(f"{label}: benchmark must be release, got {report.get('profile')}")
    if not report.get("passed", False):
        failures.append(f"{label}: report marked passed=false")
    items = benchmark_items(report)
    categories = {item.get("category") for item in items.values()}
    missing_categories = sorted(REQUIRED_CATEGORIES - categories)
    if missing_categories:
        failures.append(f"{label}: missing categories {', '.join(missing_categories)}")
    for bench_id, item in sorted(items.items()):
        if not item.get("correctness_passed", False):
            failures.append(f"{label}: {bench_id} correctness failed")
        if int(item.get("iterations", 0)) <= 0:
            failures.append(f"{label}: {bench_id} has no iterations")
        if int(item.get("ns_per_iter", 0)) <= 0:
            failures.append(f"{label}: {bench_id} has no positive timing")
    memory = report.get("memory", {})
    for key in (
        "allocations",
        "peak_bytes",
        "reused_buffers",
        "pool_hits",
        "scratch_reuses",
        "reuse_rate_per_mille",
        "kernel_ops",
        "kernel_elements",
    ):
        if int(memory.get(key, 0)) <= 0:
            failures.append(f"{label}: memory.{key} must be positive")
    return items


def validate_against_baseline(
    observed: dict,
    checked_in_report: dict,
    baseline: dict,
    failures: list[str],
) -> None:
    if baseline.get("schema") != BASELINE_SCHEMA:
        failures.append(f"baseline: unexpected schema {baseline.get('schema')}")
    if baseline.get("profile") != "release":
        failures.append(f"baseline: expected release profile, got {baseline.get('profile')}")

    baseline_items = baseline.get("benchmarks", {})
    observed_items = benchmark_items(observed)
    checked_items = benchmark_items(checked_in_report)

    missing_observed = sorted(set(baseline_items) - set(observed_items))
    missing_checked = sorted(set(baseline_items) - set(checked_items))
    if missing_observed:
        failures.append(f"observed: missing benchmark ids {', '.join(missing_observed)}")
    if missing_checked:
        failures.append(f"checked-in report: missing benchmark ids {', '.join(missing_checked)}")

    for bench_id, config in sorted(baseline_items.items()):
        for label, items in (("observed", observed_items), ("checked-in report", checked_items)):
            item = items.get(bench_id)
            if not item:
                continue
            if item.get("category") != config.get("category"):
                failures.append(
                    f"{label}: {bench_id} category changed from "
                    f"{config.get('category')} to {item.get('category')}"
                )
            actual_ns = int(item.get("ns_per_iter", -1))
            max_ns = int(config.get("max_ns_per_iter", 0))
            if actual_ns <= 0:
                failures.append(f"{label}: {bench_id} missing ns_per_iter")
            elif actual_ns > max_ns:
                failures.append(
                    f"{label}: {bench_id} {actual_ns} ns/iter exceeds threshold {max_ns}"
                )

    minimums = baseline.get("memory_minimums", {})
    for key, minimum in sorted(minimums.items()):
        for label, report in (
            ("observed", observed),
            ("checked-in report", checked_in_report),
        ):
            actual = int(report.get("memory", {}).get(key, 0))
            if actual < int(minimum):
                failures.append(f"{label}: memory.{key} {actual} is below minimum {minimum}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--baseline",
        default="docs/performance/r2006-performance-baseline.json",
        help="checked-in R-2006 baseline JSON",
    )
    parser.add_argument(
        "--checked-in-report",
        default="docs/performance/r2006-performance-report.json",
        help="checked-in R-2006 benchmark evidence report",
    )
    parser.add_argument(
        "--output",
        default="target/r2006-performance-report.json",
        help="path where the observed benchmark JSON should be written",
    )
    args = parser.parse_args()

    baseline_path = pathlib.Path(args.baseline)
    checked_report_path = pathlib.Path(args.checked_in_report)
    output_path = pathlib.Path(args.output)
    baseline = load_json(baseline_path)
    checked_in_report = load_json(checked_report_path)

    command = [
        resolve_cargo(),
        "run",
        "--release",
        "-p",
        "spectra-runtime",
        "--example",
        "r2006_tensor_performance_refresh",
    ]
    proc = subprocess.run(command, text=True, capture_output=True, check=False)
    if proc.returncode != 0:
        sys.stderr.write(proc.stdout)
        sys.stderr.write(proc.stderr)
        return proc.returncode or 1

    observed = extract_json(proc.stdout)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(observed, indent=2) + "\n", encoding="utf-8")

    failures: list[str] = []
    validate_report_shape(observed, "observed", failures)
    validate_report_shape(checked_in_report, "checked-in report", failures)
    validate_against_baseline(observed, checked_in_report, baseline, failures)

    if failures:
        for failure in failures:
            print(f"R-2006 performance failure: {failure}", file=sys.stderr)
        print(f"Observed report: {output_path}", file=sys.stderr)
        return 1

    print(
        "R-2006 performance refresh ok: "
        f"{len(benchmark_items(observed))} benchmarks, report={output_path.as_posix()}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
