#!/usr/bin/env python3
"""Run Phase 13 AI examples and emit a JSON timing report."""

from __future__ import annotations

import argparse
import json
import subprocess
import time
from pathlib import Path


def classify_failure(exit_code: int, output: str) -> str:
    lowered = output.lower()
    if exit_code == 0:
        return "none"
    if "error[syntax]" in lowered or "error[semantic]" in lowered:
        return "compile"
    if "verifier" in lowered or "codegen" in lowered or "failed to define function" in lowered:
        return "codegen"
    if "error[runtime]" in lowered or "program exited with status" in lowered:
        return "runtime"
    if exit_code < 0 or exit_code in {3221225477, 3221226505}:
        return "crash"
    return "unknown"


def build_command(binary: str | None, example: Path) -> list[str]:
    if binary:
        return [binary, "run", str(example)]
    return ["cargo", "run", "-q", "-p", "spectra-cli", "--", "run", str(example)]


def run_example(
    root: Path, example: Path, timeout_seconds: int, binary: str | None
) -> dict[str, object]:
    start = time.perf_counter()
    command = build_command(binary, example)
    proc = subprocess.run(
        command,
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=timeout_seconds,
        check=False,
    )
    elapsed_ms = int((time.perf_counter() - start) * 1000)
    failure_kind = classify_failure(proc.returncode, proc.stdout)
    return {
        "example": example.name,
        "status": "passed" if proc.returncode == 0 else "failed",
        "failure_kind": failure_kind,
        "exit_code": proc.returncode,
        "elapsed_ms": elapsed_ms,
        "output_tail": "\n".join(proc.stdout.splitlines()[-20:]),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=".", help="repository root")
    parser.add_argument("--out", default="target/ai-examples/benchmark.json")
    parser.add_argument("--timeout-seconds", type=int, default=20)
    parser.add_argument(
        "--binary",
        default=None,
        help="optional spectralang binary path; defaults to cargo run",
    )
    args = parser.parse_args()

    root = Path(args.root).resolve()
    binary = str((root / args.binary).resolve()) if args.binary else None
    examples = sorted((root / "examples" / "ai").glob("*.spectra"))
    if not examples:
        print("ERROR: no AI examples found")
        return 1

    records = []
    for example in examples:
        try:
            records.append(run_example(root, example, args.timeout_seconds, binary))
        except subprocess.TimeoutExpired as exc:
            records.append(
                {
                    "example": example.name,
                    "status": "timeout",
                    "failure_kind": "timeout",
                    "exit_code": None,
                    "elapsed_ms": args.timeout_seconds * 1000,
                    "output_tail": str(exc),
                }
            )

    report = {
        "schema": "spectralang.ai_examples_benchmark.v1",
        "count": len(records),
        "records": records,
    }

    out = (root / args.out).resolve()
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2), encoding="utf-8")

    failures = [r for r in records if r["status"] != "passed"]
    print(f"wrote {out}")
    if failures:
        for failure in failures:
            print(
                f"ERROR: {failure['example']} => "
                f"{failure['status']} ({failure['failure_kind']})"
            )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
