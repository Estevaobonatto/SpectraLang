"""Focused tests for the R-3104 codegen hot-path evidence gate."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from scripts.phase31_contract import SCENARIOS
from scripts.validate_r3104_codegen_hot_path import (
    CODEGEN_SCHEMA,
    CODEGEN_SCENARIOS,
    IR_MANIFEST_SCHEMA,
    IR_OPTIONS,
    SNAPSHOT_ROOT,
    SNAPSHOT_SCENARIOS,
    STEADY_STATE_SCENARIOS,
    validate_baseline_drift,
    validate_codegen_timing,
    validate_ir_manifest,
    validate_roadmap,
    validate_steady_state,
)


REVISION = "f7ba1dbb3295084342fc002c7816eadf096adafb"


def timing_fixture(label: str, codegen_ns: float = 100.0) -> dict:
    return {
        "schema": CODEGEN_SCHEMA,
        "task": "R-3104",
        "label": label,
        "profiling_causal_claim": False,
        "git_revision": REVISION,
        "source_tree_fingerprint": f"{label}-tree",
        "profile": "release",
        "scenarios": list(CODEGEN_SCENARIOS),
        "measurement_policy": {"warmup_runs": 3, "timed_runs": 20},
        "results": {
            scenario: {"timings": {"codegen": {"median_ns": codegen_ns}}}
            for scenario in CODEGEN_SCENARIOS
        },
    }


def steady_state_fixture(*, spectra_ns: int = 90, java: bool = False) -> dict:
    results = {}
    for scenario in STEADY_STATE_SCENARIOS:
        languages = {}
        for language, median_ns in (("spectra", spectra_ns), ("go", 100), ("rust", 95)):
            groups = [
                {
                    "warmup_runs": 3,
                    "timed_runs": 20,
                    "exit_code": 0,
                    "failure_class": None,
                    "median_ns": median_ns,
                }
                for _ in range(5)
            ]
            languages[language] = {
                "groups": groups,
                "successful_independent_runs": 5,
                "median_of_group_medians_ns": median_ns,
            }
        results[scenario] = {
            "aot_compile": {"exit_code": 0},
            "languages": languages,
            "ratios": {"spectra_to_go": spectra_ns / 100, "spectra_to_rust": spectra_ns / 95},
            "correctness_passed": True,
        }
    return {
        "schema": "spectra.phase31.r3104_steady_state.v1",
        "task": "R-3104",
        "profiling_causal_claim": False,
        "git_revision": REVISION,
        "source_tree_fingerprint": "steady-tree",
        "profile": "release",
        "binary_sha256": "release-sha",
        "benchmark_languages": ["spectra", "go", "rust"],
        "java_excluded": not java,
        "scenarios": list(STEADY_STATE_SCENARIOS),
        "measurement_policy": {"warmup_runs": 3, "timed_runs": 20, "independent_runs": 5},
        "results": results,
    }


class R3104ValidatorTests(unittest.TestCase):
    def test_codegen_cpu_group_requires_five_percent_gain(self) -> None:
        summary, errors = validate_codegen_timing(
            timing_fixture("before", 100.0), timing_fixture("after", 90.0), expected_revision=REVISION
        )
        self.assertEqual([], errors)
        self.assertAlmostEqual(10.0, summary["cpu_target_geometric_mean_improvement_pct"])

        _, errors = validate_codegen_timing(
            timing_fixture("before", 100.0), timing_fixture("after", 100.0), expected_revision=REVISION
        )
        self.assertTrue(any("geometric mean" in error for error in errors))

    def test_codegen_regression_and_policy_are_rejected(self) -> None:
        after = timing_fixture("after", 100.0)
        after["results"]["cpu-fibs"]["timings"]["codegen"]["median_ns"] = 106.0
        after["measurement_policy"]["timed_runs"] = 19
        _, errors = validate_codegen_timing(
            timing_fixture("before", 100.0), after, expected_revision=REVISION
        )
        self.assertTrue(any("cpu-fibs" in error for error in errors))
        self.assertTrue(any("policy" in error for error in errors))

    def test_codegen_requires_distinct_source_tree_fingerprints(self) -> None:
        before = timing_fixture("before", 100.0)
        after = timing_fixture("after", 90.0)
        after["source_tree_fingerprint"] = before["source_tree_fingerprint"]
        _, errors = validate_codegen_timing(before, after, expected_revision=REVISION)
        self.assertTrue(any("fingerprints are identical" in error for error in errors))

    def test_baseline_regression_above_five_percent_is_rejected(self) -> None:
        report = {
            "scenarios": [
                {"id": "cpu-loop-sum", "results": {"spectra": {"ns_per_iter": 106}}}
            ]
        }
        baseline = {"scenarios": {"cpu-loop-sum": {"spectra_ns_per_iter": 100}}}
        errors = validate_baseline_drift(report, baseline)
        self.assertTrue(any("baseline regression" in error for error in errors))

    def test_steady_state_requires_policy_and_excludes_java(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "spectralang.exe"
            binary.write_bytes(b"release")
            payload = steady_state_fixture()
            payload["binary_sha256"] = __import__("hashlib").sha256(binary.read_bytes()).hexdigest()
            summary, errors = validate_steady_state(
                payload,
                expected_revision=REVISION,
                binary=binary,
                baseline={"scenarios": {scenario: {"spectra_ns_per_iter": 100} for scenario in STEADY_STATE_SCENARIOS}},
            )
            self.assertEqual([], errors)
            self.assertEqual(6, len(summary["scenarios"]))
            payload["measurement_policy"]["timed_runs"] = 19
            payload["java_excluded"] = False
            _, errors = validate_steady_state(
                payload,
                expected_revision=REVISION,
                binary=binary,
                baseline={"scenarios": {}},
            )
            self.assertTrue(any("policy" in error for error in errors))
            self.assertTrue(any("Java" in error for error in errors))

    def test_steady_state_rejects_runtime_regression(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "spectralang.exe"
            binary.write_bytes(b"release")
            payload = steady_state_fixture(spectra_ns=106)
            payload["binary_sha256"] = __import__("hashlib").sha256(binary.read_bytes()).hexdigest()
            _, errors = validate_steady_state(
                payload,
                expected_revision=REVISION,
                binary=binary,
                baseline={"scenarios": {scenario: {"spectra_ns_per_iter": 100} for scenario in STEADY_STATE_SCENARIOS}},
            )
            self.assertTrue(any("baseline regression" in error for error in errors))

    def test_roadmap_allows_active_r3104_but_keeps_followups_closed(self) -> None:
        items = [
            {"id": "R-3102", "status": "in_progress"},
            {"id": "R-3103", "status": "complete"},
            {"id": "R-3104", "status": "in_progress"},
        ] + [{"id": f"R-{number}", "status": "not_started"} for number in range(3105, 3118)]
        self.assertEqual([], validate_roadmap({"items": items}))
        items[-2]["status"] = "in_progress"
        self.assertTrue(any("R-3116" in error for error in validate_roadmap({"items": items})))

    def test_ir_manifest_requires_current_binary_and_no_java(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "target" / "release" / "spectralang.exe"
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b"release")
            ir_root = root / "target" / "phase31" / "r3104-ir"
            files = {}
            for scenario in SCENARIOS:
                scenario_dir = ir_root / scenario
                scenario_dir.mkdir(parents=True)
                files[scenario] = {}
                for level in ("o0", "o3"):
                    path = scenario_dir / f"{level}.txt"
                    path.write_text(f"{scenario}-{level}\n", encoding="utf-8")
                    files[scenario][level] = {
                        "path": f"{scenario}/{level}.txt",
                        "sha256": __import__("hashlib").sha256(path.read_bytes()).hexdigest(),
                        "bytes": path.stat().st_size,
                    }
            for scenario in SNAPSHOT_SCENARIOS:
                for level in ("o0", "o3"):
                    snapshot = root / SNAPSHOT_ROOT / f"{scenario}-{level}.txt"
                    snapshot.parent.mkdir(parents=True, exist_ok=True)
                    generated = ir_root / scenario / f"{level}.txt"
                    snapshot.write_bytes(generated.read_bytes())
            manifest = {
                "schema": IR_MANIFEST_SCHEMA,
                "git_revision": REVISION,
                "profile": "release",
                "binary": "target/release/spectralang.exe",
                "binary_sha256": __import__("hashlib").sha256(binary.read_bytes()).hexdigest(),
                "benchmark_languages": ["spectra", "go", "rust"],
                "java_excluded": True,
                "options": IR_OPTIONS,
                "scenario_count": len(SCENARIOS),
                "scenarios": list(SCENARIOS),
                "files": files,
            }
            ir_root.mkdir(parents=True, exist_ok=True)
            (ir_root / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
            _, errors = validate_ir_manifest(root=root, ir_root=ir_root, binary=binary, expected_revision=REVISION)
            self.assertEqual([], errors)
            manifest["java_excluded"] = False
            (ir_root / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
            _, errors = validate_ir_manifest(root=root, ir_root=ir_root, binary=binary, expected_revision=REVISION)
            self.assertTrue(any("Java" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
