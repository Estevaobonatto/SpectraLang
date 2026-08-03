"""Focused tests for the R-3105 hostcall batching evidence gate."""

from __future__ import annotations

import copy
import hashlib
import tempfile
import unittest
from pathlib import Path

from scripts.phase31_contract import LANGUAGES, SCENARIOS
from scripts.validate_r3105_hostcall_batching import (
    BENCHMARK_SCHEMA,
    REPORT_SCHEMA,
    validate_baseline_sha256,
    validate_benchmark,
    validate_code_report,
    validate_release_reports,
    validate_roadmap,
)


REVISION = "r3105-test-revision"


def benchmark_fixture() -> dict:
    def group() -> dict:
        return {
            "warmup_runs": 3,
            "timed_runs": 20,
            "exit_code": 0,
            "failure_class": None,
            "timings": {"median_ns": 100},
        }

    return {
        "schema": BENCHMARK_SCHEMA,
        "task": "R-3105",
        "classification": "benchmark_and_ir_hypothesis",
        "profiling_causal_claim": False,
        "git_revision": REVISION,
        "profile": "release",
        "benchmark_languages": ["spectra"],
        "java_excluded": True,
        "source_tree_fingerprint": "candidate-tree",
        "binary_sha256": "candidate-binary",
        "control": {
            "source_tree_fingerprint": "control-tree",
            "binary_sha256": "control-binary",
        },
        "measurement_policy": {
            "warmup_runs": 3,
            "timed_runs": 20,
            "independent_runs": 5,
            "aggregation": "median_of_group_medians",
            "runtime_measurement": "precompiled_aot_executable",
        },
        "candidate_compile": {"exit_code": 0, "output_sha256": "candidate-aot"},
        "control_compile": {"exit_code": 0, "output_sha256": "control-aot"},
        "candidate_batch_stats": {
            "batched_sites": 1,
            "batched_hostcalls": 3,
            "fallback_hostcalls": 0,
            "argument_arena_bytes": 40,
            "result_arena_bytes": 24,
        },
        "candidate_runtime": {
            "groups": [group() for _ in range(5)],
            "successful_independent_runs": 5,
            "median_of_group_medians_ns": 80,
        },
        "control_runtime": {
            "groups": [group() for _ in range(5)],
            "successful_independent_runs": 5,
            "median_of_group_medians_ns": 100,
        },
        "candidate_to_control_ratio": 0.8,
        "required_max_ratio": 0.9,
        "speedup_gate_passed": True,
        "correctness_passed": True,
        "control_correctness_passed": True,
    }


def release_fixture(*, async_gap: float = 1.1, java: bool = False) -> dict:
    entries = []
    for scenario in SCENARIOS:
        results = {
            language: {
                "command": [language],
                "exit_code": 0,
                "failure_class": None,
                "ns_per_iter": 100,
                "independent_stddev_ns": 1,
            }
            for language in LANGUAGES
        }
        if java:
            results["java"] = {"command": ["java"], "exit_code": 0, "failure_class": None}
        entry = {
            "id": scenario,
            "category": "test",
            "iterations": 1,
            "correctness_passed": True,
            "results": results,
        }
        if scenario == "async-echo":
            entry.update(
                {
                    "gap_to_go": async_gap,
                    "reference_performance_passed": async_gap <= 1.202162,
                    "paired_gap_stddev_pct": 1.0,
                }
            )
        entries.append(entry)
    return {
        "schema": REPORT_SCHEMA,
        "profile": "release",
        "spectra_binary": "target\\release\\spectralang.exe",
        "git_revision": REVISION,
        "complete_scenario_set": True,
        "measurement_policy": {"warmup_runs": 3, "timed_runs": 20, "independent_runs": 5},
        "scenarios": entries,
    }


