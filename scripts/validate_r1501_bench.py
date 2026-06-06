#!/usr/bin/env python3
"""Run and validate the R-1501 release numerical benchmark suite."""

from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
import sys


REQUIRED_CATEGORIES = {
    "tensor_creation",
    "unary_ops",
    "reductions",
    "matmul",
    "convolution",
    "autodiff",
    "optimizer_steps",
    "data_loading",
}


def extract_json(stdout: str) -> dict:
    start = stdout.find("{")
    end = stdout.rfind("}")
    if start < 0 or end < start:
        raise ValueError("benchmark did not emit a JSON object")
    return json.loads(stdout[start : end + 1])


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--baseline",
        default="docs/performance/r1501-benchmark-baseline.json",
        help="checked-in R-1501 baseline JSON",
    )
    parser.add_argument(
        "--output",
        default="target/r1501-benchmark-report.json",
        help="path where the observed benchmark JSON should be written",
    )
    args = parser.parse_args()

    baseline_path = pathlib.Path(args.baseline)
    output_path = pathlib.Path(args.output)
    baseline = json.loads(baseline_path.read_text(encoding="utf-8"))

    command = [
        "cargo",
        "run",
        "--release",
        "-p",
        "spectra-runtime",
        "--example",
        "numerical_performance_bench",
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
    if observed.get("schema") != "spectra.r1501.benchmark.v1":
        failures.append(f"unexpected benchmark schema: {observed.get('schema')}")
    if observed.get("profile") != "release":
        failures.append(f"benchmark must run in release profile, got {observed.get('profile')}")
    if not observed.get("passed", False):
        failures.append("benchmark reported correctness failure")

    baseline_items = baseline.get("benchmarks", {})
    observed_items = {item.get("id"): item for item in observed.get("benchmarks", [])}
    missing = sorted(set(baseline_items) - set(observed_items))
    if missing:
        failures.append(f"missing benchmark ids: {', '.join(missing)}")

    categories = {item.get("category") for item in observed_items.values()}
    missing_categories = sorted(REQUIRED_CATEGORIES - categories)
    if missing_categories:
        failures.append(f"missing categories: {', '.join(missing_categories)}")

    for bench_id, config in baseline_items.items():
        item = observed_items.get(bench_id)
        if not item:
            continue
        if item.get("category") != config.get("category"):
            failures.append(
                f"{bench_id}: category changed from {config.get('category')} to {item.get('category')}"
            )
        if not item.get("correctness_passed", False):
            failures.append(f"{bench_id}: correctness failed")
        max_ns = int(config["max_ns_per_iter"])
        actual_ns = int(item.get("ns_per_iter", max_ns + 1))
        if actual_ns > max_ns:
            failures.append(f"{bench_id}: {actual_ns} ns/iter exceeds threshold {max_ns}")

    if failures:
        for failure in failures:
            print(f"R-1501 benchmark failure: {failure}", file=sys.stderr)
        print(f"Observed report: {output_path}", file=sys.stderr)
        return 1

    print(
        f"R-1501 benchmark ok: {len(observed_items)} benchmarks, report={output_path.as_posix()}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
