"""Focused unit tests for the R-3133 async-echo reconciliation gate."""

from __future__ import annotations

import copy
import unittest
from pathlib import Path

from scripts.phase31_contract import SCENARIOS
from scripts.validate_r3133_async_echo_reconciliation import (
    BATCH_VARIANTS,
    CONTRACT,
    DIAGNOSTIC_SCHEMA,
    REPORT_SCHEMA,
    classify_cause,
    validate_diagnostic,
    validate_report,
)


REVISION = "95b04bdead6e60207c0fdf9688ef6de774dc87a1"


def diagnostic_fixture(*, missing_batch_full: bool = False, revision: str = REVISION) -> dict:
    variants = {
        "startup": {"median_ns": 1_000, "stddev_pct": 1.0, "contract": "legacy_single_task", "process_inclusive": True, "ok": True},
    }
    for name in BATCH_VARIANTS:
        if name == "batch-full" and missing_batch_full:
            continue
        variants[name] = {
            "median_ns": 10_000,
            "stddev_pct": 1.0,
            "contract": CONTRACT,
            "process_inclusive": True,
            "ok": True,
            "diagnostics": {key: 1 for key in (
                "locks_acquired", "scheduler_ns", "execution_ns", "tasks_counted",
                "tasks_created", "tasks_executed", "task_joins", "batches_created",
                "batches_joined", "batch_spawn_fast_abi_calls", "batch_join_fast_abi_calls",
                "max_pending_tasks",
            )},
        }
    return {
        "schema": DIAGNOSTIC_SCHEMA,
        "git_revision": revision,
        "profile": "release",
        "spectra_binary": r"D:\Lang\SpectraLang\target\release\spectralang.exe",
        "workload_contract": CONTRACT,
        "variants": variants,
    }


def report_fixture(*, gap: float = 1.0, dispersion: float = 1.0, revision: str = REVISION) -> dict:
    scenarios = []
    for scenario_id in SCENARIOS:
        item = {
            "id": scenario_id,
            "correctness_passed": True,
            "results": {"spectra": {"ns_per_iter": 100}},
        }
        if scenario_id == "async-echo":
            item.update({
                "gap_to_go": gap,
                "paired_gap_stddev_pct": dispersion,
                "reference_performance_passed": True,
            })
        if scenario_id == "async-pipeline":
            item["results"]["spectra"]["ns_per_iter"] = 52_467_360
        scenarios.append(item)
    return {
        "schema": REPORT_SCHEMA,
        "profile": "release",
        "git_revision": revision,
        "spectra_binary": r"D:\Lang\SpectraLang\target\release\spectralang.exe",
        "measurement_policy": {"independent_runs": 5, "warmup_runs": 3, "timed_runs": 20},
        "scenarios": scenarios,
    }


class R3133ValidatorTests(unittest.TestCase):
    def test_schema_v2_and_all_batch_variants(self) -> None:
        self.assertEqual([], validate_diagnostic(diagnostic_fixture(), expected_revision=REVISION, binary_suffix=r"target\release\spectralang.exe"))

    def test_missing_batch_full_is_rejected(self) -> None:
        errors = validate_diagnostic(diagnostic_fixture(missing_batch_full=True), expected_revision=REVISION, binary_suffix=r"target\release\spectralang.exe")
        self.assertTrue(any("batch-full" in error for error in errors))

    def test_revision_divergence_is_rejected(self) -> None:
        errors = validate_diagnostic(diagnostic_fixture(revision="bd48a6b"), expected_revision=REVISION, binary_suffix=r"target\release\spectralang.exe")
        self.assertTrue(any("revision" in error.lower() for error in errors))

    def test_causal_profiling_claim_is_rejected_without_perf_artifact(self) -> None:
        diagnostic = diagnostic_fixture()
        diagnostic["causal_profiling"] = True
        errors = validate_diagnostic(diagnostic, expected_revision=REVISION, binary_suffix=r"target\release\spectralang.exe")
        self.assertTrue(any("causal profiling" in error for error in errors))

    def test_report_requires_current_parity(self) -> None:
        baseline = {"scenarios": {"async-pipeline": {"spectra_ns_per_iter": 52_467_360}}}
        errors = validate_report(report_fixture(gap=1.12), expected_revision=REVISION, baseline=baseline)
        self.assertTrue(any("async-echo" in error for error in errors))

    def test_async_pipeline_improvement_is_not_a_regression(self) -> None:
        baseline = {"scenarios": {"async-pipeline": {"spectra_ns_per_iter": 100}}}
        report = report_fixture()
        next(item for item in report["scenarios"] if item["id"] == "async-pipeline")["results"]["spectra"]["ns_per_iter"] = 50
        errors = validate_report(report, expected_revision=REVISION, baseline=baseline)
        self.assertFalse(any("async-pipeline" in error for error in errors))

    def test_noise_is_classified_separately_from_runtime_regression(self) -> None:
        noisy = classify_cause(diagnostic_fixture(), [report_fixture(dispersion=11.0)])
        self.assertEqual("external_noise", noisy["category"])
        runtime = classify_cause(diagnostic_fixture(), [report_fixture(dispersion=1.0)])
        self.assertEqual("runtime_batch_path", runtime["category"])

    def test_diagnostic_fixture_does_not_mutate_source(self) -> None:
        original = diagnostic_fixture()
        modified = copy.deepcopy(original)
        modified["variants"]["batch-full"]["median_ns"] = 99
        self.assertEqual(10_000, original["variants"]["batch-full"]["median_ns"])


if __name__ == "__main__":
    unittest.main()
