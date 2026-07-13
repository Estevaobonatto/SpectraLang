from __future__ import annotations

import subprocess
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

from scripts import validate_phase31_cross_lang as cross_lang
from scripts import validate_r1603_gpu_backend as gpu_backend
from scripts import phase31_run_all as phase31_runner


def report(*, profile: str = "debug", stddev_ns: int = 5, ns_per_iter: int = 100) -> dict:
    scenarios = []
    for scenario_id in cross_lang.REQUIRED_SCENARIOS:
        scenarios.append(
            {
                "id": scenario_id,
                "correctness_passed": True,
                "results": {
                    "spectra": {
                        "ns_per_iter": ns_per_iter,
                        "stddev_ns": stddev_ns,
                    }
                },
            }
        )
    return {
        "schema": "spectra.phase31.bench.v1",
        "profile": profile,
        "spectra_binary": str(Path("target/debug/spectralang.exe").resolve()),
        "measurement_policy": {
            "warmup_runs": 3,
            "timed_runs": 20,
            "max_stddev_pct": 10.0,
        },
        "scenarios": scenarios,
    }


def baseline() -> dict:
    return {
        "max_drift_pct": 15.0,
        "max_stddev_pct": 10.0,
        "scenarios": {
            scenario_id: {"spectra_ns_per_iter": 100, "placeholder": False}
            for scenario_id in cross_lang.REQUIRED_SCENARIOS
        },
    }


class Phase31GateTests(unittest.TestCase):
    def test_independent_attempts_are_aggregated(self) -> None:
        attempts = [
            {
                "id": "sample",
                "category": "cpu",
                "iterations": 1,
                "results": {"spectra": {"ns_per_iter": value, "stddev_ns": 1}},
                "correctness_passed": True,
            }
            for value in (100, 120, 110)
        ]
        aggregated = phase31_runner.aggregate_scenario_attempts(attempts)
        self.assertEqual(aggregated["results"]["spectra"]["ns_per_iter"], 110)
        self.assertEqual(aggregated["results"]["spectra"]["independent_medians_ns"], [100, 120, 110])
        self.assertEqual(aggregated["independent_runs"], 3)

    def test_stable_report_passes(self) -> None:
        failures, inconclusive = cross_lang.check_baseline(baseline(), report())
        self.assertEqual(failures, [])
        self.assertEqual(inconclusive, [])

    def test_noisy_report_is_inconclusive(self) -> None:
        failures, inconclusive = cross_lang.check_baseline(
            baseline(), report(stddev_ns=11)
        )
        self.assertEqual(failures, [])
        self.assertTrue(inconclusive)

    def test_real_regression_remains_failure(self) -> None:
        failures, inconclusive = cross_lang.check_baseline(
            baseline(), report(stddev_ns=5, ns_per_iter=120)
        )
        self.assertTrue(failures)
        self.assertEqual(inconclusive, [])

    def test_profile_metadata_mismatch_is_rejected_by_contract(self) -> None:
        value = report(profile="release")
        self.assertTrue(cross_lang.validate_report_metadata(value, "debug", None))

    def test_measurement_policy_metadata_is_required(self) -> None:
        value = report()
        value["measurement_policy"]["timed_runs"] = 12
        self.assertTrue(cross_lang.validate_report_metadata(value, "debug", None))


class R1603ValidatorTests(unittest.TestCase):
    @patch("scripts.validate_r1603_gpu_backend.subprocess.run")
    def test_timeout_is_deterministic(self, run: Mock) -> None:
        run.side_effect = subprocess.TimeoutExpired(["cargo"], 120)
        with self.assertRaises(SystemExit) as raised:
            gpu_backend.run_step("gpu test", ["cargo", "test"], timeout_s=120)
        self.assertEqual(raised.exception.code, 124)

    @patch("scripts.validate_r1603_gpu_backend.subprocess.run")
    def test_step_captures_failure_output(self, run: Mock) -> None:
        run.return_value = subprocess.CompletedProcess(
            ["cargo", "test"], 1, stdout="stdout", stderr="stderr"
        )
        with self.assertRaises(SystemExit) as raised:
            gpu_backend.run_step("gpu test", ["cargo", "test"])
        self.assertEqual(raised.exception.code, 1)
        run.assert_called_once()
        self.assertEqual(run.call_args.kwargs["timeout"], 120)


if __name__ == "__main__":
    unittest.main()
