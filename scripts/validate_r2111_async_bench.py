#!/usr/bin/env python3
"""Validate R-2111 async benchmark report shape and baseline thresholds."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BASELINE = ROOT / "docs" / "performance" / "r2111-async-benchmark-baseline.json"
REPORT = ROOT / "target" / "r2111-async-bench-report.json"


def fail(message: str) -> None:
    print(f"[R-2111] FAIL: {message}", file=sys.stderr)
    sys.exit(1)


def load_json(path: Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        fail(f"missing JSON file: {path}")
    except json.JSONDecodeError as exc:
        fail(f"invalid JSON in {path}: {exc}")


def expected_checksum(count: int) -> int:
    return count * (count + 1) // 2


def main() -> int:
    baseline = load_json(BASELINE)
    if baseline.get("schema") != "spectra.r2111.async_benchmark_baseline.v1":
        fail("baseline schema mismatch")

    REPORT.parent.mkdir(parents=True, exist_ok=True)
    command = [
        "cargo",
        "run",
        "-q",
        "-p",
        "spectra-cli",
        "--",
        "bench",
        "--async",
        "--bench-json",
        str(REPORT),
    ]
    completed = subprocess.run(command, cwd=ROOT, text=True)
    if completed.returncode != 0:
        fail(f"benchmark command failed with exit code {completed.returncode}")

    report = load_json(REPORT)
    expected_schema = baseline.get("benchmark_schema")
    if report.get("schema") != expected_schema:
        fail(f"report schema mismatch: expected {expected_schema!r}, got {report.get('schema')!r}")
    if report.get("version") != 1:
        fail("report version must be 1")

    required = list(baseline.get("required_concurrency", []))
    benchmarks = report.get("benchmarks")
    if not isinstance(benchmarks, list):
        fail("report.benchmarks must be a list")

    by_count = {case.get("concurrent_tasks"): case for case in benchmarks}
    if sorted(by_count) != required:
        fail(f"benchmark concurrency set mismatch: expected {required}, got {sorted(by_count)}")

    thresholds = baseline.get("thresholds", {})
    for count in required:
        case = by_count[count]
        case_id = f"ready_task_{count}"
        if case.get("id") != case_id:
            fail(f"{count}: expected id {case_id!r}, got {case.get('id')!r}")
        if case.get("concurrent_connections") != count:
            fail(f"{case_id}: concurrent_connections must match concurrent_tasks")
        samples = case.get("samples")
        if not isinstance(samples, int) or samples <= 0 or samples > count:
            fail(f"{case_id}: invalid sample count {samples!r}")

        p50 = case.get("p50_latency_ns")
        p95 = case.get("p95_latency_ns")
        p99 = case.get("p99_latency_ns")
        if not all(isinstance(value, int) and value >= 0 for value in (p50, p95, p99)):
            fail(f"{case_id}: latency percentiles must be non-negative integers")
        if not (p50 <= p95 <= p99):
            fail(f"{case_id}: latency percentiles are not ordered")

        throughput = case.get("throughput_tasks_per_sec")
        if not isinstance(throughput, (int, float)) or throughput <= 0:
            fail(f"{case_id}: throughput must be positive")
        if case.get("checksum") != expected_checksum(count):
            fail(f"{case_id}: checksum does not cover the full task set")

        limit = thresholds.get(case_id)
        if not isinstance(limit, dict):
            fail(f"{case_id}: missing baseline threshold")
        if p99 > limit.get("max_p99_latency_ns", -1):
            fail(f"{case_id}: p99 {p99} exceeds baseline {limit.get('max_p99_latency_ns')}")
        if throughput < limit.get("min_throughput_tasks_per_sec", float("inf")):
            fail(
                f"{case_id}: throughput {throughput:.2f} below baseline "
                f"{limit.get('min_throughput_tasks_per_sec')}"
            )

    totals = report.get("totals", {})
    if totals.get("cases") != len(required):
        fail("totals.cases mismatch")
    if totals.get("total_tasks") != sum(required):
        fail("totals.total_tasks mismatch")

    print(
        "[R-2111] async benchmark validation passed: "
        + ", ".join(f"{count} tasks" for count in required)
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
