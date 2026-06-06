#!/usr/bin/env python3
"""Run and validate the R-1503 numerical correctness certification artifact."""

from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
import sys


def extract_json(stdout: str) -> dict:
    start = stdout.find("{")
    end = stdout.rfind("}")
    if start < 0 or end < start:
        raise ValueError("certifier did not emit a JSON object")
    return json.loads(stdout[start : end + 1])


def close_enough(observed: float, expected: float, abs_tol: float, rel_tol: float) -> bool:
    diff = abs(observed - expected)
    return diff <= abs_tol or diff <= rel_tol * max(abs(expected), 1.0)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--baseline",
        default="docs/performance/r1503-correctness-baseline.json",
        help="checked-in R-1503 correctness baseline JSON",
    )
    parser.add_argument(
        "--output",
        default="target/r1503-correctness-report.json",
        help="path where the observed portable artifact should be written",
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
        "numerical_correctness_cert",
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
    if observed.get("schema") != baseline["portable_artifact_schema"]:
        failures.append(f"unexpected schema: {observed.get('schema')}")
    if not observed.get("passed", False):
        failures.append("certifier reported internal correctness failure")

    abs_tol = float(baseline["abs_tolerance"])
    rel_tol = float(baseline["rel_tolerance"])
    if float(observed.get("abs_tolerance", abs_tol)) > abs_tol:
        failures.append("observed absolute tolerance is weaker than baseline")
    if float(observed.get("rel_tolerance", rel_tol)) > rel_tol:
        failures.append("observed relative tolerance is weaker than baseline")

    observed_checks = {item.get("id"): item for item in observed.get("checks", [])}
    baseline_checks = baseline["checks"]
    missing = sorted(set(baseline_checks) - set(observed_checks))
    if missing:
        failures.append(f"missing checks: {', '.join(missing)}")

    categories = {item.get("category") for item in observed_checks.values()}
    missing_categories = sorted(set(baseline["required_categories"]) - categories)
    if missing_categories:
        failures.append(f"missing categories: {', '.join(missing_categories)}")

    for check_id, config in baseline_checks.items():
        item = observed_checks.get(check_id)
        if item is None:
            continue
        if item.get("category") != config["category"]:
            failures.append(
                f"{check_id}: category changed from {config['category']} to {item.get('category')}"
            )
        if not item.get("passed", False):
            failures.append(f"{check_id}: reported failed")
        expected = config.get("expected")
        if expected is not None:
            observed_value = float(item["observed"])
            if not close_enough(observed_value, float(expected), abs_tol, rel_tol):
                failures.append(
                    f"{check_id}: observed {observed_value} differs from expected {expected}"
                )

    if failures:
        for failure in failures:
            print(f"R-1503 correctness failure: {failure}", file=sys.stderr)
        print(f"Observed report: {output_path}", file=sys.stderr)
        return 1

    print(
        f"R-1503 correctness ok: {len(observed_checks)} checks, platform={observed.get('platform')}, report={output_path.as_posix()}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
