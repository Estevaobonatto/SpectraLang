#!/usr/bin/env python3
"""Run Phase 13 AI examples and emit a JSON timing report."""

from __future__ import annotations

import argparse
import json
import subprocess
import time
from pathlib import Path


def run_example(root: Path, example: Path, timeout_seconds: int) -> dict[str, object]:
    start = time.perf_counter()
    command = ["cargo", "run", "-q", "-p", "spectra-cli", "--", "run", str(example)]
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
    return {
        "example": example.name,
        "status": "passed" if proc.returncode == 0 else "failed",
        "exit_code": proc.returncode,
        "elapsed_ms": elapsed_ms,
        "output_tail": "\n".join(proc.stdout.splitlines()[-20:]),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=".", help="repository root")
    parser.add_argument("--out", default="target/ai-examples/benchmark.json")
    parser.add_argument("--timeout-seconds", type=int, default=20)
    args = parser.parse_args()

    root = Path(args.root).resolve()
    examples = sorted((root / "examples" / "ai").glob("*.spectra"))
    if not examples:
        print("ERROR: no AI examples found")
        return 1

    records = []
    for example in examples:
        try:
            records.append(run_example(root, example, args.timeout_seconds))
        except subprocess.TimeoutExpired as exc:
            records.append(
                {
                    "example": example.name,
                    "status": "timeout",
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
            print(f"ERROR: {failure['example']} => {failure['status']}")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
