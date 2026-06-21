from __future__ import annotations

import argparse
import json
import sys
import tomllib
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_RUNNER_REPORT = ROOT / "target" / "r2011-integrated-project-runner" / "report.json"
DEFAULT_TRIAGE_REPORT = ROOT / "target" / "r2012-failure-triage" / "report.json"
ROADMAP_PATH = ROOT / "roadmap" / "roadmap.toml"
BACKLOG_PATH = ROOT / "docs" / "roadmap-backlog.md"
PLAN_PATH = ROOT / "docs" / "production-ai-implementation-plan.md"
RUN_TESTS_PATH = ROOT / "run_tests.ps1"

SCHEMA = "spectralang.r2012_failure_triage.v1"
RUNNER_SCHEMA = "spectralang.r2011_integrated_project_runner.v1"
ROADMAP_ITEM = "R-2012"
TRACKING_EXCLUDED_IDS = {"R-2008", "R-2009", "R-2010", "R-2011", "R-2012", "R-2013"}
ALLOWED_STATUSES = {"not_started", "in_progress", "blocked", "complete"}
ALLOWED_FAILURE_CLASSES = {
    "compile",
    "semantic",
    "lowering",
    "backend",
    "runtime",
    "package",
    "fixture",
    "missing-file",
    "expectation",
    "timeout",
}
REQUIRED_PROJECT_FIELDS = {
    "project_id",
    "project_name",
    "project_path",
    "entrypoint",
    "category",
    "command",
    "exact_command",
    "executed_command",
    "expected_outcome",
    "status",
    "failure_class",
    "exit_code",
    "elapsed_ms",
    "output_tail",
}


def fail(message: str, details: list[str] | None = None) -> None:
    print(f"R-2012 validation failed: {message}", file=sys.stderr)
    for detail in details or []:
        print(f"- {detail}", file=sys.stderr)
    raise SystemExit(1)


def load_toml(path: Path) -> dict[str, Any]:
    with path.open("rb") as handle:
        return tomllib.load(handle)


def load_json(path: Path) -> dict[str, Any]:
    if not path.is_file():
        fail(f"runner report missing: {path.relative_to(ROOT)}")
    with path.open("r", encoding="utf-8") as handle:
        data = json.load(handle)
    if not isinstance(data, dict):
        fail("runner report must be a JSON object")
    return data


def roadmap_items(roadmap: dict[str, Any]) -> dict[str, dict[str, Any]]:
    items = roadmap.get("items")
    if not isinstance(items, list):
        fail("roadmap.toml must contain [[items]]")
    by_id: dict[str, dict[str, Any]] = {}
    duplicates: list[str] = []
    for item in items:
        if not isinstance(item, dict):
            continue
        item_id = item.get("id")
        if not isinstance(item_id, str):
            continue
        if item_id in by_id:
            duplicates.append(item_id)
        by_id[item_id] = item
    if duplicates:
        fail("duplicate roadmap item IDs", sorted(set(duplicates)))
    return by_id


def flatten_text(value: Any) -> str:
    if isinstance(value, str):
        return value
    if isinstance(value, list):
        return "\n".join(flatten_text(item) for item in value)
    if isinstance(value, dict):
        return "\n".join(flatten_text(item) for item in value.values())
    return ""


def validate_wiring(roadmap_by_id: dict[str, dict[str, Any]]) -> None:
    errors: list[str] = []
    item = roadmap_by_id.get(ROADMAP_ITEM)
    if not item:
        errors.append("R-2012 missing from roadmap.toml")
    else:
        if item.get("status") != "complete":
            errors.append("R-2012 roadmap status must be complete")
        if item.get("owner") != "ecosystem":
            errors.append("R-2012 owner must be ecosystem")
        if "R-2011" not in item.get("dependencies", []):
            errors.append("R-2012 must depend on R-2011")

    docs = {
        "roadmap-backlog.md": BACKLOG_PATH.read_text(encoding="utf-8"),
        "production-ai-implementation-plan.md": PLAN_PATH.read_text(encoding="utf-8"),
        "run_tests.ps1": RUN_TESTS_PATH.read_text(encoding="utf-8"),
    }
    required_tokens = [
        "R-2012",
        "validate_r2012_failure_triage.py",
        "r2012-failure-triage",
    ]
    for label, text in docs.items():
        for token in required_tokens:
            if token not in text:
                errors.append(f"{label} missing {token}")
    if 'Teste = "validate_r2012_failure_triage"' not in docs["run_tests.ps1"]:
        errors.append("run_tests.ps1 must record validate_r2012_failure_triage")

    if errors:
        fail("R-2012 planning or run_tests wiring incomplete", errors)


def validate_runner_report(report: dict[str, Any]) -> list[dict[str, Any]]:
    errors: list[str] = []
    if report.get("schema") != RUNNER_SCHEMA:
        errors.append(f"runner report schema must be {RUNNER_SCHEMA}")
    if report.get("roadmap_item") != "R-2011":
        errors.append("runner report roadmap_item must be R-2011")
    projects = report.get("projects")
    if not isinstance(projects, list) or not projects:
        errors.append("runner report projects must be a non-empty array")
        projects = []

    for index, project in enumerate(projects):
        if not isinstance(project, dict):
            errors.append(f"project #{index + 1}: must be object")
            continue
        project_id = project.get("project_id", f"project #{index + 1}")
        missing_fields = sorted(REQUIRED_PROJECT_FIELDS - set(project))
        if missing_fields:
            errors.append(f"{project_id}: missing report fields {missing_fields}")
        status = project.get("status")
        if status not in {"passed", "failed"}:
            errors.append(f"{project_id}: status must be passed or failed")
        failure_class = project.get("failure_class")
        if status == "failed":
            if failure_class not in ALLOWED_FAILURE_CLASSES:
                errors.append(f"{project_id}: invalid failure_class {failure_class!r}")
            if not str(project.get("output_tail", "")).strip():
                errors.append(f"{project_id}: failed project must preserve output_tail")
        elif failure_class is not None:
            errors.append(f"{project_id}: passed project must have null failure_class")
        if not isinstance(project.get("elapsed_ms"), int):
            errors.append(f"{project_id}: elapsed_ms must be integer")
        if not isinstance(project.get("executed_command"), list):
            errors.append(f"{project_id}: executed_command must be array")

    if errors:
        fail("R-2011 report is not triage-ready", errors)

    return [project for project in projects if project.get("status") == "failed"]


