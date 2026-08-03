"""Run and certify the R-2013 integrated release-candidate gate."""

from __future__ import annotations

import argparse
import json
import os
import shlex
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
MATRIX_PATH = ROOT / "docs" / "architecture" / "r2008-language-feature-project-matrix.toml"
REPORT_PATH = ROOT / "target" / "r2013-release-candidate" / "report.json"
R2001_REPORT = ROOT / "target" / "r2001-conformance" / "conformance-report.json"
R2011_REPORT = ROOT / "target" / "r2011-integrated-project-runner" / "report.json"
R2012_REPORT = ROOT / "target" / "r2012-failure-triage" / "report.json"
SCHEMA = "spectralang.r2013_release_candidate_gate.v1"
MATRIX_SCHEMA = "spectralang.r2008_language_feature_project_matrix.v1"
MATRIX_VERSION = 1
ALLOWED_COMMANDS = {"spectralang run", "spectralang package check", "spectralang package test"}
REQUIRED_PROJECT_FIELDS = {
    "project_id",
    "project_path",
    "entrypoint",
    "category",
    "command",
    "exact_command",
    "status",
    "failure_class",
    "exit_code",
    "output_tail",
}


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot load report {path}: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"report {path} must contain a JSON object")
    return value


def load_toml(path: Path) -> dict[str, Any]:
    import tomllib

    try:
        with path.open("rb") as handle:
            return tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise ValueError(f"cannot load matrix {path}: {error}") from error


def git_revision() -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    return result.stdout.strip() if result.returncode == 0 else "unknown"


def resolve_binary(raw_binary: str | None) -> str:
    if raw_binary:
        candidate = Path(raw_binary)
        if not candidate.is_absolute():
            candidate = ROOT / candidate
        if not candidate.is_file():
            raise ValueError(f"--binary does not exist: {raw_binary}")
        return str(candidate.resolve())

    candidate = ROOT / "target" / "debug" / ("spectralang.exe" if os.name == "nt" else "spectralang")
    if candidate.is_file():
        return str(candidate)
    from shutil import which

    path_binary = which("spectralang")
    if path_binary:
        return path_binary
    raise ValueError("cannot resolve spectralang binary")


