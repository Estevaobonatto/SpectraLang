#!/usr/bin/env python3
"""Validate and publish the R-3105 hostcall batching evidence gate."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import tomllib
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

try:
    from scripts.benchmark_r3105_hostcalls import parse_stats, sha256_file
    from scripts.phase31_contract import LANGUAGES, SCENARIOS, ASYNC_ECHO_ACCEPTED_MAX_GAP_TO_GO
    from scripts.validate_r3103_optimization_plan import (
        semantic_report,
        validate_report as validate_phase31_report,
    )
    from scripts.validate_r3104_codegen_hot_path import validate_steady_state
except ModuleNotFoundError:  # pragma: no cover
    from benchmark_r3105_hostcalls import parse_stats, sha256_file  # type: ignore[no-redef]
    from phase31_contract import (  # type: ignore[no-redef]
        ASYNC_ECHO_ACCEPTED_MAX_GAP_TO_GO,
        LANGUAGES,
        SCENARIOS,
    )
    from validate_r3103_optimization_plan import (  # type: ignore[no-redef]
        semantic_report,
        validate_report as validate_phase31_report,
    )
    from validate_r3104_codegen_hot_path import validate_steady_state  # type: ignore[no-redef]


ROOT = Path(__file__).resolve().parents[1]
EVIDENCE_SCHEMA = "spectra.phase31.r3105_hostcall_batching_evidence.v1"
BENCHMARK_SCHEMA = "spectra.phase31.r3105_hostcall_benchmark.v1"
REPORT_SCHEMA = "spectra.phase31.bench.v1"
BASELINE_SHA256 = "452a2e0e25db99d1175f5cbd1a50ac969512055e70c6ebf1c8c5ef959ca8b30b"
FIXTURE_DEFAULT = ROOT / "tests" / "validation" / "191_phase31_hostcall_batch_contract.spectra"
BINARY_DEFAULT = ROOT / "target" / "release" / "spectralang.exe"
FIXTURE_AOT_DEFAULT = ROOT / "target" / "phase31" / "r3105-hostcall-contract.exe"
REQUIRED_BATCH_STATS = (
    "batched_sites",
    "batched_hostcalls",
    "fallback_hostcalls",
    "argument_arena_bytes",
    "result_arena_bytes",
)


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"JSON object expected: {path}")
    return value


def git_revision(root: Path) -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=root, capture_output=True, text=True, check=False
    )
    if result.returncode != 0 or not result.stdout.strip():
        raise RuntimeError("unable to resolve current Git revision")
    return result.stdout.strip()


def run_command(command: list[str], *, cwd: Path = ROOT) -> tuple[int, str]:
    try:
        result = subprocess.run(
            command,
            cwd=cwd,
            env=os.environ.copy() | {"SPECTRA_R3105_STATS": "1"},
            capture_output=True,
            text=True,
            timeout=300,
            check=False,
        )
    except subprocess.TimeoutExpired as exc:
        return -1, f"timeout: {exc}"
    output = ((result.stdout or "") + "\n" + (result.stderr or "")).strip()
    return result.returncode, output[-4000:]


def validate_roadmap(roadmap: dict[str, Any]) -> list[str]:
    items = {item.get("id"): item for item in roadmap.get("items", [])}
    errors: list[str] = []
    if items.get("R-3102", {}).get("status") != "in_progress":
        errors.append("R-3102 must remain in_progress")
    if items.get("R-3103", {}).get("status") != "complete":
        errors.append("R-3103 must be complete")
    if items.get("R-3104", {}).get("status") != "complete":
        errors.append("R-3104 must remain complete before R-3105 promotion")
    r3105 = items.get("R-3105")
    if not r3105 or r3105.get("status") not in {"in_progress", "complete"}:
        errors.append("R-3105 must be in_progress or complete")
    if r3105 and r3105.get("dependencies") != ["R-3103", "R-3104"]:
        errors.append("R-3105 dependencies must be R-3103 and R-3104")
    # R-3107, R-3108, and R-3117 were already complete historical items;
    # preserve those facts while keeping the active follow-ups closed.
    for number in (3106, 3109, 3110, 3111, 3112, 3113, 3114, 3115, 3116):
        item_id = f"R-{number}"
        if items.get(item_id, {}).get("status") != "not_started":
            errors.append(f"{item_id} must remain not_started")
    return errors


def validate_baseline_sha256(path: Path, expected_sha256: str) -> list[str]:
    if not path.is_file():
        return [f"baseline is missing: {path}"]
    actual = sha256_file(path)
    if expected_sha256 and actual != expected_sha256:
        return ["baseline SHA-256 differs from the immutable R-3105 baseline"]
    return []


def validate_batch_stats(stats: Any, *, label: str) -> list[str]:
    errors: list[str] = []
    if not isinstance(stats, dict):
        return [f"{label}: batch statistics are missing"]
    for name in REQUIRED_BATCH_STATS:
        if not isinstance(stats.get(name), int) or stats.get(name, -1) < 0:
            errors.append(f"{label}: {name} must be a non-negative integer")
    if isinstance(stats.get("batched_sites"), int) and stats["batched_sites"] < 1:
        errors.append(f"{label}: at least one batched site is required")
    if isinstance(stats.get("batched_hostcalls"), int) and stats["batched_hostcalls"] < 2:
        errors.append(f"{label}: at least two hostcalls must be grouped")
    if isinstance(stats.get("argument_arena_bytes"), int) and stats["argument_arena_bytes"] <= 0:
        errors.append(f"{label}: argument arena must be non-empty")
    if isinstance(stats.get("result_arena_bytes"), int) and stats["result_arena_bytes"] <= 0:
        errors.append(f"{label}: result arena must be non-empty")
    return errors


def validate_benchmark(payload: dict[str, Any], *, expected_revision: str) -> list[str]:
    errors: list[str] = []
    if payload.get("schema") != BENCHMARK_SCHEMA:
        errors.append("dedicated benchmark schema is invalid")
    if payload.get("task") != "R-3105":
        errors.append("dedicated benchmark task is invalid")
    if payload.get("classification") != "benchmark_and_ir_hypothesis":
        errors.append("dedicated benchmark classification is invalid")
    if payload.get("profiling_causal_claim") is not False:
        errors.append("dedicated benchmark cannot claim causal profiling")
    if payload.get("git_revision") != expected_revision:
        errors.append("dedicated benchmark Git revision does not match current HEAD")
    if payload.get("profile") != "release":
        errors.append("dedicated benchmark profile must be release")
    if payload.get("benchmark_languages") != ["spectra"]:
        errors.append("dedicated benchmark must contain only Spectra")
    if payload.get("java_excluded") is not True:
        errors.append("dedicated benchmark must explicitly exclude Java")
    policy = payload.get("measurement_policy") if isinstance(payload.get("measurement_policy"), dict) else {}
    if (
        policy.get("warmup_runs") != 3
        or policy.get("timed_runs") != 20
        or policy.get("independent_runs") != 5
        or policy.get("aggregation") != "median_of_group_medians"
        or policy.get("runtime_measurement") != "precompiled_aot_executable"
    ):
        errors.append("dedicated benchmark policy must be 5 groups of 3 warmups and 20 samples")
    if not isinstance(payload.get("source_tree_fingerprint"), str) or not payload.get("source_tree_fingerprint"):
        errors.append("candidate source-tree fingerprint is missing")
    control = payload.get("control") if isinstance(payload.get("control"), dict) else {}
    if not isinstance(control.get("source_tree_fingerprint"), str) or not control.get("source_tree_fingerprint"):
        errors.append("clean control source-tree fingerprint is missing")
    elif control.get("source_tree_fingerprint") == payload.get("source_tree_fingerprint"):
        errors.append("candidate/control source-tree fingerprints are identical")
    if not isinstance(payload.get("binary_sha256"), str) or not payload.get("binary_sha256"):
        errors.append("candidate binary SHA-256 is missing")
    if not isinstance(control.get("binary_sha256"), str) or not control.get("binary_sha256"):
        errors.append("clean control binary SHA-256 is missing")

    for label, compile_result in (
        ("candidate", payload.get("candidate_compile")),
        ("control", payload.get("control_compile")),
    ):
        if not isinstance(compile_result, dict) or compile_result.get("exit_code") != 0:
            errors.append(f"{label} AOT compilation did not pass")
        elif not compile_result.get("output_sha256"):
            errors.append(f"{label} AOT output SHA-256 is missing")
    errors.extend(validate_batch_stats(payload.get("candidate_batch_stats"), label="candidate"))

    for label in ("candidate_runtime", "control_runtime"):
        runtime = payload.get(label) if isinstance(payload.get(label), dict) else {}
        groups = runtime.get("groups") if isinstance(runtime.get("groups"), list) else []
        if len(groups) != 5:
            errors.append(f"{label}: expected five independent groups")
        for group in groups:
            if group.get("warmup_runs") != 3 or group.get("timed_runs") != 20:
                errors.append(f"{label}: invalid warmup/sample policy")
            if group.get("exit_code") != 0 or group.get("failure_class") is not None:
                errors.append(f"{label}: runtime correctness failed")
            timings = group.get("timings") if isinstance(group.get("timings"), dict) else {}
            if not isinstance(timings.get("median_ns"), (int, float)) or timings.get("median_ns", 0) <= 0:
                errors.append(f"{label}: group median is missing")
        if runtime.get("successful_independent_runs") != 5:
            errors.append(f"{label}: not all independent groups passed")
        if not isinstance(runtime.get("median_of_group_medians_ns"), (int, float)) or runtime.get("median_of_group_medians_ns", 0) <= 0:
            errors.append(f"{label}: median of group medians is missing")

    candidate_median = (payload.get("candidate_runtime") or {}).get("median_of_group_medians_ns")
    control_median = (payload.get("control_runtime") or {}).get("median_of_group_medians_ns")
    ratio = payload.get("candidate_to_control_ratio")
    if not isinstance(candidate_median, (int, float)) or not isinstance(control_median, (int, float)) or control_median <= 0:
        errors.append("dedicated benchmark medians are invalid")
    elif not isinstance(ratio, (int, float)) or abs(ratio - candidate_median / control_median) > 1e-9:
        errors.append("candidate/control ratio is missing or inconsistent")
    elif ratio > 0.90:
        errors.append(f"dedicated hostcall speedup is insufficient: {ratio:.6f}x (> 0.90x)")
    if payload.get("speedup_gate_passed") is not True:
        errors.append("dedicated hostcall speedup gate is not marked passed")
    if payload.get("correctness_passed") is not True or payload.get("control_correctness_passed") is not True:
        errors.append("dedicated hostcall benchmark correctness did not pass")
    return errors


def validate_code_report(report: dict[str, Any], *, expected_revision: str) -> list[str]:
    errors: list[str] = []
    if report.get("schema") != REPORT_SCHEMA:
        errors.append("code-validation report schema is invalid")
    if report.get("git_revision") != expected_revision:
        errors.append("code-validation report Git revision does not match current HEAD")
    ids = [item.get("id") for item in report.get("scenarios", [])]
    if ids != list(SCENARIOS):
        errors.append("code-validation report must contain exactly 21 scenarios")
    for item in report.get("scenarios", []):
        scenario_id = item.get("id")
        results = item.get("results") if isinstance(item.get("results"), dict) else {}
        if set(results) != set(LANGUAGES):
            errors.append(f"{scenario_id}: code-validation matrix must be Spectra + Go + Rust only")
        if item.get("correctness_passed") is not True:
            errors.append(f"{scenario_id}: code-validation correctness failed")
        for language, result in results.items():
            if result.get("exit_code") != 0 or result.get("failure_class"):
                errors.append(f"{scenario_id}/{language}: code-validation command failed")
    return errors


def validate_release_reports(
    reports: list[dict[str, Any]], *, root: Path, expected_revision: str, baseline_hash: str, baseline: dict[str, Any]
) -> list[str]:
    errors: list[str] = []
    if len(reports) != 2:
        return ["exactly two release reports are required"]
    for index, report in enumerate(reports, start=1):
        errors.extend(
            f"release-{index}: {error}"
            for error in validate_phase31_report(
                report, root=root, expected_revision=expected_revision, baseline_hash=baseline_hash
            )
        )
        ids = [item.get("id") for item in report.get("scenarios", [])]
        if ids != list(SCENARIOS):
            errors.append(f"release-{index}: report must contain all 21 scenarios")
        for item in report.get("scenarios", []):
            results = item.get("results") if isinstance(item.get("results"), dict) else {}
            if set(results) != set(LANGUAGES):
                errors.append(f"release-{index}/{item.get('id')}: Java or another language is present")
            spectra = results.get("spectra") if isinstance(results.get("spectra"), dict) else {}
            baseline_ns = (baseline.get("scenarios", {}).get(item.get("id"), {}) or {}).get("spectra_ns_per_iter")
            measured = spectra.get("ns_per_iter")
            if isinstance(baseline_ns, (int, float)) and baseline_ns > 0 and isinstance(measured, (int, float)):
                if (measured / baseline_ns - 1.0) * 100.0 > 5.0:
                    errors.append(f"release-{index}/{item.get('id')}: baseline regression exceeds 5%")
    if semantic_report(reports[0]) != semantic_report(reports[1]):
        errors.append("release reports differ semantically")
    return errors


def validate_fixture_results(
    *, binary: Path, source: Path, aot_output: Path
) -> tuple[dict[str, Any], list[str]]:
    errors: list[str] = []
    if not binary.is_file():
        return {}, [f"fixture binary is missing: {binary}"]
    if not source.is_file():
        return {}, [f"fixture source is missing: {source}"]
    jit_command = [str(binary), "run", str(source)]
    jit_exit, jit_output = run_command(jit_command)
    compile_command = [str(binary), "compile", "--emit-exe", str(aot_output), str(source)]
    compile_exit, compile_output = run_command(compile_command)
    aot_exit = -1
    aot_output_tail = ""
    if compile_exit == 0 and aot_output.is_file():
        aot_exit, aot_output_tail = run_command([str(aot_output)])
    if jit_exit != 0:
        errors.append(f"fixture JIT returned {jit_exit}")
    if compile_exit != 0 or not aot_output.is_file():
        errors.append("fixture AOT compilation failed")
    if aot_exit != 0:
        errors.append(f"fixture AOT returned {aot_exit}")
    stats = parse_stats(compile_output, "aot")
    errors.extend(validate_batch_stats(stats, label="fixture AOT"))
    return {
        "source": str(source.relative_to(ROOT)) if source.is_relative_to(ROOT) else str(source),
        "jit": {"command": jit_command, "exit_code": jit_exit, "output_tail": jit_output},
        "aot_compile": {
            "command": compile_command,
            "exit_code": compile_exit,
            "output_tail": compile_output,
            "batch_stats": stats,
        },
        "aot_run": {"command": [str(aot_output)], "exit_code": aot_exit, "output_tail": aot_output_tail},
        "jit_aot_equivalent": jit_exit == 0 and aot_exit == 0,
    }, errors


def build_markdown(evidence: dict[str, Any]) -> str:
    status = evidence.get("status", "blocked").upper()
    benchmark = evidence.get("benchmark", {})
    ratio = benchmark.get("candidate_to_control_ratio")
    lines = [
        "# R-3105 Hostcall Batching Evidence",
        "",
        f"- Status: **{status}**",
        f"- Revision: `{evidence.get('git_revision', '')}`",
        f"- Dedicated candidate/control: `{ratio}` (gate `<= 0.90x`)",
        f"- Functional matrix: `{evidence.get('release_reports_functional', False)}` (Spectra + Go + Rust, 21 scenarios)",
        f"- JIT/AOT fixture: `{evidence.get('fixture', {}).get('jit_aot_equivalent', False)}`",
        f"- Baseline unchanged: `{evidence.get('baseline', {}).get('sha256_before') == evidence.get('baseline', {}).get('sha256_after')}`",
        "",
        "The benchmark is classified as `benchmark_and_ir_hypothesis`; no causal profiling claim is made.",
        "Java is excluded from the official matrix.",
    ]
    errors = evidence.get("errors") or []
    if errors:
        lines.extend(["", "## Gate failures", "", *[f"- {error}" for error in errors]])
    return "\n".join(lines) + "\n"


def build_evidence(
    *,
    root: Path,
    benchmark_path: Path,
    code_validation_path: Path | None,
    report_paths: list[Path],
    steady_state_path: Path | None,
    baseline_path: Path,
    roadmap_path: Path,
    binary: Path,
    fixture_source: Path,
    fixture_aot_output: Path,
    expected_baseline_sha256: str,
) -> tuple[dict[str, Any], list[str]]:
    benchmark_path = benchmark_path.resolve()
    code_validation_path = code_validation_path.resolve() if code_validation_path is not None else None
    report_paths = [path.resolve() for path in report_paths]
    steady_state_path = steady_state_path.resolve() if steady_state_path is not None else None
    baseline_path = baseline_path.resolve()
    roadmap_path = roadmap_path.resolve()
    binary = binary.resolve()
    fixture_source = fixture_source.resolve()
    fixture_aot_output = fixture_aot_output.resolve()
    revision = git_revision(root)
    baseline_before = sha256_file(baseline_path) if baseline_path.is_file() else ""
    errors: list[str] = []
    errors.extend(validate_baseline_sha256(baseline_path, expected_baseline_sha256))
    baseline = load_json(baseline_path)
    benchmark = load_json(benchmark_path)
    errors.extend(validate_benchmark(benchmark, expected_revision=revision))
    code_validation = None
    if code_validation_path is not None:
        code_validation = load_json(code_validation_path)
        errors.extend(validate_code_report(code_validation, expected_revision=revision))
    reports = [load_json(path) for path in report_paths]
    errors.extend(
        validate_release_reports(
            reports, root=root, expected_revision=revision, baseline_hash=baseline_before, baseline=baseline
        )
    )
    steady_summary: dict[str, Any] = {}
    if steady_state_path is None or not steady_state_path.is_file():
        errors.append("R-3104 six-scenario AOT steady-state evidence is missing")
    else:
        steady = load_json(steady_state_path)
        steady_summary, steady_errors = validate_steady_state(
            steady, expected_revision=revision, binary=binary, baseline=baseline
        )
        errors.extend(f"steady-state: {error}" for error in steady_errors)
    fixture, fixture_errors = validate_fixture_results(
        binary=binary, source=fixture_source, aot_output=fixture_aot_output
    )
    errors.extend(f"fixture: {error}" for error in fixture_errors)
    roadmap = tomllib.loads(roadmap_path.read_text(encoding="utf-8"))
    errors.extend(validate_roadmap(roadmap))
    baseline_after = sha256_file(baseline_path) if baseline_path.is_file() else ""
    if baseline_before != baseline_after:
        errors.append("baseline changed while R-3105 validation was running")

    evidence = {
        "schema": EVIDENCE_SCHEMA,
        "status": "passed" if not errors else "blocked",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "task": "R-3105",
        "git_revision": revision,
        "profile": "release",
        "classification": "benchmark_and_ir_hypothesis",
        "profiling_causal_claim": False,
        "benchmark": {
            "path": str(benchmark_path.relative_to(root)),
            "sha256": sha256_file(benchmark_path),
            "candidate_to_control_ratio": benchmark.get("candidate_to_control_ratio"),
            "candidate_batch_stats": benchmark.get("candidate_batch_stats"),
            "candidate_binary_sha256": benchmark.get("binary_sha256"),
            "control_binary_sha256": (benchmark.get("control") or {}).get("binary_sha256"),
        },
        "code_validation": (
            {"path": str(code_validation_path.relative_to(root)), "sha256": sha256_file(code_validation_path)}
            if code_validation_path is not None and code_validation_path.is_file()
            else None
        ),
        "release_reports": [
            {"path": str(path.relative_to(root)), "sha256": sha256_file(path)}
            for path in report_paths
            if path.is_file()
        ],
        "release_reports_semantically_compatible": len(reports) == 2
        and semantic_report(reports[0]) == semantic_report(reports[1]),
        "release_reports_functional": not any(error.startswith("release-") for error in errors),
        "steady_state": steady_summary,
        "fixture": fixture,
        "baseline": {
            "path": str(baseline_path.relative_to(root)),
            "sha256_before": baseline_before,
            "sha256_after": baseline_after,
            "expected_sha256": expected_baseline_sha256,
            "modified": baseline_before != baseline_after,
        },
        "matrix": {"scenarios": list(SCENARIOS), "languages": list(LANGUAGES), "java_excluded": True},
        "async_echo_gate": {"max_gap_to_go": ASYNC_ECHO_ACCEPTED_MAX_GAP_TO_GO},
        "errors": errors,
    }
    return evidence, errors


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--benchmark", type=Path, required=True)
    parser.add_argument("--code-validation", type=Path)
    parser.add_argument("--report", type=Path, action="append", required=True)
    parser.add_argument("--steady-state", type=Path, default=Path("target/phase31/r3104-steady-state.json"))
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--roadmap", type=Path, required=True)
    parser.add_argument("--binary", type=Path, default=BINARY_DEFAULT)
    parser.add_argument("--fixture-source", type=Path, default=FIXTURE_DEFAULT)
    parser.add_argument("--fixture-aot-output", type=Path, default=FIXTURE_AOT_DEFAULT)
    parser.add_argument("--expected-baseline-sha256", default=BASELINE_SHA256)
    parser.add_argument("--evidence", type=Path, default=Path("docs/performance/phase31-go-comparable/evidence-r3105-hostcall-batching.json"))
    parser.add_argument("--evidence-md", type=Path, default=Path("docs/performance/phase31-go-comparable/evidence-r3105-hostcall-batching.md"))
    parser.add_argument("--write-evidence", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        evidence, errors = build_evidence(
            root=ROOT,
            benchmark_path=args.benchmark,
            code_validation_path=args.code_validation,
            report_paths=args.report,
            steady_state_path=args.steady_state,
            baseline_path=args.baseline,
            roadmap_path=args.roadmap,
            binary=args.binary,
            fixture_source=args.fixture_source,
            fixture_aot_output=args.fixture_aot_output,
            expected_baseline_sha256=args.expected_baseline_sha256,
        )
        if args.write_evidence:
            args.evidence.parent.mkdir(parents=True, exist_ok=True)
            args.evidence.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8", newline="\n")
            args.evidence_md.parent.mkdir(parents=True, exist_ok=True)
            args.evidence_md.write_text(build_markdown(evidence), encoding="utf-8", newline="\n")
    except (OSError, ValueError, RuntimeError, tomllib.TOMLDecodeError) as exc:
        print(f"R-3105 validator: BLOCKED: {exc}", file=__import__("sys").stderr)
        return 1
    if errors:
        print("R-3105 validator: BLOCKED", file=__import__("sys").stderr)
        for error in errors:
            print(f"- {error}", file=__import__("sys").stderr)
        return 1
    print("R-3105 validator: PASSED")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
