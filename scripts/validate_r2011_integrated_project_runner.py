from __future__ import annotations

import argparse
import json
import os
import shlex
import shutil
import subprocess
import sys
import time
import tomllib
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
MATRIX_PATH = ROOT / "docs" / "architecture" / "r2008-language-feature-project-matrix.toml"
ROADMAP_PATH = ROOT / "roadmap" / "roadmap.toml"
BACKLOG_PATH = ROOT / "docs" / "roadmap-backlog.md"
PLAN_PATH = ROOT / "docs" / "production-ai-implementation-plan.md"
RUN_TESTS_PATH = ROOT / "run_tests.ps1"
REPORT_PATH = ROOT / "target" / "r2011-integrated-project-runner" / "report.json"

SCHEMA = "spectralang.r2011_integrated_project_runner.v1"
ROADMAP_ITEM = "R-2011"
MATRIX_SCHEMA = "spectralang.r2008_language_feature_project_matrix.v1"
TAIL_LINES = 80


def load_toml(path: Path) -> dict[str, Any]:
    with path.open("rb") as handle:
        return tomllib.load(handle)


def fail(message: str, details: list[str] | None = None) -> None:
    print(f"R-2011 validation failed: {message}", file=sys.stderr)
    for detail in details or []:
        print(f"- {detail}", file=sys.stderr)
    raise SystemExit(1)


def roadmap_items(roadmap: dict[str, Any]) -> dict[str, dict[str, Any]]:
    items = roadmap.get("items")
    if not isinstance(items, list):
        fail("roadmap.toml must contain an [[items]] array")
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
        fail("roadmap.toml contains duplicate item IDs", sorted(set(duplicates)))
    return by_id


def resolve_binary(raw_binary: str | None) -> Path | str:
    if raw_binary:
        binary = Path(raw_binary)
        if not binary.is_file():
            fail(f"--binary does not exist: {raw_binary}")
        return binary

    candidate = ROOT / "target" / "debug" / ("spectralang.exe" if os.name == "nt" else "spectralang")
    if candidate.is_file():
        return candidate

    path_binary = shutil.which("spectralang")
    if path_binary:
        return path_binary

    fail(
        "cannot resolve spectralang binary",
        [
            "build the CLI first with cargo build -p spectra-cli",
            "or pass --binary target\\debug\\spectralang.exe",
        ],
    )


def command_tokens(project_id: str, exact_command: str) -> list[str]:
    try:
        tokens = shlex.split(exact_command)
    except ValueError as error:
        fail(f"{project_id}: exact_command is not shell-tokenizable", [str(error)])
    if not tokens or tokens[0] != "spectralang":
        fail(f"{project_id}: exact_command must start with spectralang")
    return tokens


def output_tail(stdout: str, stderr: str) -> str:
    lines = (stdout + stderr).splitlines()
    return "\n".join(lines[-TAIL_LINES:])


def classify_missing_files(missing_files: list[str]) -> str | None:
    if not missing_files:
        return None
    if any("/fixtures/" in path.replace("\\", "/") for path in missing_files):
        return "fixture"
    return "missing-file"


def classify_failure(command: str, output: str, timed_out: bool, missing_files: list[str]) -> str | None:
    lower = output.lower()
    missing_class = classify_missing_files(missing_files)
    if missing_class:
        return missing_class
    if timed_out:
        return "timeout"
    if "error[semantic]" in lower or "semantic" in lower and "error" in lower:
        return "semantic"
    if "error[lowering]" in lower or "ir verification" in lower or "undefined value" in lower:
        return "lowering"
    if "error[codegen]" in lower or "backend" in lower or "cranelift" in lower:
        return "backend"
    if "error[compile]" in lower or "compilation failed" in lower or "error:" in lower:
        return "compile"
    if "runtime" in lower or "execution failed" in lower or "panicked" in lower:
        return "runtime"
    if "spectralang package" in command or "package test" in command or "package check" in command:
        return "package"
    return "expectation"