def item_tracks_failure(item: dict[str, Any], failure: dict[str, Any]) -> tuple[bool, list[str]]:
    missing: list[str] = []
    item_id = item.get("id", "<unknown>")
    text = flatten_text(item)

    for field in ("owner", "phase", "dependencies", "risk", "acceptance"):
        if field not in item or item[field] in (None, "", []):
            missing.append(f"{item_id}: missing {field}")
    if item.get("status") not in ALLOWED_STATUSES:
        missing.append(f"{item_id}: invalid status {item.get('status')!r}")

    project_path = failure.get("project_path")
    exact_command = failure.get("exact_command")
    failure_class = failure.get("failure_class")
    project_id = failure.get("project_id")

    tokens = [project_path, exact_command, failure_class, project_id]
    if not all(isinstance(token, str) and token and token in text for token in tokens):
        return False, missing
    return True, missing


def find_tracking_item(
    failure: dict[str, Any], roadmap_by_id: dict[str, dict[str, Any]], backlog_text: str
) -> tuple[str | None, list[str]]:
    candidate_errors: list[str] = []
    for item_id, item in sorted(roadmap_by_id.items()):
        if item_id in TRACKING_EXCLUDED_IDS:
            continue
        tracked, missing = item_tracks_failure(item, failure)
        if not tracked:
            continue
        if item_id not in backlog_text:
            missing.append(f"{item_id}: missing from backlog")
        for token in (
            str(failure.get("project_path", "")),
            str(failure.get("exact_command", "")),
            str(failure.get("failure_class", "")),
        ):
            if token and token not in backlog_text:
                missing.append(f"{item_id}: backlog missing {token}")
        if missing:
            candidate_errors.extend(missing)
            continue
        return item_id, []
    return None, candidate_errors


def triage_failures(
    failures: list[dict[str, Any]], roadmap_by_id: dict[str, dict[str, Any]], backlog_text: str
) -> tuple[list[dict[str, Any]], list[str]]:
    records: list[dict[str, Any]] = []
    errors: list[str] = []
    for failure in failures:
        tracking_id, candidate_errors = find_tracking_item(failure, roadmap_by_id, backlog_text)
        record = {
            "project_id": failure.get("project_id"),
            "project_path": failure.get("project_path"),
            "exact_command": failure.get("exact_command"),
            "failure_class": failure.get("failure_class"),
            "tracked_by": tracking_id,
        }
        records.append(record)
        if tracking_id is None:
            errors.append(
                "untracked failure: "
                f"{failure.get('project_id')} {failure.get('failure_class')} "
                f"{failure.get('project_path')}"
            )
            errors.extend(candidate_errors)
    return records, errors


def write_report(
    report_path: Path,
    runner_report_path: Path,
    failures: list[dict[str, Any]],
    triage_records: list[dict[str, Any]],
) -> dict[str, Any]:
    failed = len(failures)
    tracked = sum(1 for record in triage_records if record["tracked_by"])
    status = "passed" if failed == tracked else "failed"
    report = {
        "schema": SCHEMA,
        "roadmap_item": ROADMAP_ITEM,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "runner_report": str(runner_report_path.relative_to(ROOT)).replace("\\", "/"),
        "status": status,
        "summary": {
            "failed_projects": failed,
            "tracked_failures": tracked,
            "untracked_failures": failed - tracked,
        },
        "triage": triage_records,
    }
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return report


def main() -> None:
    parser = argparse.ArgumentParser(description="Validate R-2012 failure-to-roadmap triage.")
    parser.add_argument("--runner-report", default=str(DEFAULT_RUNNER_REPORT))
    parser.add_argument("--report", default=str(DEFAULT_TRIAGE_REPORT))
    args = parser.parse_args()

    runner_report_path = Path(args.runner_report)
    if not runner_report_path.is_absolute():
        runner_report_path = ROOT / runner_report_path
    triage_report_path = Path(args.report)
    if not triage_report_path.is_absolute():
        triage_report_path = ROOT / triage_report_path

    roadmap = load_toml(ROADMAP_PATH)
    roadmap_by_id = roadmap_items(roadmap)
    validate_wiring(roadmap_by_id)

    runner_report = load_json(runner_report_path)
    failures = validate_runner_report(runner_report)
    triage_records, errors = triage_failures(
        failures,
        roadmap_by_id,
        BACKLOG_PATH.read_text(encoding="utf-8"),
    )
    report = write_report(triage_report_path, runner_report_path, failures, triage_records)

    summary = report["summary"]
    print(
        "R-2012 failure triage: "
        f"{summary['tracked_failures']} tracked, "
        f"{summary['untracked_failures']} untracked, "
        f"{summary['failed_projects']} failures"
    )
    print(f"Report: {triage_report_path.relative_to(ROOT)}")

    if errors:
        fail("untracked integrated project failures remain", errors)


if __name__ == "__main__":
    main()
