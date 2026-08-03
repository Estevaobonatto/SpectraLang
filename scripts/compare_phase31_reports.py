#!/usr/bin/env python3
"""Compare semantic Phase 31 evidence while ignoring timestamps and durations."""

from __future__ import annotations

import argparse
import json
import pathlib
import sys

try:
    from scripts.phase31_contract import SCENARIOS
except ModuleNotFoundError:  # direct script execution
    from phase31_contract import SCENARIOS  # type: ignore[no-redef]


SEMANTIC_SCENARIO_FIELDS = ("id", "category", "iterations", "correctness_passed")
SEMANTIC_RESULT_FIELDS = ("command", "exit_code", "failure_class", "error")


def semantic_report(report: dict) -> dict:
    scenarios = []
    for scenario in report.get("scenarios", []):
        item = {field: scenario.get(field) for field in SEMANTIC_SCENARIO_FIELDS}
        item["results"] = {}
        for language, result in sorted(scenario.get("results", {}).items()):
            item["results"][language] = {
                field: result.get(field) for field in SEMANTIC_RESULT_FIELDS
            }
        scenarios.append(item)
    return {
        "schema": report.get("schema"),
        "profile": report.get("profile"),
        "spectra_binary": report.get("spectra_binary"),
        "git_revision": report.get("git_revision"),
        "measurement_policy": report.get("measurement_policy"),
        "scenario_ids": [scenario.get("id") for scenario in report.get("scenarios", [])],
        "scenarios": scenarios,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("left")
    parser.add_argument("right")
    args = parser.parse_args()
    left = json.loads(pathlib.Path(args.left).read_text(encoding="utf-8"))
    right = json.loads(pathlib.Path(args.right).read_text(encoding="utf-8"))
    left_semantic = semantic_report(left)
    right_semantic = semantic_report(right)
    if left_semantic["scenario_ids"] != list(SCENARIOS) or right_semantic["scenario_ids"] != list(SCENARIOS):
        print("FAIL: reports do not contain the canonical 21-scenario order", file=sys.stderr)
        return 1
    if left_semantic != right_semantic:
        print("FAIL: semantic Phase 31 evidence differs", file=sys.stderr)
        return 1
    print("PASS: semantic Phase 31 evidence matches")
    return 0


if __name__ == "__main__":
    sys.exit(main())