def validate_matrix_contract(matrix: dict[str, Any], roadmap_by_id: dict[str, dict[str, Any]]) -> list[dict[str, Any]]:
    errors: list[str] = []
    if matrix.get("schema") != MATRIX_SCHEMA:
        errors.append(f"matrix schema must be {MATRIX_SCHEMA}")
    if matrix.get("status") != "complete":
        errors.append("matrix status must be complete")
    projects = matrix.get("projects")
    if not isinstance(projects, list) or not projects:
        errors.append("matrix must contain non-empty projects array")
        projects = []

    for item_id in ("R-2009", "R-2010", "R-2011"):
        item = roadmap_by_id.get(item_id)
        if not item:
            errors.append(f"{item_id} missing from roadmap.toml")
        elif item_id in {"R-2009", "R-2010", "R-2011"} and item.get("status") != "complete":
            errors.append(f"{item_id} must be complete before R-2011 runner execution")

    for project in projects:
        if not isinstance(project, dict):
            errors.append("matrix project entry must be a table")
            continue
        project_id = project.get("id", "<missing id>")
        for field in (
            "title",
            "category",
            "command",
            "exact_command",
            "project_path",
            "entrypoint",
            "expected_outcome",
        ):
            if not isinstance(project.get(field), str) or not project[field].strip():
                errors.append(f"{project_id}: {field} must be non-empty string")
        required_files = project.get("required_files")
        if not isinstance(required_files, list) or not all(isinstance(value, str) for value in required_files):
            errors.append(f"{project_id}: required_files must be string array")

    if errors:
        fail("runner prerequisites are not satisfied", errors)

    return projects


def validate_runner_wiring() -> None:
    required_docs = {
        "roadmap-backlog.md": BACKLOG_PATH,
        "production-ai-implementation-plan.md": PLAN_PATH,
    }
    errors: list[str] = []
    required_tokens = [
        "R-2011",
        "validate_r2011_integrated_project_runner.py",
        "r2011-integrated-project-runner",
    ]
    for label, path in required_docs.items():
        text = path.read_text(encoding="utf-8")
        for token in required_tokens:
            if token not in text:
                errors.append(f"{label} missing {token}")
    run_tests = RUN_TESTS_PATH.read_text(encoding="utf-8")
    direct_wiring = all(
        token in run_tests
        for token in (
            "R-2011",
            "validate_r2011_integrated_project_runner.py",
            'Teste = "validate_r2011_integrated_project_runner"',
        )
    )
    aggregate_wiring = all(
        token in run_tests
        for token in ("R-2013", "validate_r2013_release_candidate.py")
    )
    if not direct_wiring and not aggregate_wiring:
        errors.append(
            "run_tests.ps1 must invoke R-2011 directly or delegate it through R-2013"
        )
    if errors:
        fail("R-2011 runner wiring is incomplete", errors)


def required_file_errors(project: dict[str, Any]) -> list[str]:
    project_root = ROOT / project["project_path"]
    missing: list[str] = []
    if not project_root.is_dir():
        missing.append(project["project_path"])
        return missing
    for rel in project["required_files"]:
        if not (project_root / rel).is_file():
            missing.append(f"{project['project_path']}/{rel}")
    entrypoint = project["entrypoint"]
    if not (project_root / entrypoint).is_file():
        missing.append(f"{project['project_path']}/{entrypoint}")
    return sorted(set(missing))