def validate_matrix(matrix: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if matrix.get("schema") != MATRIX_SCHEMA:
        errors.append(f"matrix schema must be {MATRIX_SCHEMA}")
    if matrix.get("status") != "complete":
        errors.append("matrix status must be complete")
    if matrix.get("version") != MATRIX_VERSION:
        errors.append(f"matrix version must be {MATRIX_VERSION}")
    projects = matrix.get("projects")
    if not isinstance(projects, list) or not projects:
        errors.append("matrix must contain projects")
        return errors

    seen: set[str] = set()
    for project in projects:
        if not isinstance(project, dict):
            errors.append("matrix project must be an object")
            continue
        project_id = project.get("id", "<missing id>")
        if project_id in seen:
            errors.append(f"duplicate matrix project: {project_id}")
        seen.add(project_id)
        for field in ("id", "project_path", "entrypoint", "exact_command", "expected_outcome"):
            if not isinstance(project.get(field), str) or not project[field].strip():
                errors.append(f"{project_id}: missing {field}")
        command = project.get("command")
        if command not in ALLOWED_COMMANDS:
            errors.append(f"{project_id}: command is not allowed: {command!r}")
        exact_command = project.get("exact_command", "")
        try:
            tokens = shlex.split(exact_command)
        except ValueError as error:
            errors.append(f"{project_id}: invalid exact_command: {error}")
            continue
        if not tokens or tokens[0] != "spectralang":
            errors.append(f"{project_id}: exact_command must start with spectralang")
        project_root = ROOT / str(project.get("project_path", ""))
        if not project_root.is_dir():
            errors.append(f"{project_id}: missing project directory {project_root}")
        required_files = project.get("required_files", [])
        if not isinstance(required_files, list):
            errors.append(f"{project_id}: required_files must be an array")
        else:
            for required in required_files:
                if not isinstance(required, str) or not (project_root / required).is_file():
                    errors.append(f"{project_id}: missing required file {required}")
        entrypoint = project.get("entrypoint")
        if isinstance(entrypoint, str) and not (project_root / entrypoint).is_file():
            errors.append(f"{project_id}: missing entrypoint {entrypoint}")
    return errors


def validate_conformance_report(report: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if report.get("schema") != "spectralang.ai_conformance_report.v1":
        errors.append("R-2001 report has invalid schema")
    if report.get("conformance_version") != "R-2001/v1":
        errors.append("R-2001 report has invalid conformance version")
    if report.get("certified") is not True or report.get("candidate_status") != "certified":
        errors.append("R-2001 conformance is not certified")
    return errors


def validate_runner_report(report: dict[str, Any], matrix: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if report.get("schema") != "spectralang.r2011_integrated_project_runner.v1":
        errors.append("R-2011 report has invalid schema")
    if report.get("roadmap_item") != "R-2011":
        errors.append("R-2011 report has invalid roadmap item")
    if report.get("matrix_schema") != MATRIX_SCHEMA or report.get("matrix_version") != matrix.get("version"):
        errors.append("R-2011 report does not match the current matrix")
    projects = report.get("projects")
    if not isinstance(projects, list) or not projects:
        errors.append("R-2011 report has no project results")
        return errors
    expected_ids = {project.get("id") for project in matrix.get("projects", []) if isinstance(project, dict)}
    actual_ids = {project.get("project_id") for project in projects if isinstance(project, dict)}
    if actual_ids != expected_ids:
        errors.append("R-2011 project set does not match the current matrix")
    for project in projects:
        if not isinstance(project, dict):
            errors.append("R-2011 project result must be an object")
            continue
        missing = sorted(REQUIRED_PROJECT_FIELDS - set(project))
        if missing:
            errors.append(f"{project.get('project_id', '<unknown>')}: missing fields {missing}")
        if project.get("status") not in {"passed", "failed"}:
            errors.append(f"{project.get('project_id', '<unknown>')}: invalid status")
        if project.get("status") == "failed" and not str(project.get("output_tail", "")).strip():
            errors.append(f"{project.get('project_id', '<unknown>')}: failed result lacks output tail")
    if report.get("status") != "passed" or report.get("summary", {}).get("failed") != 0:
        errors.append("R-2011 contains failed integrated projects")
    return errors


def validate_triage_report(report: dict[str, Any], runner_report: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if report.get("schema") != "spectralang.r2012_failure_triage.v1":
        errors.append("R-2012 report has invalid schema")
    if report.get("roadmap_item") != "R-2012":
        errors.append("R-2012 report has invalid roadmap item")
    if report.get("runner_report") != "target/r2011-integrated-project-runner/report.json":
        errors.append("R-2012 report references an unexpected runner report")
    if report.get("summary", {}).get("untracked_failures") != 0:
        errors.append("R-2012 contains untracked failures")
    if report.get("status") != "passed":
        errors.append("R-2012 triage did not pass")
    runner_projects = runner_report.get("projects", [])
    failed_count = sum(project.get("status") == "failed" for project in runner_projects if isinstance(project, dict))
    if report.get("summary", {}).get("failed_projects") != failed_count:
        errors.append("R-2012 failed-project count disagrees with R-2011")
    return errors


def run_validator(args: list[str], timeout: int) -> int:
    try:
        result = subprocess.run(
            [sys.executable, *args],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        output = error.stdout if isinstance(error.stdout, str) else ""
        print(output, end="")
        print(f"[R-2013] validator timeout after {timeout}s: {' '.join(args)}", file=sys.stderr)
        return 124
    print(result.stdout, end="")
    return result.returncode


def read_or_error(path: Path, label: str, errors: list[str]) -> dict[str, Any]:
    try:
        return load_json(path)
    except ValueError as error:
        errors.append(f"{label}: {error}")
        return {}


def build_report(
    release_candidate: str,
    matrix: dict[str, Any],
    conformance: dict[str, Any],
    runner: dict[str, Any],
    triage: dict[str, Any],
    errors: list[str],
) -> dict[str, Any]:
    projects = [
        {
            key: project.get(key)
            for key in (
                "project_id",
                "project_path",
                "category",
                "exact_command",
                "entrypoint",
                "status",
                "exit_code",
                "failure_class",
                "output_tail",
            )
        }
        for project in runner.get("projects", [])
        if isinstance(project, dict)
    ]
    triage_records = [
        record
        for record in triage.get("triage", [])
        if isinstance(record, dict) and record.get("tracked_by")
    ]
    failed = sum(project.get("status") != "passed" for project in projects)
    summary = {
        "conformance_certified": conformance.get("certified") is True,
        "project_count": len(projects),
        "projects_passed": len(projects) - failed,
        "projects_failed": failed,
        "untracked_failures": triage.get("summary", {}).get("untracked_failures"),
    }
    if failed or summary["untracked_failures"] != 0:
        errors.append("release candidate requires zero failed projects and zero untracked failures")
    status = "passed" if not errors else "failed"
    return {
        "schema": SCHEMA,
        "roadmap_item": "R-2013",
        "release_candidate": release_candidate,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "git_revision": git_revision(),
        "status": status,
        "matrix_path": str(MATRIX_PATH.relative_to(ROOT)).replace("\\", "/"),
        "matrix_schema": matrix.get("schema"),
        "matrix_version": matrix.get("version"),
        "reports": {
            "r2001": str(R2001_REPORT.relative_to(ROOT)).replace("\\", "/"),
            "r2011": str(R2011_REPORT.relative_to(ROOT)).replace("\\", "/"),
            "r2012": str(R2012_REPORT.relative_to(ROOT)).replace("\\", "/"),
        },
        "summary": summary,
        "projects": projects,
        "follow_up_roadmap_items": triage_records,
        "validation_errors": errors,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Run the R-2013 release-candidate integrated project gate.")
    parser.add_argument("--binary", help="Path to spectralang executable.")
    parser.add_argument("--release-candidate", default="local-working-tree")
    parser.add_argument("--report", default=str(REPORT_PATH))
    parser.add_argument("--stage-timeout", type=int, default=900)
    args = parser.parse_args()
    report_path = Path(args.report)
    if not report_path.is_absolute():
        report_path = ROOT / report_path
    errors: list[str] = []
    try:
        binary = resolve_binary(args.binary)
        matrix = load_toml(MATRIX_PATH)
        errors.extend(validate_matrix(matrix))
    except ValueError as error:
        errors.append(str(error))
        binary = ""
        matrix = {}

    if not errors:
        for predecessor_report in (R2001_REPORT, R2011_REPORT, R2012_REPORT):
            predecessor_report.unlink(missing_ok=True)
        run_validator(
            [
                "scripts/validate_r2001_ai_conformance.py",
                "--keep-going",
                "--binary",
                binary,
            ],
            args.stage_timeout,
        )
        run_validator(
            ["scripts/validate_r2011_integrated_project_runner.py", "--binary", binary],
            args.stage_timeout,
        )
        run_validator(["scripts/validate_r2012_failure_triage.py"], args.stage_timeout)

    conformance = read_or_error(R2001_REPORT, "R-2001", errors)
    runner = read_or_error(R2011_REPORT, "R-2011", errors)
    triage = read_or_error(R2012_REPORT, "R-2012", errors)
    if conformance:
        errors.extend(validate_conformance_report(conformance))
    if runner:
        errors.extend(validate_runner_report(runner, matrix))
    if triage:
        errors.extend(validate_triage_report(triage, runner))
    report = build_report(args.release_candidate, matrix, conformance, runner, triage, errors)
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"[R-2013] report: {report_path}")
    print(f"[R-2013] status: {report['status']}")
    if errors:
        for error in errors:
            print(f"[R-2013] ERROR: {error}", file=sys.stderr)
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
