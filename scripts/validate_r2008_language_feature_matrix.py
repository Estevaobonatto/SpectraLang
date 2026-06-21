from __future__ import annotations

import json
import sys
import tomllib
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MATRIX_PATH = ROOT / "docs" / "architecture" / "r2008-language-feature-project-matrix.toml"
ROADMAP_PATH = ROOT / "roadmap" / "roadmap.toml"
BACKLOG_PATH = ROOT / "docs" / "roadmap-backlog.md"
PLAN_PATH = ROOT / "docs" / "production-ai-implementation-plan.md"
REPORT_PATH = ROOT / "target" / "r2008-language-feature-project-matrix" / "report.json"

EXPECTED_SCHEMA = "spectralang.r2008_language_feature_project_matrix.v1"
EXPECTED_ROADMAP_ITEM = "R-2008"
ALLOWED_STATUSES = {"planned"}
ALLOWED_OWNERS = {
    "frontend",
    "semantic",
    "midend",
    "backend",
    "runtime",
    "numerics",
    "ml",
    "web",
    "db",
    "tooling",
    "ecosystem",
}


def load_toml(path: Path) -> dict:
    with path.open("rb") as handle:
        return tomllib.load(handle)


def fail(message: str, details: list[str] | None = None) -> None:
    print(f"R-2008 validation failed: {message}", file=sys.stderr)
    for detail in details or []:
        print(f"- {detail}", file=sys.stderr)
    raise SystemExit(1)


def assert_non_empty_string(project_id: str, project: dict, field: str, errors: list[str]) -> None:
    value = project.get(field)
    if not isinstance(value, str) or not value.strip():
        errors.append(f"{project_id}: field {field!r} must be a non-empty string")


def roadmap_items(roadmap: dict) -> dict[str, dict]:
    items = roadmap.get("items")
    if not isinstance(items, list):
        fail("roadmap.toml must contain an [[items]] array")
    by_id: dict[str, dict] = {}
    duplicates: list[str] = []
    for item in items:
        item_id = item.get("id")
        if not isinstance(item_id, str):
            continue
        if item_id in by_id:
            duplicates.append(item_id)
        by_id[item_id] = item
    if duplicates:
        fail("roadmap.toml contains duplicate item IDs", sorted(set(duplicates)))
    return by_id


