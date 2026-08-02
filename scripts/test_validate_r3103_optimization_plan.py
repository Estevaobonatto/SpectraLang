"""Focused unit tests for the R-3103 benchmark/IR evidence gate."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.generate_r3103_ir import file_record, manifest_payload
from scripts.phase31_contract import (
    ASYNC_ECHO_ACCEPTED_MAX_GAP_TO_GO,
    LANGUAGES,
    SCENARIOS,
)
from scripts.validate_r3103_optimization_plan import (
    PLAN_ITEM_IDS,
    REPORT_SCHEMA,
    baseline_unchanged,
    collect_ir,
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
        def result(ns_per_iter: int) -> dict:
            return {
                "ns_per_iter": ns_per_iter,
                "median_ns": ns_per_iter,
                "stddev_ns": 1,
                "independent_stddev_ns": dispersion,
                "exit_code": 0,
            }

        item = {
            "id": scenario_id,
            "correctness_passed": True,
            "results": {
                "spectra": result(100),
                "go": result(100),
                "rust": result(80),
            },
        }
        if scenario_id == "async-echo":
            item.update(
                {
                    "gap_to_go": 1.0,
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
        "complete_scenario_set": True,
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

    def test_async_echo_accepts_the_user_approved_limit(self) -> None:
        report = report_fixture()
        async_echo = next(item for item in report["scenarios"] if item["id"] == "async-echo")
        async_echo["gap_to_go"] = ASYNC_ECHO_ACCEPTED_MAX_GAP_TO_GO
        self.assertEqual(
            [],
            validate_report(
                report,
                root=Path.cwd(),
                expected_revision="rev",
                baseline_hash="baseline",
            ),
        )

    def test_async_echo_rejects_gap_above_approved_limit(self) -> None:
        report = report_fixture()
        async_echo = next(item for item in report["scenarios"] if item["id"] == "async-echo")
        async_echo["gap_to_go"] = ASYNC_ECHO_ACCEPTED_MAX_GAP_TO_GO + 0.000001
        errors = validate_report(
            report,
            root=Path.cwd(),
            expected_revision="rev",
            baseline_hash="baseline",
        )
        self.assertTrue(any("1.202162x" in error for error in errors))

    def test_matrix_requires_spectra_go_and_rust_without_java(self) -> None:
        report = report_fixture()
        report["scenarios"][0]["results"].pop("rust")
        errors = validate_report(
            report,
            root=Path.cwd(),
            expected_revision="rev",
            baseline_hash="baseline",
        )
        self.assertTrue(any("missing active benchmark languages rust" in error for error in errors))

        report = report_fixture()
        report["scenarios"][0]["results"]["java"] = dict(report["scenarios"][0]["results"]["go"])
        errors = validate_report(
            report,
            root=Path.cwd(),
            expected_revision="rev",
            baseline_hash="baseline",
        )
        self.assertTrue(any("Java is excluded" in error for error in errors))

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
                {"id": "R-3104", "phase": "phase_31", "dependencies": ["R-3103"], "status": "not_started"},
            ],
        }
        self.assertEqual([], validate_roadmap(roadmap))

    def test_r3104_in_progress_is_accepted_but_invalid_status_is_rejected(self) -> None:
        roadmap = {
            "phases": [{"id": "phase_31"}],
            "items": [
                {"id": "R-3101", "phase": "phase_31", "dependencies": [], "status": "complete"},
                {"id": "R-3102", "phase": "phase_31", "dependencies": ["R-3101"], "status": "in_progress"},
                {"id": "R-3103", "phase": "phase_31", "dependencies": ["R-3101"], "status": "complete"},
                {"id": "R-3104", "phase": "phase_31", "dependencies": ["R-3103"], "status": "in_progress"},
            ],
        }
        self.assertEqual([], validate_roadmap(roadmap))
        roadmap["items"][-1]["status"] = "blocked"
        self.assertTrue(any("R-3104" in error for error in validate_roadmap(roadmap)))

    def test_ir_manifest_is_required_and_complete(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "target" / "release" / "spectralang.exe"
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b"release-binary")
            ir_root = root / "target" / "phase31" / "r3103-ir"
            files = {}
            for scenario in SCENARIOS:
                scenario_root = ir_root / scenario
                scenario_root.mkdir(parents=True)
                scenario_files = {}
                for level in ("o0", "o3"):
                    path = scenario_root / f"{level}.txt"
                    path.write_text(f"{scenario}-{level}\n", encoding="utf-8")
                    scenario_files[level] = file_record(path, f"{scenario}/{level}.txt")
                files[scenario] = scenario_files
            manifest = manifest_payload(
                root=root,
                binary=binary,
                output_root=ir_root,
                revision="rev",
                files=files,
            )
            (ir_root / "manifest.json").write_text(
                __import__("json").dumps(manifest), encoding="utf-8"
            )
            self.assertEqual([], collect_ir(root=root, ir_root=ir_root, expected_revision="rev")["errors"])

            (ir_root / "manifest.json").unlink()
            self.assertTrue(any("manifest.json is missing" in error for error in collect_ir(root=root, ir_root=ir_root, expected_revision="rev")["errors"]))

    def test_ir_manifest_rejects_incomplete_and_incompatible_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "target" / "release" / "spectralang.exe"
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b"release-binary")
            ir_root = root / "target" / "phase31" / "r3103-ir"
            ir_root.mkdir(parents=True)
            manifest = {
                "schema": "spectra.phase31.r3103_ir_manifest.v1",
                "git_revision": "old-revision",
                "profile": "release",
                "binary": "target/release/spectralang.exe",
                "binary_sha256": "wrong",
                "options": {"o0": [], "o3": []},
                "scenario_count": 1,
                "scenarios": [],
                "files": {},
            }
            (ir_root / "manifest.json").write_text(
                __import__("json").dumps(manifest), encoding="utf-8"
            )
            errors = collect_ir(root=root, ir_root=ir_root, expected_revision="rev")["errors"]
        self.assertTrue(any("Git revision" in error for error in errors))
        self.assertTrue(any("scenario_count" in error for error in errors))
        self.assertTrue(any("missing IR o0.txt" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
