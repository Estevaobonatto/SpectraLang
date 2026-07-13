from __future__ import annotations

import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from scripts import validate_r2013_release_candidate as r2013


def matrix() -> dict:
    return {
        "schema": r2013.MATRIX_SCHEMA,
        "status": "complete",
        "version": 1,
        "projects": [
            {
                "id": "sample",
                "project_path": "tests/projects/valid/integrated_basic_runtime",
                "entrypoint": "src/main.spectra",
                "command": "spectralang run",
                "exact_command": "spectralang run tests/projects/valid/integrated_basic_runtime",
                "required_files": [],
                "expected_outcome": "passes",
            }
        ],
    }


def runner(status: str = "passed") -> dict:
    project = {
        "project_id": "sample",
        "project_path": "tests/projects/valid/integrated_basic_runtime",
        "entrypoint": "src/main.spectra",
        "category": "basic_components",
        "command": "spectralang run",
        "exact_command": "spectralang run tests/projects/valid/integrated_basic_runtime",
        "status": status,
        "failure_class": None if status == "passed" else "runtime",
        "exit_code": 0 if status == "passed" else 1,
        "output_tail": "" if status == "passed" else "failure",
    }
    return {
        "schema": "spectralang.r2011_integrated_project_runner.v1",
        "roadmap_item": "R-2011",
        "matrix_schema": r2013.MATRIX_SCHEMA,
        "matrix_version": 1,
        "status": status,
        "summary": {"failed": 0 if status == "passed" else 1},
        "projects": [project],
    }


def conformance(certified: bool = True) -> dict:
    return {
        "schema": "spectralang.ai_conformance_report.v1",
        "conformance_version": "R-2001/v1",
        "certified": certified,
        "candidate_status": "certified" if certified else "rejected",
    }


def triage(untracked: int = 0) -> dict:
    return {
        "schema": "spectralang.r2012_failure_triage.v1",
        "roadmap_item": "R-2012",
        "runner_report": "target/r2011-integrated-project-runner/report.json",
        "status": "passed" if untracked == 0 else "failed",
        "summary": {"failed_projects": untracked, "untracked_failures": untracked},
        "triage": [],
    }


class R2013ValidationTests(unittest.TestCase):
    def test_matrix_contract_accepts_current_shape(self) -> None:
        self.assertEqual(r2013.validate_matrix(matrix()), [])

    def test_matrix_rejects_unknown_command(self) -> None:
        value = matrix()
        value["projects"][0]["command"] = "spectralang lint"
        self.assertTrue(r2013.validate_matrix(value))

    def test_matrix_rejects_missing_project_and_version(self) -> None:
        value = matrix()
        value["version"] = 2
        value["projects"] = []
        errors = r2013.validate_matrix(value)
        self.assertTrue(any("version" in error for error in errors))
        self.assertTrue(any("project" in error for error in errors))

    def test_matrix_rejects_invalid_schema(self) -> None:
        value = matrix()
        value["schema"] = "wrong.schema"
        self.assertTrue(r2013.validate_matrix(value))

    def test_conformance_requires_certification(self) -> None:
        self.assertEqual(r2013.validate_conformance_report(conformance()), [])
        self.assertTrue(r2013.validate_conformance_report(conformance(False)))

    def test_conformance_rejects_invalid_schema(self) -> None:
        value = conformance()
        value["schema"] = "wrong.schema"
        self.assertTrue(r2013.validate_conformance_report(value))

    def test_runner_rejects_project_failure(self) -> None:
        value = runner("failed")
        self.assertTrue(r2013.validate_runner_report(value, matrix()))

    def test_runner_rejects_matrix_mismatch(self) -> None:
        value = runner()
        value["matrix_version"] = 99
        self.assertTrue(r2013.validate_runner_report(value, matrix()))

    def test_triage_rejects_untracked_failures(self) -> None:
        self.assertTrue(r2013.validate_triage_report(triage(1), runner()))

    def test_predecessor_report_absence_is_an_error(self) -> None:
        with TemporaryDirectory() as directory:
            errors: list[str] = []
            value = r2013.read_or_error(Path(directory) / "missing.json", "R-2011", errors)
            self.assertEqual(value, {})
            self.assertTrue(errors)

    def test_report_passes_only_when_all_inputs_pass(self) -> None:
        errors: list[str] = []
        report = r2013.build_report("test", matrix(), conformance(), runner(), triage(), errors)
        self.assertEqual(report["status"], "passed")
        self.assertEqual(report["summary"]["projects_failed"], 0)
        self.assertEqual(report["follow_up_roadmap_items"], [])

    def test_report_records_follow_up_items_and_fails(self) -> None:
        triage_report = triage()
        triage_report["triage"] = [{"project_id": "sample", "tracked_by": "R-9999"}]
        errors: list[str] = []
        report = r2013.build_report("test", matrix(), conformance(), runner("failed"), triage_report, errors)
        self.assertEqual(report["status"], "failed")
        self.assertEqual(report["follow_up_roadmap_items"][0]["tracked_by"], "R-9999")


if __name__ == "__main__":
    unittest.main()
