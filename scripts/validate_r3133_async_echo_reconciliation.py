#!/usr/bin/env python3
"""Validate current-revision async-echo batch evidence for R-3133.

The validator is intentionally diagnostic-only: it never updates the Phase 31
baseline and never treats benchmark timing as causal profiler evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path
import subprocess
import sys
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    tomllib = None  # type: ignore[assignment]

try:
    from scripts.phase31_contract import SCENARIOS
    from scripts.compare_phase31_reports import semantic_report
except ModuleNotFoundError:  # direct script execution
    from phase31_contract import SCENARIOS  # type: ignore[no-redef]
    from compare_phase31_reports import semantic_report  # type: ignore[no-redef]


ROOT = Path(__file__).resolve().parents[1]
DIAGNOSTIC_SCHEMA = "spectra.phase31.async_echo_diagnostics.v2"
REPORT_SCHEMA = "spectra.phase31.bench.v1"
CONTRACT = "fanout_fanin_real_concurrency.v2"
BATCH_VARIANTS = (
    "batch-reset-only",
    "batch-spawn-only",
    "batch-join-only",
    "batch-full",
    "batch-full-no-reset",
)
REQUIRED_BATCH_METRICS = (
    "locks_acquired",
    "scheduler_ns",
    "execution_ns",
    "tasks_counted",
    "tasks_created",
    "tasks_executed",
    "task_joins",
    "batches_created",
    "batches_joined",
    "batch_spawn_fast_abi_calls",
    "batch_join_fast_abi_calls",
    "max_pending_tasks",
)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git_revision(root: Path) -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=root, capture_output=True, text=True, check=False
    )
    return result.stdout.strip() if result.returncode == 0 else ""


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"JSON root must be an object: {path}")
    return value


def baseline_unchanged(before: str, after: str) -> bool:
    return bool(before) and before == after


def validate_diagnostic(
    diagnostic: dict[str, Any], *, expected_revision: str, binary_suffix: str
) -> list[str]:
    errors: list[str] = []
    if diagnostic.get("schema") != DIAGNOSTIC_SCHEMA:
        errors.append("diagnostic schema must be spectra.phase31.async_echo_diagnostics.v2")
    if diagnostic.get("git_revision") != expected_revision:
        errors.append("diagnostic Git revision does not match current HEAD")
    if diagnostic.get("profile") != "release":
        errors.append("diagnostic profile must be release")
    binary = str(diagnostic.get("spectra_binary", ""))
    if not binary.lower().endswith(binary_suffix.lower()):
        errors.append("diagnostic must use target/release/spectralang.exe")
    if diagnostic.get("workload_contract") != CONTRACT:
        errors.append("diagnostic workload contract must be fanout_fanin_real_concurrency.v2")
    if diagnostic.get("causal_profiling") is True or diagnostic.get("profiling_artifact"):
        errors.append("diagnostic cannot claim causal profiling without an official perf artifact")
    variants = diagnostic.get("variants")
    if not isinstance(variants, dict):
        return errors + ["diagnostic variants must be an object"]
    missing = [name for name in BATCH_VARIANTS if name not in variants]
    if missing:
        errors.append(f"diagnostic missing batch variants: {', '.join(missing)}")
    for name in BATCH_VARIANTS:
        item = variants.get(name)
        if not isinstance(item, dict):
            errors.append(f"{name}: diagnostic variant must be an object")
            continue
        if item.get("contract") != CONTRACT:
            errors.append(f"{name}: contract mismatch")
        if item.get("process_inclusive") is not True:
            errors.append(f"{name}: process_inclusive must be true")
        if item.get("ok") is not True:
            errors.append(f"{name}: diagnostic execution failed")
        if not isinstance(item.get("median_ns"), (int, float)) or item.get("median_ns", 0) <= 0:
            errors.append(f"{name}: median_ns is missing")
        if not isinstance(item.get("stddev_pct"), (int, float)):
            errors.append(f"{name}: stddev_pct is missing")
        metrics = item.get("diagnostics") or {}
        if name in {"batch-full", "batch-full-no-reset"}:
            for metric in REQUIRED_BATCH_METRICS:
                if metric not in metrics:
                    errors.append(f"{name}: missing diagnostic metric {metric}")
    return errors


def _scenario(report: dict[str, Any], scenario_id: str) -> dict[str, Any] | None:
    return next((item for item in report.get("scenarios", []) if item.get("id") == scenario_id), None)


def validate_report(
    report: dict[str, Any], *, expected_revision: str, baseline: dict[str, Any]
) -> list[str]:
    errors: list[str] = []
    if report.get("causal_profiling") is True or report.get("profiling_artifact"):
        errors.append("report cannot claim causal profiling without an official perf artifact")
    if report.get("schema") != REPORT_SCHEMA:
        errors.append("report schema must be spectra.phase31.bench.v1")
    if report.get("profile") != "release":
        errors.append("report profile must be release")
    if report.get("git_revision") != expected_revision:
        errors.append("report Git revision does not match current HEAD")
    binary = str(report.get("spectra_binary", ""))
    if not binary.lower().endswith("target\\release\\spectralang.exe"):
        errors.append("report must use target/release/spectralang.exe")
    if [item.get("id") for item in report.get("scenarios", [])] != list(SCENARIOS):
        errors.append("report must contain the canonical 21 scenarios in order")
    policy = report.get("measurement_policy") or {}
    if policy.get("independent_runs", 0) < 5 or policy.get("warmup_runs") != 3 or policy.get("timed_runs") != 20:
        errors.append("report must use five independent runs, three warmups, and twenty timed samples")
    async_entry = _scenario(report, "async-echo")
    if not async_entry:
        errors.append("report is missing async-echo")
    else:
        gap = async_entry.get("gap_to_go")
        if not isinstance(gap, (int, float)) or not 0.95 <= gap <= 1.05:
            errors.append("async-echo: gap to Go is outside 0.95..1.05")
        if async_entry.get("reference_performance_passed") is not True:
            errors.append("async-echo: reference_performance_passed is not true")
        if (async_entry.get("paired_gap_stddev_pct") or 0) > 10:
            errors.append("async-echo: paired ratio dispersion exceeds 10%")
    pipeline = _scenario(report, "async-pipeline")
    baseline_entry = baseline.get("scenarios", {}).get("async-pipeline", {})
    observed = (pipeline or {}).get("results", {}).get("spectra", {}).get("ns_per_iter")
    expected = baseline_entry.get("spectra_ns_per_iter")
    if not isinstance(observed, (int, float)) or not isinstance(expected, (int, float)):
        errors.append("async-pipeline: missing baseline or current median")
    elif expected > 0 and (observed / expected - 1.0) * 100.0 > 5.0:
        errors.append("async-pipeline: drift exceeds 5%")
    for item in report.get("scenarios", []):
        if item.get("correctness_passed") is not True:
            errors.append(f"{item.get('id')}: correctness did not pass")
    return errors


def classify_cause(diagnostic: dict[str, Any], reports: list[dict[str, Any]]) -> dict[str, Any]:
    variants = diagnostic.get("variants", {})
    full = variants.get("batch-full", {})
    startup = variants.get("startup", {})
    metrics = full.get("diagnostics") or {}
    paired_noise = [
        item.get("paired_gap_stddev_pct", 0)
        for report in reports
        for item in report.get("scenarios", [])
        if item.get("id") == "async-echo"
    ]
    if any(value > 10 for value in paired_noise) or (full.get("stddev_pct", 0) or 0) > 10:
        category = "external_noise"
        confidence = "high"
        reason = "paired async-echo or process-inclusive batch dispersion exceeded 10%"
    elif full.get("contract") != CONTRACT:
        category = "benchmark_contract"
        confidence = "high"
        reason = "batch-full does not carry the current fanout/fanin contract"
    elif any(metric not in metrics for metric in REQUIRED_BATCH_METRICS):
        category = "compiler_backend_lowering"
        confidence = "medium"
        reason = "the current batch path cannot prove direct Fast ABI accounting"
    elif (
        isinstance(startup.get("median_ns"), (int, float))
        and isinstance(full.get("median_ns"), (int, float))
        and full["median_ns"] > 0
        and startup["median_ns"] / full["median_ns"] >= 0.90
    ):
        category = "benchmark_process_startup"
        confidence = "medium"
        reason = "startup accounts for at least 90% of the batch-full process-inclusive median"
    else:
        category = "runtime_batch_path"
        confidence = "medium"
        reason = "batch-full remains slower after startup and direct batch accounting is present"
    return {"category": category, "confidence": confidence, "reason": reason}


def load_roadmap(path: Path) -> dict[str, Any]:
    if tomllib is None:
        raise ValueError("tomllib is required")
    return tomllib.loads(path.read_text(encoding="utf-8"))


def validate_roadmap(roadmap: dict[str, Any]) -> list[str]:
    items = {item.get("id"): item for item in roadmap.get("items", [])}
    errors: list[str] = []
    r3131 = items.get("R-3131")
    r3103 = items.get("R-3103")
    r3104 = items.get("R-3104")
    if not r3131 or r3131.get("status") not in {"complete", "in_progress"}:
        errors.append("R-3131 must be complete or in_progress")
    if not r3103 or r3103.get("status") != "in_progress":
        errors.append("R-3103 must remain in_progress")
    if not r3104 or r3104.get("status") != "not_started":
        errors.append("R-3104 must remain not_started")
    r3133 = items.get("R-3133")
    if not r3133:
        errors.append("R-3133 is missing")
    else:
        expected = ["R-3130", "R-3131", "R-3132"]
        if r3133.get("dependencies") != expected:
            errors.append("R-3133 dependencies must be R-3130, R-3131, R-3132")
    return errors


def build_evidence(
    *, root: Path, diagnostic_path: Path, report_paths: list[Path], baseline_path: Path, roadmap_path: Path
) -> tuple[dict[str, Any], str, list[str]]:
    revision = git_revision(root)
    before = sha256_file(baseline_path) if baseline_path.is_file() else ""
    diagnostic = load_json(diagnostic_path)
    reports = [load_json(path) for path in report_paths]
    baseline = load_json(baseline_path)
    errors: list[str] = validate_diagnostic(
        diagnostic, expected_revision=revision, binary_suffix="target\\release\\spectralang.exe"
    )
    for report in reports:
        errors.extend(validate_report(report, expected_revision=revision, baseline=baseline))
    flattened = list(errors)
    if len(reports) != 2:
        flattened.append("exactly two release reports are required")
    elif semantic_report(reports[0]) != semantic_report(reports[1]):
        flattened.append("release reports differ semantically")
    roadmap = load_roadmap(roadmap_path)
    flattened.extend(validate_roadmap(roadmap))
    after = sha256_file(baseline_path) if baseline_path.is_file() else ""
    if not baseline_unchanged(before, after):
        flattened.append("baseline changed while R-3133 validation was running")
    classification = classify_cause(diagnostic, reports)
    evidence = {
        "schema": "spectra.phase31.r3133_async_echo_reconciliation.v1",
        "status": "passed" if not flattened else "blocked",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "revision": revision,
        "profile": "release",
        "binary": "target/release/spectralang.exe",
        "workload_contract": CONTRACT,
        "classification": classification,
        "baseline": {
            "path": str(baseline_path.relative_to(root)),
            "sha256_before": before,
            "sha256_after": after,
            "modified": not baseline_unchanged(before, after),
        },
        "diagnostic": {"path": str(diagnostic_path.relative_to(root)), "sha256": sha256_file(diagnostic_path)},
        "reports": [
            {"path": str(path.relative_to(root)), "sha256": sha256_file(path)}
            for path in report_paths if path.is_file()
        ],
        "reports_semantically_compatible": len(reports) == 2 and semantic_report(reports[0]) == semantic_report(reports[1]),
        "scenario_contract": list(SCENARIOS),
        "scenarios": [
            {
                "id": item.get("id"),
                "correctness_passed": item.get("correctness_passed"),
                "spectra": {
                    key: (item.get("results", {}).get("spectra", {}) or {}).get(key)
                    for key in ("median_ns", "p95_ns", "stddev_ns", "ns_per_iter", "failure_class")
                },
                "gap_to_go": item.get("gap_to_go"),
                "reference_performance_passed": item.get("reference_performance_passed"),
            }
            for item in (reports[0].get("scenarios", []) if reports else [])
        ],
        "async_echo": [
            {
                "report": str(path.relative_to(root)),
                "gap_to_go": (_scenario(report, "async-echo") or {}).get("gap_to_go"),
                "paired_gap_stddev_pct": (_scenario(report, "async-echo") or {}).get("paired_gap_stddev_pct"),
                "reference_performance_passed": (_scenario(report, "async-echo") or {}).get("reference_performance_passed"),
            }
            for path, report in zip(report_paths, reports)
        ],
        "batch_variants": {
            name: {
                "median_ns": diagnostic.get("variants", {}).get(name, {}).get("median_ns"),
                "stddev_pct": diagnostic.get("variants", {}).get(name, {}).get("stddev_pct"),
                "diagnostics": diagnostic.get("variants", {}).get(name, {}).get("diagnostics"),
            }
            for name in BATCH_VARIANTS
        },
        "failures": flattened,
        "baseline_modified": not baseline_unchanged(before, after),
    }
    markdown = render_markdown(evidence)
    return evidence, markdown, flattened


def render_markdown(evidence: dict[str, Any]) -> str:
    lines = [
        "# R-3133 Async Echo Reconciliation",
        "",
        f"- Status: `{evidence['status']}`",
        f"- Revision: `{evidence['revision']}`",
        f"- Classification: `{evidence['classification']['category']}` ({evidence['classification']['confidence']})",
        f"- Baseline modified: `{evidence['baseline_modified']}`",
        "",
        "This report covers the current batch benchmark only; historical R-3131/R-3132 evidence remains unchanged.",
        "",
        "## Async-echo reports",
        "",
        "| Report | Gap to Go | Paired dispersion | Parity |",
        "|---|---:|---:|---|",
    ]
    for item in evidence["async_echo"]:
        lines.append(f"| `{item['report']}` | {item['gap_to_go']} | {item['paired_gap_stddev_pct']}% | {item['reference_performance_passed']} |")
    lines.extend(["", "## Batch variants", "", "| Variant | Median ns | Stddev % |", "|---|---:|---:|"])
    for name, item in evidence["batch_variants"].items():
        lines.append(f"| `{name}` | {item['median_ns']} | {item['stddev_pct']} |")
    lines.extend(["", "## Failures", ""])
    lines.extend(f"- {failure}" for failure in evidence.get("failures") or ["none"])
    return "\n".join(lines) + "\n"


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--diagnostic", default="target/phase31/async-echo-diagnostics/r3133-release.json")
    parser.add_argument("--report", action="append", required=True)
    parser.add_argument("--baseline", default="docs/performance/phase31-go-comparable/baseline.json")
    parser.add_argument("--roadmap", default="roadmap/roadmap.toml")
    parser.add_argument("--evidence", default="docs/performance/phase31-go-comparable/evidence-r3133-async-echo.json")
    parser.add_argument("--evidence-md", default="docs/performance/phase31-go-comparable/evidence-r3133-async-echo.md")
    parser.add_argument("--write-evidence", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    paths = {name: (ROOT / value).resolve() for name, value in {
        "diagnostic": args.diagnostic, "baseline": args.baseline, "roadmap": args.roadmap,
        "evidence": args.evidence, "evidence_md": args.evidence_md,
    }.items()}
    reports = [(ROOT / value).resolve() for value in args.report]
    try:
        evidence, markdown, errors = build_evidence(
            root=ROOT, diagnostic_path=paths["diagnostic"], report_paths=reports,
            baseline_path=paths["baseline"], roadmap_path=paths["roadmap"],
        )
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"R-3133 validator: ERROR: {exc}", file=sys.stderr)
        return 2
    if args.write_evidence:
        paths["evidence"].parent.mkdir(parents=True, exist_ok=True)
        paths["evidence_md"].parent.mkdir(parents=True, exist_ok=True)
        paths["evidence"].write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
        paths["evidence_md"].write_text(markdown, encoding="utf-8")
        print(f"R-3133 evidence written: {paths['evidence']}")
    if errors:
        print("R-3133 validation: BLOCKED")
        for error in errors:
            print(f"- {error}")
        return 1
    print("R-3133 validation: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
