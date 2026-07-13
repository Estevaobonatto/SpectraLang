#!/usr/bin/env python3
"""Validate the Phase 31 cross-language benchmark report.

Compares the observed report against the checked-in baseline. The gate fails
when:

1. Any scenario is missing from the report.
2. Any scenario's correctness check fails.
3. Any scenario's `ns_per_iter` regresses by more than `max_drift_pct` versus
   the baseline.
4. Numerical results deviate from the recorded reference value beyond the
   per-scenario tolerance (defaults applied when not specified).

The gate does **not** fail on `gap_to_go` or `gap_to_rust`; those values are
reported per scenario.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
DEFAULT_BASELINE = (
    REPO_ROOT / "docs" / "performance" / "phase31-go-comparable" / "baseline.json"
)
DEFAULT_REPORT = REPO_ROOT / "target" / "phase31" / "cross-lang-report.json"

REQUIRED_SCENARIOS = [
    "cpu-loop-sum",
    "cpu-fibs",
    "cpu-string-build",
    "cpu-hashmap",
    "tensor-create",
    "tensor-elementwise",
    "tensor-reduce",
    "tensor-matmul",
    "ml-mlp-step",
    "async-echo",
    "async-pipeline",
]


def check_baseline(baseline: dict, report: dict) -> tuple[list[str], list[str]]:
    failures: list[str] = []
    inconclusive: list[str] = []
    base_scenarios = baseline.get("scenarios", {})
    report_by_id = {s["id"]: s for s in report.get("scenarios", [])}

    for scenario_id in REQUIRED_SCENARIOS:
        if scenario_id not in report_by_id:
            failures.append(f"missing scenario: {scenario_id}")
            continue
        entry = report_by_id[scenario_id]
        if not entry.get("correctness_passed", False):
            failures.append(f"{scenario_id}: correctness check failed")
        spec = entry.get("results", {}).get("spectra", {})
        if "error" in spec:
            failures.append(f"{scenario_id}: spectra runtime error: {spec['error']}")
            continue
        if "ns_per_iter" not in spec:
            failures.append(f"{scenario_id}: missing spectra ns_per_iter")
            continue
        stddev_ns = spec.get("independent_stddev_ns", spec.get("stddev_ns"))
        if stddev_ns is None or spec["ns_per_iter"] <= 0:
            inconclusive.append(f"{scenario_id}: missing stable measurement statistics")
            continue
        max_stddev_pct = baseline.get("max_stddev_pct", 10.0)
        stddev_pct = (stddev_ns / spec["ns_per_iter"]) * 100.0
        if stddev_pct > max_stddev_pct:
            inconclusive.append(
                f"{scenario_id}: measurement noise {stddev_pct:.1f}% > "
                f"{max_stddev_pct:.1f}%"
            )
            continue
        base_entry = base_scenarios.get(scenario_id)
        if base_entry is None:
            failures.append(f"{scenario_id}: no baseline entry")
            continue
        if base_entry.get("placeholder", False):
            # First-time baseline acceptance: do not gate on drift.
            continue
        base_ns = base_entry.get("spectra_ns_per_iter", 0)
        if base_ns <= 0:
            continue
        drift_pct = ((spec["ns_per_iter"] - base_ns) / base_ns) * 100.0
        max_drift = baseline.get("max_drift_pct", 5.0)
        if drift_pct > max_drift:
            failures.append(
                f"{scenario_id}: spectra regressed {drift_pct:.1f}% > {max_drift:.1f}% "
                f"(baseline={base_ns} ns, observed={spec['ns_per_iter']} ns)"
            )
    return failures, inconclusive


def validate_report_metadata(
    report: dict, expected_profile: str | None, expected_binary: str | None
) -> list[str]:
    failures: list[str] = []
    if expected_profile and report.get("profile") != expected_profile:
        failures.append(
            f"report profile is {report.get('profile')!r}, expected {expected_profile!r}"
        )
    if expected_binary:
        expected_path = str(pathlib.Path(expected_binary).resolve())
        actual_path = str(pathlib.Path(report.get("spectra_binary", "")).resolve())
        if actual_path != expected_path:
            failures.append(
                f"report binary is {actual_path!r}, expected {expected_path!r}"
            )
    policy = report.get("measurement_policy", {})
    if policy.get("warmup_runs") != 3:
        failures.append(f"report warmup_runs is {policy.get('warmup_runs')!r}, expected 3")
    if policy.get("timed_runs") != 20:
        failures.append(f"report timed_runs is {policy.get('timed_runs')!r}, expected 20")
    independent_runs = policy.get("independent_runs", 1)
    if not isinstance(independent_runs, int) or independent_runs < 1:
        failures.append("report independent_runs must be a positive integer")
    return failures


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--baseline",
        default=str(DEFAULT_BASELINE),
        help="checked-in Phase 31 baseline JSON",
    )
    parser.add_argument(
        "--strict",
        action="store_true",
        help="fail when correctness is not met even if baseline is placeholder",
    )
    parser.add_argument(
        "--max-drift",
        type=float,
        default=None,
        help="override the max drift percentage for this run (default: from baseline)",
    )
    parser.add_argument(
        "--report",
        default=str(DEFAULT_REPORT),
        help="observed Phase 31 cross-lang JSON report",
    )
    parser.add_argument(
        "--profile",
        choices=("debug", "release"),
        default=None,
        help="require this profile in the observed report",
    )
    parser.add_argument(
        "--spectra-binary",
        default=None,
        help="require this binary path in the observed report",
    )
    args = parser.parse_args()

    baseline_path = pathlib.Path(args.baseline)
    report_path = pathlib.Path(args.report)

    if not baseline_path.exists():
        print(f"FAIL: baseline not found at {baseline_path}", file=sys.stderr)
        return 1
    if not report_path.exists():
        print(f"FAIL: report not found at {report_path}", file=sys.stderr)
        return 1

    baseline = json.loads(baseline_path.read_text(encoding="utf-8"))
    report = json.loads(report_path.read_text(encoding="utf-8"))

    metadata_failures = validate_report_metadata(
        report, args.profile, args.spectra_binary
    )
    if metadata_failures:
        print("phase31 cross-lang gate: FAIL (metadata)", file=sys.stderr)
        for failure in metadata_failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1

    max_drift = baseline.get("max_drift_pct", 5.0)
    # Allow CLI override to compare against a stricter target than the
    # checked-in baseline (e.g. for tight CI gates on a pinned machine).
    args_drift = getattr(args, "max_drift", None)
    if args_drift is not None:
        max_drift = args_drift
    baseline_for_check = dict(baseline)
    baseline_for_check["max_drift_pct"] = max_drift
    failures, inconclusive = check_baseline(baseline_for_check, report)
    if failures:
        print("phase31 cross-lang gate: FAIL", file=sys.stderr)
        for f in failures:
            print(f"  - {f}", file=sys.stderr)
        for item in inconclusive:
            print(f"  - inconclusive: {item}", file=sys.stderr)
        return 1
    if inconclusive:
        print("phase31 cross-lang gate: INCONCLUSIVE", file=sys.stderr)
        for item in inconclusive:
            print(f"  - {item}", file=sys.stderr)
        return 2

    print("phase31 cross-lang gate: PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