def run_project(project: dict[str, Any], binary: Path | str, timeout: int) -> dict[str, Any]:
    project_id = project["id"]
    exact_command = project["exact_command"]
    tokens = command_tokens(project_id, exact_command)
    executed_command = [str(binary), *tokens[1:]]
    missing = required_file_errors(project)

    record: dict[str, Any] = {
        "project_id": project_id,
        "project_name": project["title"],
        "project_path": project["project_path"],
        "entrypoint": project["entrypoint"],
        "category": project["category"],
        "roadmap_item": project.get("roadmap_item"),
        "owner": project.get("owner"),
        "command": project["command"],
        "exact_command": exact_command,
        "executed_command": executed_command,
        "expected_outcome": project["expected_outcome"],
        "status": "failed" if missing else "pending",
        "failure_class": classify_missing_files(missing),
        "exit_code": None,
        "elapsed_ms": 0,
        "output_tail": "",
        "missing_files": missing,
    }
    if missing:
        record["output_tail"] = "\n".join(f"missing: {path}" for path in missing)
        return record

    start = time.perf_counter()
    try:
        completed = subprocess.run(
            executed_command,
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
            check=False,
        )
        elapsed_ms = int((time.perf_counter() - start) * 1000)
        tail = output_tail(completed.stdout, completed.stderr)
        passed = completed.returncode == 0
        record.update(
            {
                "status": "passed" if passed else "failed",
                "failure_class": None
                if passed
                else classify_failure(exact_command, completed.stdout + completed.stderr, False, []),
                "exit_code": completed.returncode,
                "elapsed_ms": elapsed_ms,
                "output_tail": tail,
            }
        )
    except subprocess.TimeoutExpired as error:
        elapsed_ms = int((time.perf_counter() - start) * 1000)
        stdout = error.stdout if isinstance(error.stdout, str) else ""
        stderr = error.stderr if isinstance(error.stderr, str) else ""
        record.update(
            {
                "status": "failed",
                "failure_class": "timeout",
                "exit_code": None,
                "elapsed_ms": elapsed_ms,
                "output_tail": output_tail(stdout, stderr),
            }
        )
    return record


def build_report(project_results: list[dict[str, Any]], binary: Path | str, timeout: int, matrix: dict[str, Any]) -> dict[str, Any]:
    passed = sum(1 for result in project_results if result["status"] == "passed")
    failed = len(project_results) - passed
    failure_classes: dict[str, int] = {}
    for result in project_results:
        klass = result.get("failure_class")
        if klass:
            failure_classes[klass] = failure_classes.get(klass, 0) + 1

    return {
        "schema": SCHEMA,
        "roadmap_item": ROADMAP_ITEM,
        "matrix_path": str(MATRIX_PATH.relative_to(ROOT)).replace("\\", "/"),
        "matrix_schema": matrix.get("schema"),
        "matrix_version": matrix.get("version"),
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "binary": str(binary),
        "timeout_seconds": timeout,
        "status": "passed" if failed == 0 else "failed",
        "summary": {
            "project_count": len(project_results),
            "passed": passed,
            "failed": failed,
            "failure_classes": failure_classes,
        },
        "projects": project_results,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description="Run R-2011 integrated project matrix commands.")
    parser.add_argument("--binary", help="Path to spectralang executable.")
    parser.add_argument("--timeout", type=int, default=180, help="Per-project timeout in seconds.")
    parser.add_argument("--report", default=str(REPORT_PATH), help="JSON report path.")
    args = parser.parse_args()

    matrix = load_toml(MATRIX_PATH)
    roadmap = load_toml(ROADMAP_PATH)
    projects = validate_matrix_contract(matrix, roadmap_items(roadmap))
    validate_runner_wiring()
    binary = resolve_binary(args.binary)

    results = [run_project(project, binary, args.timeout) for project in projects]
    report = build_report(results, binary, args.timeout, matrix)

    report_path = Path(args.report)
    if not report_path.is_absolute():
        report_path = ROOT / report_path
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    summary = report["summary"]
    print(
        "R-2011 integrated project runner: "
        f"{summary['passed']} passed, {summary['failed']} failed, {summary['project_count']} total"
    )
    print(f"Report: {report_path.relative_to(ROOT)}")

    if report["status"] != "passed":
        failed = [
            f"{project['project_id']}: {project['failure_class']} exit={project['exit_code']}"
            for project in results
            if project["status"] != "passed"
        ]
        fail("integrated project runner found failures", failed)


if __name__ == "__main__":
    main()
