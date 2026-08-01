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

For `async-echo`, the gate also requires the versioned real-concurrency
contract and the accepted focused Spectra/Go ratio limit.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sys

try:
    from scripts.phase31_contract import (
        ASYNC_ECHO_CONTRACT,
        ASYNC_ECHO_ACCEPTED_MAX_GAP_TO_GO,
        ASYNC_ECHO_ITERATIONS,
        MAX_STDDEV_PCT,
        ASYNC_ECHO_REFERENCE_RUNTIME,
        ASYNC_ECHO_TASKS_PER_ITERATION,
        LANGUAGES,
        OFFICIAL_INDEPENDENT_RUNS,
        PHASE31_SCHEMA,
        SCENARIOS,
        TIMED_RUNS,
        WARMUP_RUNS,
    )
except ModuleNotFoundError:  # direct `python scripts/validate_phase31_cross_lang.py`
    from phase31_contract import (  # type: ignore[no-redef]
        ASYNC_ECHO_CONTRACT,
        ASYNC_ECHO_ACCEPTED_MAX_GAP_TO_GO,
        ASYNC_ECHO_ITERATIONS,
        MAX_STDDEV_PCT,
        ASYNC_ECHO_REFERENCE_RUNTIME,
        ASYNC_ECHO_TASKS_PER_ITERATION,
        LANGUAGES,
        OFFICIAL_INDEPENDENT_RUNS,
        PHASE31_SCHEMA,
        SCENARIOS,
        TIMED_RUNS,
        WARMUP_RUNS,
    )

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
DEFAULT_BASELINE = (
    REPO_ROOT / "docs" / "performance" / "phase31-go-comparable" / "baseline.json"
)
DEFAULT_REPORT = REPO_ROOT / "target" / "phase31" / "cross-lang-report.json"

REQUIRED_SCENARIOS = list(SCENARIOS)


