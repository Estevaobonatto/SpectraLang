#!/usr/bin/env python3
"""Validate R-105 diagnostics standardization artifacts."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def count_diagnostic_rows(markdown: str) -> int:
    count = 0
    for line in markdown.splitlines():
        stripped = line.strip()
        if not stripped.startswith("| `"):
            continue
        cells = [cell.strip() for cell in stripped.strip("|").split("|")]
        if len(cells) >= 4 and cells[0].startswith("`") and cells[3]:
            count += 1
    return count


def validate_json(path: Path) -> list[str]:
    errors: list[str] = []
    payload = json.loads(path.read_text(encoding="utf-8-sig"))
    if payload.get("version") != 1:
        errors.append("JSON diagnostics version must be 1")
    if "success" not in payload:
        errors.append("JSON diagnostics must include success")
    files = payload.get("files")
    if not isinstance(files, list) or not files:
        errors.append("JSON diagnostics must include at least one file")
        return errors
    diagnostics = files[0].get("diagnostics", [])
    if not diagnostics:
        errors.append("JSON diagnostics must include diagnostics")
        return errors
    first = diagnostics[0]
    for field in ("severity", "code", "message", "phase", "range", "related"):
        if field not in first:
            errors.append(f"JSON diagnostic missing field: {field}")
    return errors


def validate_sarif(path: Path) -> list[str]:
    errors: list[str] = []
    payload = json.loads(path.read_text(encoding="utf-8-sig"))
    if payload.get("version") != "2.1.0":
        errors.append("SARIF version must be 2.1.0")
    runs = payload.get("runs")
    if not isinstance(runs, list) or not runs:
        errors.append("SARIF must include at least one run")
        return errors
    run = runs[0]
    driver = run.get("tool", {}).get("driver", {})
    if driver.get("name") != "SpectraLang":
        errors.append("SARIF driver name must be SpectraLang")
    if not driver.get("rules"):
        errors.append("SARIF driver must include rules")
    results = run.get("results")
    if not isinstance(results, list) or not results:
        errors.append("SARIF must include results")
        return errors
    first = results[0]
    for field in ("ruleId", "level", "message", "locations"):
        if field not in first:
            errors.append(f"SARIF result missing field: {field}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=".")
    parser.add_argument("--json-report")
    parser.add_argument("--sarif-report")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    reference = root / "docs" / "diagnostics" / "error-code-reference.md"
    errors: list[str] = []

    if not reference.is_file():
        errors.append(f"missing diagnostic reference: {reference}")
    else:
        text = reference.read_text(encoding="utf-8")
        row_count = count_diagnostic_rows(text)
        if row_count < 20:
            errors.append(f"expected at least 20 high-frequency diagnostics, found {row_count}")
        for phrase in (
            "Machine-Readable JSON Diagnostics",
            "Machine-Readable SARIF Diagnostics",
            "Expected hint/action",
        ):
            if phrase not in text:
                errors.append(f"diagnostic reference missing phrase: {phrase}")

    if args.json_report:
        errors.extend(validate_json(Path(args.json_report)))
    if args.sarif_report:
        errors.extend(validate_sarif(Path(args.sarif_report)))

    if errors:
        for error in errors:
            print(f"ERROR: {error}")
        return 1

    print("validated R-105 diagnostics standardization")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
