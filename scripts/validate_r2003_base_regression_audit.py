#!/usr/bin/env python3
"""Validate the R-2003 base language/std regression audit gate."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
REPORT_SCHEMA = "spectralang.r2003_base_regression_audit.v1"


@dataclass(frozen=True)
class Case:
    category: str
    path: str
    command: str
    expected_exit: int
    reason: str


COMPILE_ONLY_CASES = [
    Case(
        "compile_only",
        "tests/validation/55_stdlib_comprehensive.spectra",
        "check",
        0,
        "intentional nonzero runtime return; validates trait/default std surface at compile time",
    ),
]


RUNTIME_ZERO_CASES = [
    Case(
        "base_language",
        "tests/validation/60_pattern_control_surface.spectra",
        "run",
        0,
        "existing struct-variant pattern control-flow regression must execute",
    ),
    Case(
        "base_language",
        "tests/validation/110_match_if_while_let_binding_stress.spectra",
        "run",
        0,
        "existing tuple-variant while-let/if-let regression must execute",
    ),
    Case(
        "base_language",
        "tests/validation/120_stable_promoted_control_flow.spectra",
        "run",
        0,
        "stable switch/unless/do-while/loop constructs must execute without feature gates",
    ),
    Case(
        "base_language",
        "tests/validation/140_base_enum_tuple_while_let_runtime.spectra",
        "run",
        0,
        "distilled tuple enum while-let runtime regression",
    ),
    Case(
        "base_language",
        "tests/validation/141_base_enum_struct_while_let_runtime.spectra",
        "run",
        0,
        "distilled struct enum while-let runtime regression",
    ),
    Case(
        "base_language",
        "tests/validation/142_base_pattern_match_string_runtime.spectra",
        "run",
        0,
        "string literal pattern matching must compare values through runtime execution",
    ),
    Case(
        "base_language",
        "tests/validation/143_base_loop_break_continue_runtime.spectra",
        "run",
        0,
        "nested loop break/continue and mutable bindings must execute",
    ),
    Case(
        "std_tensor",
        "tests/validation/68_tensor_phase4_kernels.spectra",
        "run",
        0,
        "existing tensor kernel and metrics surface must execute",
    ),
    Case(
        "std_tensor",
        "tests/validation/83_tensor_memory_planner.spectra",
        "run",
        0,
        "existing tensor lifetime and reuse metrics must execute",
    ),
    Case(
        "std_tensor",
        "tests/validation/144_std_tensor_materialization_perf_guard.spectra",
        "run",
        0,
        "new tensor materialization and buffer reuse guard must execute",
    ),
]


def command_text(command: list[str]) -> str:
    return " ".join(command)


def run_case(binary: Path, case: Case, timeout_seconds: int) -> dict[str, Any]:
    path = ROOT / case.path
    if not path.exists():
        return {
            "category": case.category,
            "path": case.path,
            "command": case.command,
            "expected_exit": case.expected_exit,
            "status": "missing",
            "exit_code": None,
            "reason": case.reason,
            "output_tail": f"missing file: {case.path}",
        }

    command = [str(binary), case.command, case.path]
    print(f"[R-2003] {case.category}: {command_text(command)}")
    try:
        completed = subprocess.run(
            command,
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=timeout_seconds,
            check=False,
        )
        output = completed.stdout or ""
        status = "passed" if completed.returncode == case.expected_exit else "failed"
        return {
            "category": case.category,
            "path": case.path,
            "command": case.command,
            "expected_exit": case.expected_exit,
            "status": status,
            "exit_code": completed.returncode,
            "reason": case.reason,
            "output_tail": "\n".join(output.splitlines()[-30:]),
        }
    except subprocess.TimeoutExpired as exc:
        output = exc.stdout if isinstance(exc.stdout, str) else ""
        return {
            "category": case.category,
            "path": case.path,
            "command": case.command,
            "expected_exit": case.expected_exit,
            "status": "timeout",
            "exit_code": None,
            "reason": case.reason,
            "output_tail": "\n".join(output.splitlines()[-30:]) if output else str(exc),
        }


def build_report(results: list[dict[str, Any]]) -> dict[str, Any]:
    passed = all(result["status"] == "passed" for result in results)
    return {
        "schema": REPORT_SCHEMA,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "status": "passed" if passed else "failed",
        "passed": passed,
        "compile_only_cases": [case.path for case in COMPILE_ONLY_CASES],
        "runtime_zero_cases": [case.path for case in RUNTIME_ZERO_CASES],
        "results": results,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--binary",
        default="target/debug/spectralang.exe",
        help="spectralang binary to execute",
    )
    parser.add_argument(
        "--report",
        default="target/r2003-base-regression-audit/report.json",
        help="path to write the audit report JSON",
    )
    parser.add_argument("--timeout-seconds", type=int, default=30)
    args = parser.parse_args()

    binary = (ROOT / args.binary).resolve() if not Path(args.binary).is_absolute() else Path(args.binary)
    if not binary.exists():
        print(f"R-2003 failure: binary not found: {binary}", file=sys.stderr)
        return 1

    cases = COMPILE_ONLY_CASES + RUNTIME_ZERO_CASES
    results = [run_case(binary, case, args.timeout_seconds) for case in cases]
    report = build_report(results)

    report_path = ROOT / args.report
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"[R-2003] wrote {report_path}")

    failures = [result for result in results if result["status"] != "passed"]
    if failures:
        for failure in failures:
            print(
                f"R-2003 failure: {failure['path']} expected exit {failure['expected_exit']} "
                f"but got {failure['status']} ({failure['exit_code']})",
                file=sys.stderr,
            )
            tail = failure.get("output_tail") or ""
            if tail:
                print(tail, file=sys.stderr)
        return 1

    print(f"R-2003 base regression audit ok: {len(results)} cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