def validate_matrix(matrix: dict, roadmap_by_id: dict[str, dict]) -> dict:
    errors: list[str] = []

    if matrix.get("schema") != EXPECTED_SCHEMA:
        errors.append(f"schema must be {EXPECTED_SCHEMA!r}")
    if matrix.get("roadmap_item") != EXPECTED_ROADMAP_ITEM:
        errors.append("roadmap_item must be 'R-2008'")
    if matrix.get("version") != 1:
        errors.append("version must be 1")
    if matrix.get("status") != "complete":
        errors.append("matrix status must be 'complete'")

    required_features = matrix.get("required_features")
    allowed_commands = matrix.get("allowed_commands")
    gap_items = matrix.get("gap_roadmap_items")
    projects = matrix.get("projects")

    if not isinstance(required_features, list) or not all(isinstance(value, str) for value in required_features):
        errors.append("required_features must be a string array")
        required_features = []
    if not isinstance(allowed_commands, list) or not all(isinstance(value, str) for value in allowed_commands):
        errors.append("allowed_commands must be a string array")
        allowed_commands = []
    if not isinstance(gap_items, list) or not all(isinstance(value, str) for value in gap_items):
        errors.append("gap_roadmap_items must be a string array")
        gap_items = []
    if not isinstance(projects, list) or not projects:
        errors.append("projects must be a non-empty array")
        projects = []

    expected_gap_items = {"R-2009", "R-2010", "R-2011", "R-2012", "R-2013"}
    missing_gap_items = expected_gap_items - set(gap_items)
    if missing_gap_items:
        errors.append(f"gap_roadmap_items missing {sorted(missing_gap_items)}")

    project_ids: list[str] = []
    command_counter: Counter[str] = Counter()
    feature_counter: Counter[str] = Counter()
    roadmap_counter: Counter[str] = Counter()

    for index, project in enumerate(projects):
        project_id = project.get("id")
        if not isinstance(project_id, str) or not project_id.strip():
            project_id = f"<project #{index + 1}>"
            errors.append(f"{project_id}: field 'id' must be a non-empty string")
        project_ids.append(project_id)

        for field in ("title", "category", "roadmap_item", "owner", "command", "project_path", "status", "expected_outcome"):
            assert_non_empty_string(project_id, project, field, errors)

        owner = project.get("owner")
        if isinstance(owner, str) and owner not in ALLOWED_OWNERS:
            errors.append(f"{project_id}: owner {owner!r} is not a known owner group")

        status = project.get("status")
        if isinstance(status, str) and status not in ALLOWED_STATUSES:
            errors.append(f"{project_id}: status {status!r} must be one of {sorted(ALLOWED_STATUSES)}")

        command = project.get("command")
        if isinstance(command, str):
            command_counter[command] += 1
            if command not in allowed_commands:
                errors.append(f"{project_id}: command {command!r} is not declared in allowed_commands")

        roadmap_item = project.get("roadmap_item")
        if isinstance(roadmap_item, str):
            roadmap_counter[roadmap_item] += 1
            if roadmap_item not in roadmap_by_id:
                errors.append(f"{project_id}: roadmap_item {roadmap_item!r} does not exist in roadmap.toml")
            if roadmap_item not in {"R-2009", "R-2010"}:
                errors.append(f"{project_id}: roadmap_item must be R-2009 or R-2010")

        project_path = project.get("project_path")
        if isinstance(project_path, str) and not project_path.startswith("tests/projects/valid/integrated_"):
            errors.append(f"{project_id}: project_path must be under tests/projects/valid/integrated_*")

        features = project.get("features")
        if not isinstance(features, list) or not features or not all(isinstance(value, str) for value in features):
            errors.append(f"{project_id}: features must be a non-empty string array")
            continue
        unknown_features = set(features) - set(required_features)
        if unknown_features:
            errors.append(f"{project_id}: unknown features {sorted(unknown_features)}")
        feature_counter.update(features)

    duplicate_ids = sorted(project_id for project_id, count in Counter(project_ids).items() if count > 1)
    if duplicate_ids:
        errors.append(f"duplicate project ids: {duplicate_ids}")

    missing_features = sorted(set(required_features) - set(feature_counter))
    if missing_features:
        errors.append(f"required features without project coverage: {missing_features}")

    missing_commands = sorted(set(allowed_commands) - set(command_counter))
    if missing_commands:
        errors.append(f"allowed commands without project coverage: {missing_commands}")

    for required_item in ("R-2009", "R-2010"):
        if roadmap_counter[required_item] == 0:
            errors.append(f"no project maps to {required_item}")

    roadmap_r2008 = roadmap_by_id.get(EXPECTED_ROADMAP_ITEM)
    if not roadmap_r2008:
        errors.append("R-2008 missing from roadmap.toml")
    elif roadmap_r2008.get("status") != "complete":
        errors.append("R-2008 roadmap status must be 'complete'")

    for required_item in expected_gap_items:
        if required_item not in roadmap_by_id:
            errors.append(f"{required_item} missing from roadmap.toml")

    if errors:
        fail("matrix contract is not satisfied", errors)

    return {
        "schema": matrix["schema"],
        "roadmap_item": matrix["roadmap_item"],
        "version": matrix["version"],
        "project_count": len(projects),
        "required_feature_count": len(required_features),
        "covered_features": sorted(feature_counter),
        "commands": dict(sorted(command_counter.items())),
        "roadmap_targets": dict(sorted(roadmap_counter.items())),
        "gap_roadmap_items": gap_items,
        "status": "pass",
    }


def validate_docs(report: dict) -> None:
    docs = {
        "backlog": BACKLOG_PATH.read_text(encoding="utf-8"),
        "plan": PLAN_PATH.read_text(encoding="utf-8"),
    }
    required_tokens = [
        "R-2008",
        "r2008-language-feature-project-matrix.toml",
        "validate_r2008_language_feature_matrix.py",
        "R-2009",
        "R-2010",
        "R-2011",
        "R-2012",
        "R-2013",
    ]
    errors: list[str] = []
    for name, text in docs.items():
        for token in required_tokens:
            if token not in text:
                errors.append(f"{name} is missing {token}")
    if errors:
        fail("planning docs are not synchronized with R-2008", errors)

    doc_features = set(report["covered_features"])
    aliases = {
        "structs_classes": ["structs/classes", "structs-classes"],
        "graph_fusion": ["graph/fusion", "graph-fusion"],
    }
    for feature in doc_features:
        candidates = [feature, feature.replace("_", "-"), *aliases.get(feature, [])]
        if not any(candidate in docs["backlog"] for candidate in candidates):
            errors.append(f"backlog does not mention covered feature {feature}")
    if errors:
        fail("backlog feature coverage is incomplete", errors)


def main() -> None:
    if not MATRIX_PATH.exists():
        fail(f"matrix file does not exist: {MATRIX_PATH.relative_to(ROOT)}")

    matrix = load_toml(MATRIX_PATH)
    roadmap = load_toml(ROADMAP_PATH)
    report = validate_matrix(matrix, roadmap_items(roadmap))
    validate_docs(report)

    REPORT_PATH.parent.mkdir(parents=True, exist_ok=True)
    REPORT_PATH.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(
        "R-2008 language feature project matrix OK: "
        f"{report['project_count']} projects, {report['required_feature_count']} required features"
    )
    print(f"Report: {REPORT_PATH.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
