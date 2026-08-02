#!/usr/bin/env python3
"""Validate and publish the benchmark/IR evidence for R-3103.

This gate deliberately does not claim profiler attribution.  R-3102 remains the
separate Linux perf/flamegraph workstream; this validator certifies only the
repeatable benchmark and IR contract used to prioritize later optimizations.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path
import re
import subprocess
import sys
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python 3.10 fallback
    tomllib = None  # type: ignore[assignment]

try:
    from scripts.phase31_contract import (
        ASYNC_ECHO_ACCEPTED_MAX_GAP_TO_GO,
        LANGUAGES,
        OFFICIAL_INDEPENDENT_RUNS,
        SCENARIOS,
    )
except ModuleNotFoundError:
    from phase31_contract import (  # type: ignore[no-redef]
        ASYNC_ECHO_ACCEPTED_MAX_GAP_TO_GO,
        LANGUAGES,
        OFFICIAL_INDEPENDENT_RUNS,
        SCENARIOS,
    )


ROOT = Path(__file__).resolve().parents[1]
PLAN_ITEM_IDS = tuple(f"R-{number}" for number in range(3104, 3118))
EVIDENCE_SCHEMA = "spectra.phase31.r3103_evidence.v1"
REPORT_SCHEMA = "spectra.phase31.bench.v1"
IR_MANIFEST_SCHEMA = "spectra.phase31.r3103_ir_manifest.v1"
TOP_SNAPSHOT_SCENARIOS = (
    "cpu-string-build",
    "tensor-create",
    "cpu-hashmap",
    "tensor-matmul",
    "ml-mlp-step",
)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git_revision(root: Path) -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        return ""
    return result.stdout.strip()


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ValueError(f"cannot read JSON {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise ValueError(f"JSON root must be an object: {path}")
    return value


def semantic_report(report: dict[str, Any]) -> dict[str, Any]:
    scenarios: list[dict[str, Any]] = []
    for scenario in report.get("scenarios", []):
        item = {
            "id": scenario.get("id"),
            "category": scenario.get("category"),
            "iterations": scenario.get("iterations"),
            "correctness_passed": scenario.get("correctness_passed"),
            "results": {},
        }
        for language, result in sorted(scenario.get("results", {}).items()):
            item["results"][language] = {
                "command": result.get("command"),
                "exit_code": result.get("exit_code"),
                "failure_class": result.get("failure_class"),
                "error": result.get("error"),
            }
        scenarios.append(item)
    return {
        "schema": report.get("schema"),
        "profile": report.get("profile"),
        "spectra_binary": report.get("spectra_binary"),
        "git_revision": report.get("git_revision"),
        "measurement_policy": report.get("measurement_policy"),
        "scenario_ids": [item.get("id") for item in scenarios],
        "scenarios": scenarios,
    }


def scenario_ids(report: dict[str, Any]) -> list[str]:
    return [str(item.get("id")) for item in report.get("scenarios", [])]


def baseline_unchanged(before_hash: str, after_hash: str) -> bool:
    """Return whether the checked-in baseline stayed byte-for-byte identical."""
    return bool(before_hash) and before_hash == after_hash


def scenario_failure_class(item: dict[str, Any]) -> str:
    """Classify the observed evidence without implying profiler causality."""
    if item.get("correctness_passed") is not True:
        return "correctness_failure"
    if item.get("id") == "async-echo":
        if item.get("reference_performance_passed") is not True:
            return "reference_parity_failure"
        if float(item.get("paired_gap_stddev_pct") or 0) > 10.0:
            return "inconclusive"
    spectra = item.get("results", {}).get("spectra", {})
    median = spectra.get("ns_per_iter")
    dispersion = spectra.get("independent_stddev_ns", spectra.get("stddev_ns"))
    if not isinstance(median, (int, float)) or median <= 0:
        return "inconclusive"
    if not isinstance(dispersion, (int, float)) or dispersion / median * 100.0 > 10.0:
        return "inconclusive"
    for result in item.get("results", {}).values():
        if result.get("failure_class"):
            return str(result["failure_class"])
    return "none"


def validate_report(
    report: dict[str, Any],
    *,
    root: Path,
    expected_revision: str,
    baseline_hash: str,
) -> list[str]:
    errors: list[str] = []
    if report.get("schema") != REPORT_SCHEMA:
        errors.append("report schema must be spectra.phase31.bench.v1")
    if report.get("profile") != "release":
        errors.append("report profile must be release")
    if report.get("git_revision") != expected_revision:
        errors.append("report Git revision does not match current HEAD")
    binary = str(report.get("spectra_binary", ""))
    if not binary.lower().endswith("target\\release\\spectralang.exe"):
        errors.append("report must use target/release/spectralang.exe")
    ids = scenario_ids(report)
    if ids != list(SCENARIOS):
        errors.append("report must contain the canonical 21 scenarios exactly once")
    policy = report.get("measurement_policy") or {}
    if policy.get("independent_runs") != OFFICIAL_INDEPENDENT_RUNS:
        errors.append(f"report independent_runs must be exactly {OFFICIAL_INDEPENDENT_RUNS}")
    if policy.get("timed_runs") != 20 or policy.get("warmup_runs") != 3:
        errors.append("report must use 3 warmups and 20 timed samples")
    if report.get("complete_scenario_set") is not True:
        errors.append("report must declare complete_scenario_set=true")
    if baseline_hash == "":
        errors.append("baseline hash is unavailable")
    for item in report.get("scenarios", []):
        scenario_id = item.get("id")
        if item.get("correctness_passed") is not True:
            errors.append(f"{scenario_id}: correctness did not pass")
        results = item.get("results") if isinstance(item.get("results"), dict) else {}
        languages = set(results)
        missing_languages = set(LANGUAGES) - languages
        extra_languages = languages - set(LANGUAGES)
        if missing_languages:
            errors.append(
                f"{scenario_id}: missing active benchmark languages "
                f"{', '.join(sorted(missing_languages))}"
            )
        if extra_languages:
            errors.append(
                f"{scenario_id}: unsupported benchmark languages "
                f"{', '.join(sorted(extra_languages))}; Java is excluded"
            )
        for language, result in results.items():
            if result.get("exit_code") != 0:
                errors.append(f"{scenario_id}/{language}: command failed")
            if result.get("error"):
                errors.append(f"{scenario_id}/{language}: command reported an error")
        if scenario_id != "async-echo":
            spectra = results.get("spectra", {})
            median = spectra.get("ns_per_iter")
            dispersion = spectra.get("independent_stddev_ns", spectra.get("stddev_ns"))
            if not isinstance(median, (int, float)) or not isinstance(dispersion, (int, float)):
                errors.append(f"{scenario_id}: measurement is inconclusive (missing dispersion)")
            elif median <= 0 or dispersion / median * 100.0 > 10.0:
                errors.append(f"{scenario_id}: measurement is inconclusive (dispersion > 10%)")
        else:
            paired_dispersion = item.get("paired_gap_stddev_pct")
            if not isinstance(paired_dispersion, (int, float)) or paired_dispersion > 10.0:
                errors.append(f"{scenario_id}: reference measurement is inconclusive (paired dispersion > 10%)")
    async_echo = next(
        (item for item in report.get("scenarios", []) if item.get("id") == "async-echo"),
        None,
    )
    if async_echo:
        async_gap = async_echo.get("gap_to_go")
        if not isinstance(async_gap, (int, float)) or not 0 < async_gap <= ASYNC_ECHO_ACCEPTED_MAX_GAP_TO_GO:
            errors.append(
                "async-echo: Go parity gap must be within "
                f"0 < gap_to_go <= {ASYNC_ECHO_ACCEPTED_MAX_GAP_TO_GO}x"
            )
        if async_echo.get("reference_performance_passed") is not True:
            errors.append("async-echo: Go reference parity is outside the accepted window")
    for item in report.get("scenarios", []):
        for language, result in item.get("results", {}).items():
            if result.get("failure_class"):
                errors.append(f"{item.get('id')}/{language}: {result['failure_class']}")
    return errors


def file_metrics(path: Path) -> dict[str, Any]:
    text = path.read_text(encoding="utf-8", errors="replace")
    return {
        "sha256": sha256_file(path),
        "bytes": path.stat().st_size,
        "lines": text.count("\n"),
        "blocks": len(re.findall(r"(?m)^\s*block", text)),
        "allocas": len(re.findall(r"\balloca?\b", text, re.IGNORECASE)),
        "host_calls": len(re.findall(r"host.?call|spectra\.", text, re.IGNORECASE)),
    }


def validate_ir_manifest(*, root: Path, ir_root: Path, expected_revision: str) -> tuple[dict[str, Any], list[str]]:
    errors: list[str] = []
    manifest_path = ir_root / "manifest.json"
    if not manifest_path.is_file():
        return {}, ["IR manifest.json is missing"]
    try:
        manifest = load_json(manifest_path)
    except ValueError as exc:
        return {}, [str(exc)]
    if manifest.get("schema") != IR_MANIFEST_SCHEMA:
        errors.append(f"IR manifest schema must be {IR_MANIFEST_SCHEMA}")
    if manifest.get("git_revision") != expected_revision:
        errors.append("IR manifest Git revision does not match current HEAD")
    if manifest.get("profile") != "release":
        errors.append("IR manifest profile must be release")
    binary_value = str(manifest.get("binary", "")).replace("\\", "/")
    if binary_value != "target/release/spectralang.exe":
        errors.append("IR manifest must reference target/release/spectralang.exe")
    binary = root / Path(binary_value)
    if not binary.is_file():
        errors.append(f"IR manifest binary is missing: {binary}")
    elif manifest.get("binary_sha256") != sha256_file(binary):
        errors.append("IR manifest binary SHA-256 does not match the release binary")
    expected_options = {
        "o0": ["compile", "--dump-ir", "-O0"],
        "o3": ["compile", "--dump-ir", "-O3"],
    }
    if manifest.get("options") != expected_options:
        errors.append("IR manifest options do not match the required O0/O3 commands")
    if manifest.get("scenario_count") != len(SCENARIOS):
        errors.append("IR manifest scenario_count must be 21")
    if manifest.get("scenarios") != list(SCENARIOS):
        errors.append("IR manifest must contain the canonical 21 scenarios exactly once")
    files = manifest.get("files") if isinstance(manifest.get("files"), dict) else {}
    if set(files) != set(SCENARIOS):
        errors.append("IR manifest file entries must cover exactly the canonical 21 scenarios")
    for scenario in SCENARIOS:
        scenario_files = files.get(scenario) if isinstance(files.get(scenario), dict) else {}
        for level in ("o0", "o3"):
            entry = scenario_files.get(level) if isinstance(scenario_files.get(level), dict) else {}
            relative = str(entry.get("path", "")).replace("\\", "/")
            expected_relative = f"{scenario}/{level}.txt"
            if relative != expected_relative:
                errors.append(f"{scenario}: manifest path for {level} is invalid")
                continue
            path = (ir_root / Path(relative)).resolve()
            if ir_root.resolve() not in path.parents:
                errors.append(f"{scenario}: manifest path for {level} escapes the IR root")
                continue
            if not path.is_file() or path.stat().st_size == 0:
                errors.append(f"{scenario}: manifest file for {level} is missing or empty")
                continue
            if entry.get("bytes") != path.stat().st_size:
                errors.append(f"{scenario}: manifest byte count for {level} is stale")
            if entry.get("sha256") != sha256_file(path):
                errors.append(f"{scenario}: manifest SHA-256 for {level} is stale")
    return manifest, errors


def collect_ir(*, root: Path, ir_root: Path, expected_revision: str) -> dict[str, Any]:
    errors: list[str] = []
    scenarios: dict[str, Any] = {}
    manifest, manifest_errors = validate_ir_manifest(
        root=root,
        ir_root=ir_root,
        expected_revision=expected_revision,
    )
    errors.extend(manifest_errors)
    for scenario in SCENARIOS:
        directory = ir_root / scenario
        before = directory / "o0.txt"
        after = directory / "o3.txt"
        if not before.is_file() or before.stat().st_size == 0:
            errors.append(f"{scenario}: missing IR o0.txt")
            continue
        if not after.is_file() or after.stat().st_size == 0:
            errors.append(f"{scenario}: missing IR o3.txt")
            continue
        scenarios[scenario] = {"o0": file_metrics(before), "o3": file_metrics(after)}
    return {"errors": errors, "manifest": manifest, "scenarios": scenarios}


def collect_tracked_snapshots(root: Path) -> tuple[dict[str, Any], list[str]]:
    directory = root / "docs" / "performance" / "phase31-go-comparable" / "ir" / "r3103"
    snapshots: dict[str, Any] = {}
    errors: list[str] = []
    for scenario in TOP_SNAPSHOT_SCENARIOS:
        files = {}
        for level in ("o0", "o3"):
            path = directory / f"{scenario}-{level}.txt"
            if not path.is_file() or path.stat().st_size == 0:
                errors.append(f"{scenario}: missing tracked {level} IR snapshot")
                continue
            files[level] = file_metrics(path)
        if len(files) == 2:
            snapshots[scenario] = files
    return snapshots, errors


def load_roadmap(path: Path) -> dict[str, Any]:
    if tomllib is None:
        raise ValueError("tomllib is required to validate roadmap.toml")
    try:
        return tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise ValueError(f"cannot parse roadmap: {exc}") from exc


def validate_roadmap(roadmap: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    items = roadmap.get("items", [])
    ids = [item.get("id") for item in items]
    if len(ids) != len(set(ids)):
        errors.append("roadmap contains duplicate item IDs")
    item_map = {item.get("id"): item for item in items}
    phases = {phase.get("id") for phase in roadmap.get("phases", [])}
    for item in items:
        if item.get("phase") not in phases:
            errors.append(f"{item.get('id')}: unknown phase")
        for dependency in item.get("dependencies", []):
            if dependency not in item_map:
                errors.append(f"{item.get('id')}: missing dependency {dependency}")
    r3102 = item_map.get("R-3102")
    r3103 = item_map.get("R-3103")
    if not r3102 or r3102.get("status") != "in_progress":
        errors.append("R-3102 must remain in_progress")
    if not r3103:
        errors.append("R-3103 is missing")
    elif r3103.get("dependencies") != ["R-3101"]:
        errors.append("R-3103 must depend only on R-3101")
    r3104 = item_map.get("R-3104")
    if not r3104 or r3104.get("status") not in {"not_started", "in_progress", "complete"}:
        errors.append("R-3104 must be not_started, in_progress, or complete")
    return errors


def read_plan_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def plan_contains_ids(text: str) -> dict[str, bool]:
    return {item_id: item_id in text for item_id in PLAN_ITEM_IDS}


def plan_matrix_rows(text: str) -> dict[str, dict[str, str]]:
    fields = (
        "id", "scenarios", "evidence", "hypothesis", "intervention",
        "metric", "expected_gain", "rejection_risk", "rollback",
        "dependencies", "validation_command",
    )
    rows: dict[str, dict[str, str]] = {}
    for line in text.splitlines():
        if not line.startswith("| R-"):
            continue
        columns = [column.strip() for column in line.split("|")][1:-1]
        if len(columns) != len(fields):
            continue
        rows[columns[0]] = dict(zip(fields, columns))
    return rows


def validate_plan_text(path: Path) -> list[str]:
    try:
        text = read_plan_text(path)
    except OSError as exc:
        return [f"cannot read optimization plan: {exc}"]
    errors: list[str] = []
    for item_id in PLAN_ITEM_IDS:
        if item_id not in text:
            errors.append(f"optimization plan is missing {item_id}")
            continue
        rows = [line for line in text.splitlines() if line.startswith(f"| {item_id} |")]
        if len(rows) != 1:
            errors.append(f"optimization plan must contain exactly one matrix row for {item_id}")
            continue
        columns = [column.strip() for column in rows[0].split("|")]
        # Empty edge columns are expected; the matrix has 11 data columns.
        if len(columns) < 12:
            errors.append(f"optimization plan matrix row for {item_id} is incomplete")
            continue
        for column_index, label in ((2, "scenario"), (3, "evidence"), (4, "hypothesis"),
                                    (5, "intervention"), (6, "metric"), (7, "expected gain"),
                                    (8, "rejection risk"), (9, "rollback"), (10, "dependencies"),
                                    (11, "validation command")):
            if not columns[column_index]:
                errors.append(f"{item_id}: matrix field {label} is empty")
    lowered = text.lower()
    if "benchmark_and_ir_hypothesis" not in lowered:
        errors.append("optimization plan is missing required term: benchmark_and_ir_hypothesis")
    if "rejection" not in lowered and "rejeição" not in lowered:
        errors.append("optimization plan is missing required term: rejection/rejeição")
    if "rollback" not in lowered:
        errors.append("optimization plan is missing required term: rollback")
    causal_claim_patterns = (
        r"profil(?:er|ing).{0,50}\b(?:proves?|confirmed|identified)\b",
        r"causal\s+(?:hotspot|bottleneck).{0,50}\b(?:confirmed|proven|identified)\b",
        r"perf.{0,50}\b(?:proves?|confirmed)\b",
    )
    for pattern in causal_claim_patterns:
        if re.search(pattern, lowered, re.DOTALL):
            errors.append("optimization plan makes an unsupported causal profiling claim")
            break
    return errors


def report_summaries(report: dict[str, Any]) -> list[dict[str, Any]]:
    summaries: list[dict[str, Any]] = []
    for item in report.get("scenarios", []):
        spectra = item.get("results", {}).get("spectra", {})
        median = spectra.get("ns_per_iter")
        dispersion = spectra.get("independent_stddev_ns", spectra.get("stddev_ns"))
        summaries.append(
            {
                "id": item.get("id"),
                "correctness_passed": item.get("correctness_passed"),
                "median_ns": spectra.get("median_ns"),
                "p95_ns": spectra.get("p95_ns"),
                "stddev_ns": spectra.get("stddev_ns"),
                "independent_stddev_ns": spectra.get("independent_stddev_ns"),
                "dispersion_pct": (dispersion / median * 100.0
                                    if isinstance(median, (int, float)) and median > 0
                                    and isinstance(dispersion, (int, float)) else None),
                "gap_to_go": item.get("gap_to_go"),
                "reference_performance_passed": item.get("reference_performance_passed"),
                "failure_class": scenario_failure_class(item),
            }
        )
    return summaries


def build_evidence(
    *,
    root: Path,
    report_paths: list[Path],
    baseline_path: Path,
    ir_root: Path,
    roadmap_path: Path,
    plan_path: Path,
) -> tuple[dict[str, Any], str, list[str]]:
    errors: list[str] = []
    revision = git_revision(root)
    baseline_hash = sha256_file(baseline_path) if baseline_path.is_file() else ""
    reports = [load_json(path) for path in report_paths]
    if len(reports) != 2:
        errors.append("exactly two release reports are required")
    for report in reports:
        errors.extend(validate_report(report, root=root, expected_revision=revision, baseline_hash=baseline_hash))
    if len(reports) == 2 and semantic_report(reports[0]) != semantic_report(reports[1]):
        errors.append("the two reports differ semantically")
    if len({path.resolve() for path in report_paths}) != 2:
        errors.append("exactly two distinct release report files are required")
    ir = collect_ir(root=root, ir_root=ir_root, expected_revision=revision)
    errors.extend(ir["errors"])
    tracked_snapshots, tracked_snapshot_errors = collect_tracked_snapshots(root)
    errors.extend(tracked_snapshot_errors)
    roadmap = load_roadmap(roadmap_path)
    errors.extend(validate_roadmap(roadmap))
    plan_errors = validate_plan_text(plan_path)
    errors.extend(plan_errors)
    try:
        plan_text = read_plan_text(plan_path)
    except OSError:
        plan_text = ""
    baseline_after_hash = sha256_file(baseline_path) if baseline_path.is_file() else ""
    if not baseline_unchanged(baseline_hash, baseline_after_hash):
        errors.append("baseline changed while the R-3103 validator was running")
    item_map = {item.get("id"): item for item in roadmap.get("items", [])}
    plan_presence = plan_contains_ids(plan_text)
    matrix_rows = plan_matrix_rows(plan_text)
    coverage = {
        item_id: {
            "roadmap_status": item_map.get(item_id, {}).get("status"),
            "present_in_plan": plan_presence[item_id],
            "matrix": matrix_rows.get(item_id),
        }
        for item_id in PLAN_ITEM_IDS
    }
    evidence = {
        "schema": EVIDENCE_SCHEMA,
        "status": "passed" if not errors else "blocked",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "revision": revision,
        "profile": "release",
        "binary": "target/release/spectralang.exe",
        "baseline": {
            "path": str(baseline_path.relative_to(root)),
            "sha256": baseline_hash,
            "sha256_before": baseline_hash,
            "sha256_after": baseline_after_hash,
        },
        "classification": "benchmark_and_ir_hypothesis",
        "profiling_causal_claim": False,
        "reports": [
            {"path": str(path.relative_to(root)), "sha256": sha256_file(path)}
            for path in report_paths
            if path.is_file()
        ],
        "scenarios": report_summaries(reports[0]) if reports else [],
        "ir": {
            "root": str(ir_root.relative_to(root)),
            "manifest": ir["manifest"],
            "scenarios": ir["scenarios"],
            "tracked_textual_snapshots": tracked_snapshots,
        },
        "coverage": coverage,
        "failures": errors,
        "baseline_modified": not baseline_unchanged(baseline_hash, baseline_after_hash),
    }
    markdown = render_markdown(evidence)
    return evidence, markdown, errors


def render_markdown(evidence: dict[str, Any]) -> str:
    lines = [
        "# R-3103 Benchmark + IR Evidence",
        "",
        f"- Status: `{evidence['status']}`",
        f"- Schema: `{evidence['schema']}`",
        f"- Revision: `{evidence['revision']}`",
        f"- Profile: `{evidence['profile']}`",
        f"- Classification: `{evidence['classification']}`",
        f"- Profiling causal claim: `{evidence['profiling_causal_claim']}`",
        f"- Baseline modified: `{evidence['baseline_modified']}`",
        "",
        "The report is benchmark/IR evidence only. Linux perf/flamegraph attribution remains R-3102.",
        "",
        "## Report hashes",
        "",
        "| Report | SHA-256 |",
        "|---|---|",
    ]
    for report in evidence.get("reports", []):
        lines.append(f"| `{report['path']}` | `{report['sha256']}` |")
    lines.extend([
        "",
        "## Scenario evidence",
        "",
        "| Scenario | Median ns | P95 ns | Stddev ns | Dispersion % | Gap to Go | Correctness | Reference parity | Failure class |",
        "|---|---:|---:|---:|---:|---:|---|---|---|",
    ])
    for item in evidence["scenarios"]:
        lines.append(
            "| {id} | {median_ns} | {p95_ns} | {stddev_ns} | {dispersion_pct} | {gap_to_go} | {correctness_passed} | {reference_performance_passed} | {failure_class} |".format(**item)
        )
    lines.extend(["", "## Failures", ""])
    failures = evidence.get("failures") or ["none"]
    lines.extend(f"- {failure}" for failure in failures)
    lines.extend([
        "", "## IR evidence", "",
        "| Scenario | O0 SHA-256 | O0 blocks | O0 allocas | O0 host calls | O3 SHA-256 | O3 blocks | O3 allocas | O3 host calls |",
        "|---|---|---:|---:|---:|---|---:|---:|---:|",
    ])
    for scenario, snapshots in evidence.get("ir", {}).get("scenarios", {}).items():
        o0, o3 = snapshots["o0"], snapshots["o3"]
        lines.append(
            f"| {scenario} | `{o0['sha256']}` | {o0['blocks']} | {o0['allocas']} | {o0['host_calls']} | `{o3['sha256']}` | {o3['blocks']} | {o3['allocas']} | {o3['host_calls']} |"
        )
    lines.extend(["", "Tracked textual snapshots:", ""])
    for scenario, snapshots in evidence.get("ir", {}).get("tracked_textual_snapshots", {}).items():
        lines.append(f"- `{scenario}` O0 `{snapshots['o0']['sha256']}`, O3 `{snapshots['o3']['sha256']}`")
    lines.extend([
        "", "## R-3104–R-3117 coverage", "",
        "| ID | Roadmap status | Matrix row | Metric | Rejection risk | Rollback |",
        "|---|---|---|---|---|---|",
    ])
    for item_id, coverage in evidence.get("coverage", {}).items():
        matrix = coverage.get("matrix") or {}
        lines.append(
            f"| {item_id} | {coverage.get('roadmap_status')} | {coverage.get('present_in_plan')} | {matrix.get('metric', '')} | {matrix.get('rejection_risk', '')} | {matrix.get('rollback', '')} |"
        )
    lines.extend(["", "Baseline and IR hashes are generated from the current working tree; no profiler causal claim is made.", ""])
    return "\n".join(lines) + "\n"


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", action="append", required=True)
    parser.add_argument("--baseline", default="docs/performance/phase31-go-comparable/baseline.json")
    parser.add_argument("--ir-root", default="target/phase31/r3103-ir")
    parser.add_argument("--roadmap", default="roadmap/roadmap.toml")
    parser.add_argument("--plan", default="docs/performance/phase31-go-comparable/optimization-plan.md")
    parser.add_argument("--evidence", default="docs/performance/phase31-go-comparable/evidence-r3103-benchmark-ir.json")
    parser.add_argument("--evidence-md", default="docs/performance/phase31-go-comparable/evidence-r3103-benchmark-ir.md")
    parser.add_argument("--write-evidence", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    root = ROOT
    report_paths = [(root / path).resolve() for path in args.report]
    baseline_path = (root / args.baseline).resolve()
    ir_root = (root / args.ir_root).resolve()
    roadmap_path = (root / args.roadmap).resolve()
    plan_path = (root / args.plan).resolve()
    try:
        evidence, markdown, errors = build_evidence(
            root=root,
            report_paths=report_paths,
            baseline_path=baseline_path,
            ir_root=ir_root,
            roadmap_path=roadmap_path,
            plan_path=plan_path,
        )
    except ValueError as exc:
        print(f"R-3103 validator: ERROR: {exc}", file=sys.stderr)
        return 2
    if args.write_evidence:
        evidence_path = (root / args.evidence).resolve()
        markdown_path = (root / args.evidence_md).resolve()
        evidence_path.parent.mkdir(parents=True, exist_ok=True)
        markdown_path.parent.mkdir(parents=True, exist_ok=True)
        evidence_path.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
        markdown_path.write_text(markdown, encoding="utf-8")
        print(f"R-3103 evidence written: {evidence_path}")
    if errors:
        print("R-3103 validation: BLOCKED")
        for error in errors:
            print(f"- {error}")
        return 1
    print("R-3103 validation: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
