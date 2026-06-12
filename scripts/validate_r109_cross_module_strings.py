#!/usr/bin/env python3
"""Validate R-109 cross-module string materialization and string concatenation."""

from __future__ import annotations

import argparse
import subprocess
from pathlib import Path


EXPECTED_LINES = [
    "--- user module strings ---",
    "Hello, Spectra",
    "left|right",
    "HELLO",
    "loud",
    "hahaha",
    "bonono",
    "cba",
    "ok",
    "--- std.string direct strings ---",
    "HelloWorld",
    "***",
    "HELLO",
    "loud",
    "bonono",
    "cba",
    "ok",
    "--- main concat and convert strings ---",
    "Hello, World",
    "value=42",
    "bool=true",
    "after concat",
]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--root", default=".")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    binary = Path(args.binary).resolve()
    project = root / "tests" / "projects" / "valid" / "cross_module_strings"

    proc = subprocess.run(
        [str(binary), "run", str(project)],
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=10,
    )

    if proc.returncode != 0:
        raise AssertionError(
            f"R-109 project failed with exit code {proc.returncode}:\n{proc.stdout}"
        )

    observed = [line.rstrip() for line in proc.stdout.splitlines() if line.strip()]
    if observed != EXPECTED_LINES:
        expected_text = "\n".join(EXPECTED_LINES)
        observed_text = "\n".join(observed)
        raise AssertionError(
            "R-109 stdout mismatch.\n"
            f"Expected:\n{expected_text}\n\nObserved:\n{observed_text}\n"
        )

    print("validated R-109 cross-module string handling")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
