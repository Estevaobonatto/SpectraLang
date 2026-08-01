"""Focused unit tests for the R-3103 benchmark/IR evidence gate."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.phase31_contract import SCENARIOS
from scripts.validate_r3103_optimization_plan import (
    PLAN_ITEM_IDS,
    REPORT_SCHEMA,
    baseline_unchanged,
    validate_plan_text,
    validate_report,
    validate_roadmap,
)


def report_fixture(*, duplicate: bool = False, dispersion: int = 1, revision: str = "rev") -> dict:
    ids = list(SCENARIOS)
    if duplicate:
        ids[-1] = ids[-2]
    scenarios = []
    for scenario_id in ids:
        spectra = {
            "ns_per_iter": 100,
            "median_ns": 100,
            "stddev_ns": 1,
            "independent_stddev_ns": dispersion,
            "exit_code": 0,
        }
        item = {
            "id": scenario_id,
            "correctness_passed": True,
            "results": {"spectra": spectra},
        }
        if scenario_id == "async-echo":
            item.update(
                {
                    "reference_performance_passed": True,
                    "paired_gap_stddev_pct": 1.0,
                }
            )
        scenarios.append(item)
    return {
        "schema": REPORT_SCHEMA,
        "profile": "release",
        "spectra_binary": r"D:\Lang\SpectraLang\target\release\spectralang.exe",
        "git_revision": revision,
        "measurement_policy": {"independent_runs": 5, "warmup_runs": 3, "timed_runs": 20},
        "scenarios": scenarios,
    }


class R3103ValidatorTests(unittest.TestCase):
    def test_matrix_incomplete(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "plan.md"
            path.write_text("# plan\nbenchmark_and_ir_hypothesis rejection rollback\n", encoding="utf-8")
            errors = validate_plan_text(path)
        self.assertTrue(any("R-3104" in error for error in errors))

    def test_item_without_metric(self) -> None:
        row = "| R-3104 | cpu | evidence | hypothesis | intervention |  | gain | risk | rollback | dep | command |"
        text = "\n".join([row, *[f"| {item} | s | e | h | i | m | g | r | b | d | c |" for item in PLAN_ITEM_IDS[1:]], "benchmark_and_ir_hypothesis rejection rollback"])
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "plan.md"
            path.write_text(text, encoding="utf-8")
            errors = validate_plan_text(path)
        self.assertTrue(any("R-3104" in error and "metric" in error for error in errors))

    def test_duplicate_scenario(self) -> None:
        errors = validate_report(
            report_fixture(duplicate=True),
            root=Path.cwd(),
            expected_revision="rev",
            baseline_hash="baseline",
        )
        self.assertTrue(any("canonical 21 scenarios" in error for error in errors))

    def test_revision_divergence(self) -> None:
        errors = validate_report(
            report_fixture(),
            root=Path.cwd(),
            expected_revision="current",
            baseline_hash="baseline",
        )
        self.assertTrue(any("Git revision" in error for error in errors))

    def test_baseline_modified(self) -> None:
        self.assertFalse(baseline_unchanged("before", "after"))
        self.assertTrue(baseline_unchanged("same", "same"))

    def test_inconclusive_report(self) -> None:
        errors = validate_report(
            report_fixture(dispersion=20),
            root=Path.cwd(),
            expected_revision="rev",
            baseline_hash="baseline",
        )
        self.assertTrue(any("inconclusive" in error for error in errors))

    def test_benchmark_only_classification(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "plan.md"
            rows = [f"| {item} | s | e | h | i | metric | gain | risk | rollback | dep | command |" for item in PLAN_ITEM_IDS]
            path.write_text("\n".join(rows) + "\nbenchmark_and_ir_hypothesis rejection rollback\n", encoding="utf-8")
            errors = validate_plan_text(path)
        self.assertEqual([], errors)

    def test_rejects_unsupported_causal_claim(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "plan.md"
            rows = [f"| {item} | s | e | h | i | metric | gain | risk | rollback | dep | command |" for item in PLAN_ITEM_IDS]
            path.write_text("\n".join(rows) + "\nbenchmark_and_ir_hypothesis rejection rollback\nprofiling proves the causal hotspot\n", encoding="utf-8")
            errors = validate_plan_text(path)
        self.assertTrue(any("unsupported causal" in error for error in errors))

    def test_r3102_stays_in_progress(self) -> None:
        roadmap = {
            "phases": [{"id": "phase_31"}],
            "items": [
                {"id": "R-3101", "phase": "phase_31", "dependencies": [], "status": "complete"},
                {"id": "R-3102", "phase": "phase_31", "dependencies": ["R-3101"], "status": "in_progress"},
                {"id": "R-3103", "phase": "phase_31", "dependencies": ["R-3101"], "status": "in_progress"},
            ],
        }
        self.assertEqual([], validate_roadmap(roadmap))


if __name__ == "__main__":
    unittest.main()
