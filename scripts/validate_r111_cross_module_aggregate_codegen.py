#!/usr/bin/env python3
"""Validate R-111 cross-module aggregate codegen regressions."""

from __future__ import annotations

import argparse
import subprocess
from pathlib import Path


EXPECTED_LINES = [
    "alpha|beta|gamma",
    "10",
    "7",
    "-4",
    "11",
    "-6",
]


def run_command(binary: Path, root: Path, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [str(binary), *args],
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=10,
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--root", default=".")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    binary = Path(args.binary).resolve()
    project = root / "tests" / "projects" / "valid" / "cross_module_aggregate_codegen"

    run = run_command(binary, root, "run", str(project))
    if run.returncode != 0:
        raise AssertionError(
            f"R-111 project failed with exit code {run.returncode}:\n{run.stdout}"
        )
    if "Verifier errors" in run.stdout or "Value " in run.stdout and "not found" in run.stdout:
        raise AssertionError(f"R-111 run produced backend internal error:\n{run.stdout}")

    observed = [line.rstrip() for line in run.stdout.splitlines() if line.strip()]
    if observed != EXPECTED_LINES:
        raise AssertionError(
            "R-111 stdout mismatch.\n"
            f"Expected:\n{chr(10).join(EXPECTED_LINES)}\n\n"
            f"Observed:\n{chr(10).join(observed)}\n"
        )

    dump = run_command(binary, root, "compile", str(project), "--dump-ir")
    if dump.returncode != 0:
        raise AssertionError(
            f"R-111 --dump-ir compile failed with exit code {dump.returncode}:\n{dump.stdout}"
        )
    forbidden = ["load(void)", "Verifier errors", "Value "]
    for marker in forbidden:
        if marker == "Value ":
            if "Value " in dump.stdout and "not found" in dump.stdout:
                raise AssertionError(f"R-111 IR dump contains value lookup failure:\n{dump.stdout}")
        elif marker in dump.stdout:
            raise AssertionError(f"R-111 IR dump contains forbidden marker {marker!r}")

    print("validated R-111 cross-module aggregate codegen")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
