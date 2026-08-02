#!/usr/bin/env python3
"""Validate and publish the R-3104 dense-value-map evidence gate."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import subprocess
import tomllib
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

try:
    from scripts.phase31_contract import LANGUAGES, SCENARIOS
    from scripts.validate_r3103_optimization_plan import (
        REPORT_SCHEMA,
        semantic_report,
        validate_report as validate_phase31_report,
    )
except ModuleNotFoundError:  # pragma: no cover
    from phase31_contract import LANGUAGES, SCENARIOS  # type: ignore[no-redef]
    from validate_r3103_optimization_plan import (  # type: ignore[no-redef]
        REPORT_SCHEMA,
        semantic_report,
        validate_report as validate_phase31_report,
    )


ROOT = Path(__file__).resolve().parents[1]
EVIDENCE_SCHEMA = "spectra.phase31.r3104_codegen_evidence.v1"
IR_MANIFEST_SCHEMA = "spectra.phase31.r3104_ir_manifest.v1"
CODEGEN_SCHEMA = "spectra.phase31.r3104_codegen_timing.v1"
STEADY_STATE_SCHEMA = "spectra.phase31.r3104_steady_state.v1"
STEADY_STATE_MAX_SPECTRA_TO_GO = 1.25
STEADY_STATE_MAX_CANDIDATE_REGRESSION = 1.05
CODEGEN_SCENARIOS = (
    "cpu-loop-sum",
    "cpu-fibs",
    "cpu-hashmap",
    "tensor-create",
    "tensor-elementwise",
    "tensor-matmul",
)
CPU_TARGET_SCENARIOS = ("cpu-loop-sum", "cpu-fibs", "cpu-hashmap")
STEADY_STATE_SCENARIOS = (
    "cpu-loop-sum",
    "cpu-fibs",
    "cpu-hashmap",
    "tensor-create",
    "tensor-elementwise",
    "tensor-matmul",
)
SNAPSHOT_SCENARIOS = ("cpu-string-build", "tensor-create", "cpu-hashmap", "tensor-matmul", "ml-mlp-step")
SNAPSHOT_ROOT = Path("docs/performance/phase31-go-comparable/ir/r3103")
IR_OPTIONS = {"o0": ["compile", "--dump-ir", "-O0"], "o3": ["compile", "--dump-ir", "-O3"]}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"JSON object expected: {path}")
    return value


def git_revision(root: Path) -> str:
    result = subprocess.run(["git", "rev-parse", "HEAD"], cwd=root, capture_output=True, text=True, check=False)
    if result.returncode != 0 or not result.stdout.strip():
        raise RuntimeError("unable to resolve current Git revision")
    return result.stdout.strip()


def validate_roadmap(roadmap: dict[str, Any]) -> list[str]:
    items = {item.get("id"): item for item in roadmap.get("items", [])}
    errors: list[str] = []
    if items.get("R-3102", {}).get("status") != "in_progress":
        errors.append("R-3102 must remain in_progress")
    if items.get("R-3103", {}).get("status") != "complete":
        errors.append("R-3103 must be complete before R-3104 promotion")
    if items.get("R-3104", {}).get("status") not in {"in_progress", "complete"}:
        errors.append("R-3104 must be in_progress or complete")
    # R-3107, R-3108, and R-3117 were already complete before R-3104 started; do not
    # regress those existing roadmap facts while keeping the active follow-ups
    # closed.
    for number in (3105, 3106, 3109, 3110, 3111, 3112, 3113, 3114, 3115, 3116):
        item_id = f"R-{number}"
        if items.get(item_id, {}).get("status") != "not_started":
            errors.append(f"{item_id} must remain not_started")
    return errors


def validate_baseline_drift(report: dict[str, Any], baseline: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    baseline_scenarios = baseline.get("scenarios") if isinstance(baseline.get("scenarios"), dict) else {}
    for item in report.get("scenarios", []):
        scenario_id = item.get("id")
        expected = baseline_scenarios.get(scenario_id, {})
        baseline_ns = expected.get("spectra_ns_per_iter")
        measured = item.get("results", {}).get("spectra", {}).get("ns_per_iter")
        if not isinstance(baseline_ns, (int, float)) or baseline_ns <= 0:
            continue
        if not isinstance(measured, (int, float)) or measured <= 0:
            errors.append(f"{scenario_id}: missing Spectra baseline comparison")
            continue
        regression_pct = (measured / baseline_ns - 1.0) * 100.0
        if regression_pct > 5.0:
            errors.append(f"{scenario_id}: baseline regression is {regression_pct:.3f}% (> 5%)")
    return errors


def validate_codegen_timing(
    before: dict[str, Any], after: dict[str, Any], *, expected_revision: str
) -> tuple[dict[str, Any], list[str]]:
    errors: list[str] = []
    for label, payload in (("before", before), ("after", after)):
        if payload.get("schema") != CODEGEN_SCHEMA:
            errors.append(f"codegen {label}: schema is invalid")
        if payload.get("label") != label:
            errors.append(f"codegen {label}: label is invalid")
        if payload.get("task") != "R-3104":
            errors.append(f"codegen {label}: task is invalid")
        if payload.get("git_revision") != expected_revision:
            errors.append(f"codegen {label}: Git revision does not match current HEAD")
        if not isinstance(payload.get("source_tree_fingerprint"), str) or not payload.get("source_tree_fingerprint"):
            errors.append(f"codegen {label}: source-tree fingerprint is missing")
        if payload.get("profiling_causal_claim") is not False:
            errors.append(f"codegen {label}: causal profiling claim is not false")
        if payload.get("profile") != "release":
            errors.append(f"codegen {label}: profile must be release")
        policy = payload.get("measurement_policy") or {}
        if (
            policy.get("warmup_runs") != 3
            or policy.get("timed_runs") != 20
            or policy.get("independent_runs") != 5
            or policy.get("aggregation") != "median_of_group_medians"
        ):
            errors.append(f"codegen {label}: policy must use 3 warmups, 20 samples, and 5 independent groups")
        if payload.get("scenarios") != list(CODEGEN_SCENARIOS):
            errors.append(f"codegen {label}: scenario set must be the six controlled scenarios")
        groups = payload.get("groups") if isinstance(payload.get("groups"), list) else []
        if len(groups) != 5:
            errors.append(f"codegen {label}: expected 5 independent groups")
        for group in groups:
            if not isinstance(group, dict) or not isinstance(group.get("results"), dict):
                errors.append(f"codegen {label}: independent group is malformed")
                continue
            if set(group["results"]) != set(CODEGEN_SCENARIOS):
                errors.append(f"codegen {label}: independent group scenario coverage is incomplete")
            for scenario in CODEGEN_SCENARIOS:
                item = group["results"].get(scenario, {})
                if item.get("warmup_runs") != 3 or item.get("timed_runs") != 20:
                    errors.append(f"codegen {label}/{scenario}: invalid independent group policy")
    before_fingerprint = before.get("source_tree_fingerprint")
    after_fingerprint = after.get("source_tree_fingerprint")
    if isinstance(before_fingerprint, str) and isinstance(after_fingerprint, str) and before_fingerprint == after_fingerprint:
        errors.append("codegen before/after source-tree fingerprints are identical")
    before_results = before.get("results") if isinstance(before.get("results"), dict) else {}
    after_results = after.get("results") if isinstance(after.get("results"), dict) else {}
    ratios: dict[str, float] = {}
    for scenario in CODEGEN_SCENARIOS:
        before_timing = before_results.get(scenario, {}).get("timings", {}).get("codegen", {})
        after_timing = after_results.get(scenario, {}).get("timings", {}).get("codegen", {})
        before_ns = before_timing.get("median_ns")
        after_ns = after_timing.get("median_ns")
        if not isinstance(before_ns, (int, float)) or not isinstance(after_ns, (int, float)) or before_ns <= 0:
            errors.append(f"codegen {scenario}: missing positive median")
            continue
        ratio = after_ns / before_ns
        ratios[scenario] = ratio
        if ratio > 1.05:
            errors.append(f"codegen {scenario}: regression is {(ratio - 1.0) * 100:.3f}% (> 5%)")
    target_ratios = [ratios[scenario] for scenario in CPU_TARGET_SCENARIOS if scenario in ratios]
    geometric_mean = math.exp(sum(math.log(ratio) for ratio in target_ratios) / len(target_ratios)) if target_ratios else None
    if geometric_mean is None:
        errors.append("codegen CPU target group is incomplete")
    return {
        "scenarios": ratios,
        "cpu_target_geometric_mean_ratio": geometric_mean,
        "cpu_target_geometric_mean_improvement_pct": (1.0 - geometric_mean) * 100.0 if geometric_mean is not None else None,
    }, errors


def validate_steady_state(
    payload: dict[str, Any], *, expected_revision: str, binary: Path, baseline: dict[str, Any]
) -> tuple[dict[str, Any], list[str]]:
    """Validate runtime-only evidence produced from precompiled executables."""
    errors: list[str] = []
    if payload.get("schema") != STEADY_STATE_SCHEMA:
        errors.append("steady-state schema is invalid")
    if payload.get("task") != "R-3104":
        errors.append("steady-state task is invalid")
    if payload.get("git_revision") != expected_revision:
        errors.append("steady-state Git revision does not match current HEAD")
    if not isinstance(payload.get("source_tree_fingerprint"), str) or not payload.get("source_tree_fingerprint"):
        errors.append("steady-state source-tree fingerprint is missing")
    if payload.get("profile") != "release":
        errors.append("steady-state profile must be release")
    if payload.get("benchmark_languages") != ["spectra", "go", "rust"]:
        errors.append("steady-state matrix must contain only Spectra, Go, and Rust")
    if payload.get("java_excluded") is not True:
        errors.append("steady-state must explicitly exclude Java")
    if payload.get("binary_sha256") != sha256_file(binary) if binary.is_file() else True:
        errors.append("steady-state binary SHA-256 does not match release binary")
    control = payload.get("control") if isinstance(payload.get("control"), dict) else {}
    if not isinstance(control.get("binary_sha256"), str) or not control.get("binary_sha256"):
        errors.append("steady-state clean control binary SHA-256 is missing")
    if not isinstance(control.get("source_tree_fingerprint"), str) or not control.get("source_tree_fingerprint"):
        errors.append("steady-state clean control source-tree fingerprint is missing")
    elif control.get("source_tree_fingerprint") == payload.get("source_tree_fingerprint"):
        errors.append("steady-state control/candidate source-tree fingerprints are identical")
    if payload.get("scenarios") != list(STEADY_STATE_SCENARIOS):
        errors.append("steady-state scenario set must be the six controlled scenarios")
    policy = payload.get("measurement_policy") if isinstance(payload.get("measurement_policy"), dict) else {}
    if policy.get("warmup_runs") != 3 or policy.get("timed_runs") != 20 or policy.get("independent_runs") != 5:
        errors.append("steady-state policy must use 3 warmups, 20 samples, and 5 independent runs")
    results = payload.get("results") if isinstance(payload.get("results"), dict) else {}
    summary: dict[str, Any] = {"scenarios": {}, "runtime_regressions": {}}
    baseline_scenarios = baseline.get("scenarios") if isinstance(baseline.get("scenarios"), dict) else {}

    def validate_language(
        language_result: dict[str, Any], scenario: str, language: str
    ) -> int | None:
        groups = language_result.get("groups") if isinstance(language_result.get("groups"), list) else []
        if len(groups) != 5:
            errors.append(f"steady-state {scenario}/{language}: expected 5 independent groups")
        successful = 0
        for group in groups:
            if group.get("warmup_runs") != 3 or group.get("timed_runs") != 20:
                errors.append(f"steady-state {scenario}/{language}: invalid warmup/sample policy")
            if group.get("exit_code") != 0 or group.get("failure_class") is not None:
                errors.append(f"steady-state {scenario}/{language}: failed runtime sample group")
            if group.get("exit_code") == 0:
                successful += 1
        if successful != 5:
            errors.append(f"steady-state {scenario}/{language}: not all independent groups passed")
        median_ns = language_result.get("median_of_group_medians_ns")
        if not isinstance(median_ns, (int, float)) or median_ns <= 0:
            errors.append(f"steady-state {scenario}/{language}: missing positive runtime median")
            return None
        return int(median_ns)

    for scenario in STEADY_STATE_SCENARIOS:
        item = results.get(scenario) if isinstance(results.get(scenario), dict) else {}
        compile_result = item.get("aot_compile") if isinstance(item.get("aot_compile"), dict) else {}
        if compile_result.get("exit_code") != 0:
            errors.append(f"steady-state {scenario}: AOT compile did not pass")
        control_compile = item.get("control_aot_compile") if isinstance(item.get("control_aot_compile"), dict) else {}
        if control_compile.get("exit_code") != 0:
            errors.append(f"steady-state {scenario}: clean control AOT compile did not pass")
        if item.get("correctness_passed") is not True:
            errors.append(f"steady-state {scenario}: functional execution did not pass")
        if item.get("control_correctness_passed") is not True:
            errors.append(f"steady-state {scenario}: clean control functional execution did not pass")
        languages = item.get("languages") if isinstance(item.get("languages"), dict) else {}
        if set(languages) != {"spectra", "go", "rust"}:
            errors.append(f"steady-state {scenario}: language coverage is incomplete")
            continue
        control_languages = item.get("control_languages") if isinstance(item.get("control_languages"), dict) else {}
        if set(control_languages) != {"spectra"}:
            errors.append(f"steady-state {scenario}: clean control coverage is incomplete")
            continue
        medians: dict[str, int] = {}
        for language in ("spectra", "go", "rust"):
            median_ns = validate_language(languages.get(language, {}), scenario, language)
            if median_ns is not None:
                medians[language] = median_ns
        control_spectra = validate_language(control_languages.get("spectra", {}), scenario, "control/spectra")
        ratios = item.get("ratios") if isinstance(item.get("ratios"), dict) else {}
        spectra_to_go = ratios.get("spectra_to_go")
        spectra_to_rust = ratios.get("spectra_to_rust")
        candidate_to_control = ratios.get("candidate_spectra_to_control_spectra")
        if not isinstance(spectra_to_go, (int, float)) or not isinstance(spectra_to_rust, (int, float)):
            errors.append(f"steady-state {scenario}: Go/Rust ratios are missing")
        elif medians.get("go") and medians.get("rust"):
            if not math.isclose(spectra_to_go, medians["spectra"] / medians["go"], rel_tol=1e-6):
                errors.append(f"steady-state {scenario}: Spectra/Go ratio is inconsistent")
            if not math.isclose(spectra_to_rust, medians["spectra"] / medians["rust"], rel_tol=1e-6):
                errors.append(f"steady-state {scenario}: Spectra/Rust ratio is inconsistent")
        if not isinstance(spectra_to_go, (int, float)) or spectra_to_go > STEADY_STATE_MAX_SPECTRA_TO_GO:
            errors.append(
                f"steady-state {scenario}: Spectra/Go ratio is {spectra_to_go!r} (> {STEADY_STATE_MAX_SPECTRA_TO_GO:.2f}x)"
            )
        if (
            not isinstance(candidate_to_control, (int, float))
            or control_spectra is None
            or medians.get("spectra") is None
            or not math.isclose(candidate_to_control, medians["spectra"] / control_spectra, rel_tol=1e-6)
        ):
            errors.append(f"steady-state {scenario}: candidate/control ratio is missing or inconsistent")
        elif candidate_to_control > STEADY_STATE_MAX_CANDIDATE_REGRESSION:
            errors.append(
                f"steady-state {scenario}: candidate/control runtime regression is {(candidate_to_control - 1.0) * 100:.3f}% (> 5%)"
            )
        baseline_ns = baseline_scenarios.get(scenario, {}).get("spectra_ns_per_iter")
        spectra_ns = medians.get("spectra")
        regression_pct = None
        if isinstance(baseline_ns, (int, float)) and baseline_ns > 0 and spectra_ns:
            regression_pct = (spectra_ns / baseline_ns - 1.0) * 100.0
            if regression_pct > 5.0:
                errors.append(f"steady-state {scenario}: baseline regression is {regression_pct:.3f}% (> 5%)")
            summary["runtime_regressions"][scenario] = regression_pct
        summary["scenarios"][scenario] = {
            "median_ns": spectra_ns,
            "spectra_to_go": spectra_to_go,
            "spectra_to_rust": spectra_to_rust,
            "candidate_to_control": candidate_to_control,
            "control_median_ns": control_spectra,
            "baseline_regression_pct": regression_pct,
        }
    return summary, errors


def validate_ir_manifest(*, root: Path, ir_root: Path, binary: Path, expected_revision: str) -> tuple[dict[str, Any], list[str]]:
    errors: list[str] = []
    manifest_path = ir_root / "manifest.json"
    if not manifest_path.is_file():
        return {}, ["R-3104 IR manifest.json is missing"]
    try:
        manifest = load_json(manifest_path)
    except (OSError, ValueError) as exc:
        return {}, [f"cannot read R-3104 IR manifest: {exc}"]
    if manifest.get("schema") != IR_MANIFEST_SCHEMA:
        errors.append(f"IR manifest schema must be {IR_MANIFEST_SCHEMA}")
    if manifest.get("git_revision") != expected_revision:
        errors.append("IR manifest Git revision does not match current HEAD")
    if manifest.get("profile") != "release":
        errors.append("IR manifest profile must be release")
    if manifest.get("benchmark_languages") != ["spectra", "go", "rust"]:
        errors.append("IR manifest must record only Spectra, Go, and Rust")
    if manifest.get("java_excluded") is not True:
        errors.append("IR manifest must explicitly exclude Java")
    if manifest.get("options") != IR_OPTIONS:
        errors.append("IR manifest options do not match O0/O3")
    if manifest.get("scenario_count") != len(SCENARIOS) or manifest.get("scenarios") != list(SCENARIOS):
        errors.append("IR manifest must contain the canonical 21 scenarios")
    expected_binary = "target/release/spectralang.exe"
    if str(manifest.get("binary", "")).replace("\\", "/") != expected_binary:
        errors.append("IR manifest must reference target/release/spectralang.exe")
    if not binary.is_file():
        errors.append("release binary is missing for IR validation")
    elif manifest.get("binary_sha256") != sha256_file(binary):
        errors.append("IR manifest binary SHA-256 does not match release binary")
    files = manifest.get("files") if isinstance(manifest.get("files"), dict) else {}
    if set(files) != set(SCENARIOS):
        errors.append("IR manifest file coverage is not exactly 21 scenarios")
    for scenario in SCENARIOS:
        for level in ("o0", "o3"):
            entry = files.get(scenario, {}).get(level, {}) if isinstance(files.get(scenario), dict) else {}
            relative = str(entry.get("path", "")).replace("\\", "/")
            expected_relative = f"{scenario}/{level}.txt"
            path = ir_root / Path(relative)
            if relative != expected_relative or not path.is_file() or path.stat().st_size == 0:
                errors.append(f"{scenario}: missing or invalid IR {level}")
                continue
            if entry.get("bytes") != path.stat().st_size or entry.get("sha256") != sha256_file(path):
                errors.append(f"{scenario}: stale IR {level} hash or size")
    snapshots: dict[str, Any] = {}
    for scenario in SNAPSHOT_SCENARIOS:
        snapshot_entry: dict[str, Any] = {}
        for level in ("o0", "o3"):
            snapshot = root / SNAPSHOT_ROOT / f"{scenario}-{level}.txt"
            generated = ir_root / scenario / f"{level}.txt"
            if not snapshot.is_file():
                errors.append(f"R-3103 snapshot is missing: {snapshot}")
                continue
            snapshot_hash = sha256_file(snapshot)
            generated_hash = sha256_file(generated) if generated.is_file() else ""
            snapshot_entry[level] = {"sha256": snapshot_hash, "generated_sha256": generated_hash, "matches": snapshot_hash == generated_hash}
        snapshots[scenario] = snapshot_entry
    manifest["r3103_snapshot_comparison"] = snapshots
    return manifest, errors


def run_jit_aot_smoke(*, binary: Path, source: Path, output: Path, root: Path) -> tuple[dict[str, Any], list[str]]:
    errors: list[str] = []
    output.parent.mkdir(parents=True, exist_ok=True)
    result: dict[str, Any] = {"source": source.relative_to(root).as_posix(), "jit": {}, "aot": {}}
    jit = subprocess.run([str(binary), "compile", "--timings", str(source)], cwd=root, capture_output=True, text=True, timeout=300, check=False)
    result["jit"] = {"exit_code": jit.returncode}
    if jit.returncode != 0:
        errors.append(f"JIT compile smoke failed: {jit.stderr[-1000:]}")
    aot = subprocess.run([str(binary), "compile", "--emit-object", str(output), str(source)], cwd=root, capture_output=True, text=True, timeout=300, check=False)
    result["aot"] = {"exit_code": aot.returncode, "output": output.relative_to(root).as_posix()}
    if aot.returncode != 0 or not output.is_file() or output.stat().st_size == 0:
        errors.append(f"AOT compile smoke failed: {aot.stderr[-1000:]}")
    return result, errors


def build_evidence(
    *, root: Path, report_paths: list[Path], baseline_path: Path, ir_root: Path, codegen_before_path: Path,
    codegen_after_path: Path, steady_state_path: Path, roadmap_path: Path, plan_path: Path, binary: Path | None = None,
    aot_source: Path | None = None, aot_output: Path | None = None,
) -> tuple[dict[str, Any], str, list[str]]:
    report_paths = [path.resolve() for path in report_paths]
    baseline_path = baseline_path.resolve()
    ir_root = ir_root.resolve()
    codegen_before_path = codegen_before_path.resolve()
    codegen_after_path = codegen_after_path.resolve()
    steady_state_path = steady_state_path.resolve()
    roadmap_path = roadmap_path.resolve()
    plan_path = plan_path.resolve()
    if binary is not None:
        binary = binary.resolve()
    if aot_source is not None:
        aot_source = aot_source.resolve()
    if aot_output is not None:
        aot_output = aot_output.resolve()
    errors: list[str] = []
    revision = git_revision(root)
    baseline_before = sha256_file(baseline_path) if baseline_path.is_file() else ""
    if not baseline_before:
        errors.append("baseline is missing")
    reports: list[dict[str, Any]] = []
    if len(report_paths) != 2 or len({path.resolve() for path in report_paths}) != 2:
        errors.append("exactly two distinct release reports are required")
    for path in report_paths:
        try:
            reports.append(load_json(path))
        except (OSError, ValueError) as exc:
            errors.append(f"cannot read report {path}: {exc}")
    baseline = load_json(baseline_path) if baseline_path.is_file() else {}
    for report in reports:
        errors.extend(validate_phase31_report(report, root=root, expected_revision=revision, baseline_hash=baseline_before))
        errors.extend(validate_baseline_drift(report, baseline))
    if len(reports) == 2 and semantic_report(reports[0]) != semantic_report(reports[1]):
        errors.append("the two release reports differ semantically")
    timing: dict[str, Any] = {}
    try:
        timing_before = load_json(codegen_before_path)
        timing_after = load_json(codegen_after_path)
        timing, timing_errors = validate_codegen_timing(timing_before, timing_after, expected_revision=revision)
        errors.extend(timing_errors)
    except (OSError, ValueError) as exc:
        errors.append(f"cannot read codegen timing control: {exc}")
    binary = (binary or root / "target/release/spectralang.exe").resolve()
    steady_state: dict[str, Any] = {}
    try:
        steady_state_payload = load_json(steady_state_path)
        steady_state, steady_errors = validate_steady_state(
            steady_state_payload,
            expected_revision=revision,
            binary=binary,
            baseline=baseline,
        )
        errors.extend(steady_errors)
    except (OSError, ValueError) as exc:
        errors.append(f"cannot read steady-state runtime evidence: {exc}")
    manifest, ir_errors = validate_ir_manifest(root=root, ir_root=ir_root, binary=binary, expected_revision=revision)
    errors.extend(ir_errors)
    jit_aot: dict[str, Any] = {"required": True, "status": "not_run"}
    if aot_source is not None and aot_output is not None:
        jit_aot, smoke_errors = run_jit_aot_smoke(binary=binary, source=aot_source.resolve(), output=aot_output.resolve(), root=root)
        errors.extend(smoke_errors)
    else:
        errors.append("JIT/AOT smoke proof is required")
    try:
        roadmap = tomllib.loads(roadmap_path.read_text(encoding="utf-8"))
        errors.extend(validate_roadmap(roadmap))
    except (OSError, tomllib.TOMLDecodeError) as exc:
        errors.append(f"cannot validate roadmap: {exc}")
    try:
        plan_text = plan_path.read_text(encoding="utf-8")
        if "R-3104" not in plan_text or "benchmark_and_ir_hypothesis" not in plan_text or "rollback" not in plan_text.lower():
            errors.append("optimization plan is missing R-3104 evidence, classification, or rollback")
    except OSError as exc:
        errors.append(f"cannot read optimization plan: {exc}")
    baseline_after = sha256_file(baseline_path) if baseline_path.is_file() else ""
    if not baseline_before or baseline_before != baseline_after:
        errors.append("baseline changed while the R-3104 validator was running")
    evidence = {
        "schema": EVIDENCE_SCHEMA,
        "status": "passed" if not errors else "blocked",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "task": "R-3104",
        "revision": revision,
        "profile": "release",
        "binary": "target/release/spectralang.exe",
        "binary_sha256": sha256_file(binary) if binary.is_file() else "",
        "classification": "benchmark_and_ir_hypothesis",
        "profiling_causal_claim": False,
        "benchmark_languages": list(LANGUAGES),
        "java_excluded": True,
        "reports": [{"path": str(path.relative_to(root)), "sha256": sha256_file(path)} for path in report_paths if path.is_file()],
        "codegen_timing": timing,
        "steady_state": steady_state,
        "ir": {"root": str(ir_root.relative_to(root)), "manifest": manifest},
        "jit_aot": jit_aot,
        "baseline": {"path": str(baseline_path.relative_to(root)), "sha256_before": baseline_before, "sha256_after": baseline_after},
        "baseline_modified": baseline_before != baseline_after,
        "scenario_count": len(SCENARIOS),
        "failures": errors,
    }
    return evidence, render_markdown(evidence), errors


def render_markdown(evidence: dict[str, Any]) -> str:
    timing = evidence.get("codegen_timing", {})
    steady = evidence.get("steady_state", {})
    lines = [
        "# R-3104 Codegen Hot Path Evidence", "", f"- Status: `{evidence['status']}`",
        f"- Revision: `{evidence['revision']}`", f"- Profile: `{evidence['profile']}`",
        f"- Classification: `{evidence['classification']}`", f"- Profiling causal claim: `{evidence['profiling_causal_claim']}`",
        f"- Matrix: `Spectra + Go + Rust` (Java excluded)", f"- Scenarios: `{evidence['scenario_count']}/21`",
        f"- Baseline modified: `{evidence['baseline_modified']}`", "",
        "The evidence is benchmark/IR based; causal Linux profiling remains R-3102.", "",
        "## Controlled codegen comparison", "",
        f"- CPU target geometric-mean ratio after/before: `{timing.get('cpu_target_geometric_mean_ratio')}`",
        f"- CPU target improvement: `{timing.get('cpu_target_geometric_mean_improvement_pct')}%`", "",
        "## Runtime steady state", "",
        "- Measurement: precompiled Spectra AOT executable, excluding JIT/process startup.",
        f"- Scenarios: `{len(steady.get('scenarios', {}))}/6`", "",
        "| Scenario | Spectra/Go | Spectra/Rust | Baseline drift |", "|---|---:|---:|---:|",
    ]
    for scenario, result in steady.get("scenarios", {}).items():
        lines.append(
            f"| `{scenario}` | `{result.get('spectra_to_go')}` | `{result.get('spectra_to_rust')}` | "
            f"`{result.get('baseline_regression_pct')}%` |"
        )
    lines.extend(["", "## Reports", "", "| Artifact | SHA-256 |", "|---|---|"])
    for report in evidence.get("reports", []):
        lines.append(f"| `{report['path']}` | `{report['sha256']}` |")
    lines.extend(["", "## Failures", ""])
    failures = evidence.get("failures", [])
    lines.extend(f"- {failure}" for failure in failures) if failures else lines.append("- none")
    return "\n".join(lines) + "\n"


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", action="append", required=True)
    parser.add_argument("--baseline", type=Path, default=Path("docs/performance/phase31-go-comparable/baseline.json"))
    parser.add_argument("--ir-root", type=Path, default=Path("target/phase31/r3104-ir"))
    parser.add_argument("--codegen-before", type=Path, default=Path("target/phase31/r3104-codegen-before.json"))
    parser.add_argument("--codegen-after", type=Path, default=Path("target/phase31/r3104-codegen-after.json"))
    parser.add_argument("--steady-state", type=Path, default=Path("target/phase31/r3104-steady-state.json"))
    parser.add_argument("--roadmap", type=Path, default=Path("roadmap/roadmap.toml"))
    parser.add_argument("--plan", type=Path, default=Path("docs/performance/phase31-go-comparable/optimization-plan.md"))
    parser.add_argument("--binary", type=Path, default=Path("target/release/spectralang.exe"))
    parser.add_argument("--aot-source", type=Path)
    parser.add_argument("--aot-output", type=Path)
    parser.add_argument("--evidence-json", type=Path, default=Path("docs/performance/phase31-go-comparable/evidence-r3104-codegen.json"))
    parser.add_argument("--evidence-md", type=Path, default=Path("docs/performance/phase31-go-comparable/evidence-r3104-codegen.md"))
    parser.add_argument("--write-evidence", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    evidence, markdown, errors = build_evidence(
        root=ROOT,
        report_paths=[Path(path) for path in args.report],
        baseline_path=args.baseline,
        ir_root=args.ir_root,
        codegen_before_path=args.codegen_before,
        codegen_after_path=args.codegen_after,
        steady_state_path=args.steady_state,
        roadmap_path=args.roadmap,
        plan_path=args.plan,
        binary=args.binary,
        aot_source=args.aot_source,
        aot_output=args.aot_output,
    )
    if args.write_evidence:
        args.evidence_json.parent.mkdir(parents=True, exist_ok=True)
        args.evidence_json.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8", newline="\n")
        args.evidence_md.write_text(markdown, encoding="utf-8", newline="\n")
    print(f"R-3104 validation: {evidence['status']}")
    for error in errors:
        print(f"- {error}")
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