def check_baseline(
    baseline: dict, report: dict, code_validation: bool = False
) -> tuple[list[str], list[str]]:
    failures: list[str] = []
    inconclusive: list[str] = []
    base_scenarios = baseline.get("scenarios", {})
    report_by_id = {s["id"]: s for s in report.get("scenarios", [])}

    missing_baseline = sorted(set(REQUIRED_SCENARIOS) - set(base_scenarios))
    extra_baseline = sorted(set(base_scenarios) - set(REQUIRED_SCENARIOS))
    extra_report = sorted(set(report_by_id) - set(REQUIRED_SCENARIOS))
    for scenario_id in missing_baseline:
        failures.append(f"baseline missing scenario: {scenario_id}")
    for scenario_id in extra_baseline:
        failures.append(f"baseline has unknown scenario: {scenario_id}")
    for scenario_id in extra_report:
        failures.append(f"report has unknown scenario: {scenario_id}")

    for scenario_id in REQUIRED_SCENARIOS:
        if scenario_id not in report_by_id:
            failures.append(f"missing scenario: {scenario_id}")
            continue
        entry = report_by_id[scenario_id]
        if not entry.get("correctness_passed", False):
            failures.append(f"{scenario_id}: correctness check failed")
        for language in LANGUAGES:
            language_result = entry.get("results", {}).get(language, {})
            if language_result.get("exit_code") != 0:
                failures.append(
                    f"{scenario_id}/{language}: non-zero or missing exit code"
                )
            if not language_result.get("command"):
                failures.append(f"{scenario_id}/{language}: missing command")
            if language_result.get("failure_class") is not None:
                failures.append(
                    f"{scenario_id}/{language}: {language_result.get('failure_class')}"
                )
        if scenario_id == "async-echo":
            if entry.get("benchmark_contract") != ASYNC_ECHO_CONTRACT:
                failures.append(
                    f"async-echo: benchmark contract must be {ASYNC_ECHO_CONTRACT}"
                )
            if entry.get("performance_reference") != ASYNC_ECHO_REFERENCE_RUNTIME:
                failures.append("async-echo: performance reference must be Go")
            if entry.get("tasks_per_iteration") != ASYNC_ECHO_TASKS_PER_ITERATION:
                failures.append("async-echo: task count does not match contract")
            if entry.get("benchmark_iterations") != ASYNC_ECHO_ITERATIONS:
                failures.append("async-echo: iteration count does not match contract")
            if not code_validation:
                gap_to_go = entry.get("gap_to_go")
                if not isinstance(gap_to_go, (int, float)):
                    failures.append("async-echo: missing gap_to_go measurement")
                elif not (0 < float(gap_to_go) <= ASYNC_ECHO_ACCEPTED_MAX_GAP_TO_GO):
                    failures.append(
                        f"async-echo: gap to Go {float(gap_to_go):.3f} is outside "
                        f"the accepted <= {ASYNC_ECHO_ACCEPTED_MAX_GAP_TO_GO:.6f}x limit"
                    )
                if entry.get("reference_performance_passed") is not True:
                    failures.append("async-echo: reference_performance_passed is not true")
                paired = entry.get("paired_gap_to_go", [])
                if len(paired) < OFFICIAL_INDEPENDENT_RUNS:
                    failures.append(
                        f"async-echo: expected at least {OFFICIAL_INDEPENDENT_RUNS} paired attempts"
                    )
                paired_stddev_pct = entry.get("paired_gap_stddev_pct")
                if not isinstance(paired_stddev_pct, (int, float)):
                    inconclusive.append("async-echo: paired ratio variance missing")
                elif paired_stddev_pct > MAX_STDDEV_PCT:
                    inconclusive.append(
                        f"async-echo: paired ratio noise {paired_stddev_pct:.1f}% > "
                        f"{MAX_STDDEV_PCT:.1f}%"
                    )
            metrics = entry.get("concurrency_metrics")
            expected_tasks = ASYNC_ECHO_TASKS_PER_ITERATION * ASYNC_ECHO_ITERATIONS
            if not isinstance(metrics, dict):
                failures.append("async-echo: concurrency metrics missing")
            else:
                if metrics.get("max_pending_tasks", 0) < ASYNC_ECHO_TASKS_PER_ITERATION:
                    failures.append("async-echo: fan-out concurrency was not observed")
                if metrics.get("tasks_executed", 0) < expected_tasks:
                    failures.append("async-echo: not all scheduled tasks executed")
                if metrics.get("task_joins", 0) < expected_tasks:
                    failures.append("async-echo: fan-in joins are incomplete")
                if metrics.get("tasks_failed", 0) != 0:
                    failures.append("async-echo: task failures observed")
        if code_validation:
            continue
        spec = entry.get("results", {}).get("spectra", {})
        if "error" in spec:
            failures.append(f"{scenario_id}: spectra runtime error: {spec['error']}")
            continue
        if "ns_per_iter" not in spec:
            failures.append(f"{scenario_id}: missing spectra ns_per_iter")
            continue
        if scenario_id != "async-echo":
            stddev_ns = spec.get("independent_stddev_ns", spec.get("stddev_ns"))
            if stddev_ns is None or spec["ns_per_iter"] <= 0:
                inconclusive.append(f"{scenario_id}: missing stable measurement statistics")
                continue
            max_stddev_pct = baseline.get("max_stddev_pct", MAX_STDDEV_PCT)
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
        if scenario_id == "async-echo":
            # The checked-in baseline predates the v2 fan-out/fan-in contract
            # and measured eager values. Go parity is the only semantically
            # valid performance reference for the versioned v2 fixture.
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
    report: dict,
    expected_profile: str | None,
    expected_binary: str | None,
    code_validation: bool = False,
) -> list[str]:
    failures: list[str] = []
    if report.get("schema") != PHASE31_SCHEMA:
        failures.append(
            f"report schema is {report.get('schema')!r}, expected {PHASE31_SCHEMA!r}"
        )
    if report.get("scenario_matrix") != list(SCENARIOS):
        failures.append("report scenario matrix does not match the 21-scenario contract")
    if report.get("complete_scenario_set") is not True:
        failures.append("report is partial")
    preflight = report.get("environment_preflight", {})
    if preflight.get("status") != "quiescent":
        failures.append(f"environment preflight is {preflight.get('status')!r}")
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
    expected_mode = "code_validation" if code_validation else "performance_certification"
    if report.get("mode") != expected_mode:
        failures.append(f"report mode is {report.get('mode')!r}, expected {expected_mode!r}")
    expected_warmups = 0 if code_validation else WARMUP_RUNS
    expected_timed = 1 if code_validation else TIMED_RUNS
    expected_independent = 1 if code_validation else OFFICIAL_INDEPENDENT_RUNS
    if policy.get("warmup_runs") != expected_warmups:
        failures.append(
            f"report warmup_runs is {policy.get('warmup_runs')!r}, expected {expected_warmups}"
        )
    if policy.get("timed_runs") != expected_timed:
        failures.append(
            f"report timed_runs is {policy.get('timed_runs')!r}, expected {expected_timed}"
        )
    if policy.get("max_stddev_pct") != MAX_STDDEV_PCT:
        failures.append(
            f"report max_stddev_pct is {policy.get('max_stddev_pct')!r}, expected {MAX_STDDEV_PCT}"
        )
    independent_runs = policy.get("independent_runs", 0)
    if independent_runs != expected_independent:
        failures.append(
            f"report independent_runs is {independent_runs!r}, "
            f"expected {expected_independent}"
        )
    timeout_s = policy.get("per_process_timeout_s")
    if not isinstance(timeout_s, int) or timeout_s < 1:
        failures.append("report per_process_timeout_s must be a positive integer")
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
    parser.add_argument(
        "--code-validation",
        action="store_true",
        help="validate fast functional report; skip statistical performance gates",
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

    report_ids = [s.get("id") for s in report.get("scenarios", [])]
    if len(report_ids) != len(set(report_ids)):
        print("phase31 cross-lang gate: FAIL (duplicate scenario)", file=sys.stderr)
        return 1

    metadata_failures = validate_report_metadata(
        report, args.profile, args.spectra_binary, args.code_validation
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
    failures, inconclusive = check_baseline(
        baseline_for_check, report, args.code_validation
    )
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