class R3105ValidatorTests(unittest.TestCase):
    def test_accepts_valid_dedicated_batch_benchmark(self) -> None:
        self.assertEqual([], validate_benchmark(benchmark_fixture(), expected_revision=REVISION))

    def test_rejects_speedup_threshold(self) -> None:
        payload = benchmark_fixture()
        payload["candidate_to_control_ratio"] = 0.91
        payload["speedup_gate_passed"] = False
        errors = validate_benchmark(payload, expected_revision=REVISION)
        self.assertTrue(any("speedup" in error for error in errors))

    def test_rejects_same_fingerprint_and_wrong_policy(self) -> None:
        payload = benchmark_fixture()
        payload["control"]["source_tree_fingerprint"] = "candidate-tree"
        payload["measurement_policy"]["timed_runs"] = 19
        errors = validate_benchmark(payload, expected_revision=REVISION)
        self.assertTrue(any("fingerprints" in error for error in errors))
        self.assertTrue(any("policy" in error for error in errors))

    def test_rejects_missing_or_empty_batch_stats(self) -> None:
        payload = benchmark_fixture()
        payload["candidate_batch_stats"] = {
            "batched_sites": 0,
            "batched_hostcalls": 1,
            "fallback_hostcalls": 0,
            "argument_arena_bytes": 0,
            "result_arena_bytes": 0,
        }
        errors = validate_benchmark(payload, expected_revision=REVISION)
        self.assertTrue(any("at least one batched site" in error for error in errors))
        self.assertTrue(any("at least two hostcalls" in error for error in errors))
        self.assertTrue(any("arena" in error for error in errors))
        payload["candidate_batch_stats"] = None
        self.assertTrue(any("statistics are missing" in error for error in validate_benchmark(payload, expected_revision=REVISION)))

    def test_rejects_java_and_functional_or_aot_failure(self) -> None:
        payload = benchmark_fixture()
        payload["benchmark_languages"] = ["spectra", "java"]
        payload["candidate_compile"]["exit_code"] = 1
        payload["candidate_runtime"]["groups"][0]["exit_code"] = 1
        errors = validate_benchmark(payload, expected_revision=REVISION)
        self.assertTrue(any("only Spectra" in error for error in errors))
        self.assertTrue(any("AOT compilation" in error for error in errors))
        self.assertTrue(any("runtime correctness" in error for error in errors))

    def test_code_validation_rejects_java_and_functional_failure(self) -> None:
        report = release_fixture()
        report["scenarios"][0]["results"]["java"] = {"exit_code": 0}
        report["scenarios"][1]["correctness_passed"] = False
        errors = validate_code_report(report, expected_revision=REVISION)
        self.assertTrue(any("Spectra + Go + Rust" in error for error in errors))
        self.assertTrue(any("correctness" in error for error in errors))

    def test_release_reports_reject_async_gap_and_incompatible_reports(self) -> None:
        first = release_fixture(async_gap=1.30)
        second = copy.deepcopy(first)
        second["scenarios"][0]["category"] = "different"
        baseline = {"scenarios": {scenario: {"spectra_ns_per_iter": 100} for scenario in SCENARIOS}}
        errors = validate_release_reports(
            [first, second],
            root=Path.cwd(),
            expected_revision=REVISION,
            baseline_hash="baseline",
            baseline=baseline,
        )
        self.assertTrue(any("async-echo" in error for error in errors))
        self.assertTrue(any("semantically" in error for error in errors))

    def test_release_reports_reject_java(self) -> None:
        report = release_fixture(java=True)
        baseline = {"scenarios": {scenario: {"spectra_ns_per_iter": 100} for scenario in SCENARIOS}}
        errors = validate_release_reports(
            [report, copy.deepcopy(report)],
            root=Path.cwd(),
            expected_revision=REVISION,
            baseline_hash="baseline",
            baseline=baseline,
        )
        self.assertTrue(any("Java" in error for error in errors))

    def test_baseline_sha256_is_immutable(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "baseline.json"
            path.write_bytes(b"baseline")
            digest = hashlib.sha256(b"baseline").hexdigest()
            self.assertEqual([], validate_baseline_sha256(path, digest))
            self.assertTrue(validate_baseline_sha256(path, "changed"))
            path.write_bytes(b"changed")
            self.assertTrue(validate_baseline_sha256(path, digest))

    def test_roadmap_allows_r3105_in_progress_but_closes_followups(self) -> None:
        items = [
            {"id": "R-3102", "status": "in_progress"},
            {"id": "R-3103", "status": "complete"},
            {"id": "R-3104", "status": "complete"},
            {"id": "R-3105", "status": "in_progress", "dependencies": ["R-3103", "R-3104"]},
        ] + [{"id": f"R-{number}", "status": "not_started"} for number in range(3106, 3118)]
        self.assertEqual([], validate_roadmap({"items": items}))
        items[-2]["status"] = "in_progress"
        self.assertTrue(any("R-3116" in error for error in validate_roadmap({"items": items})))


if __name__ == "__main__":
    unittest.main()
