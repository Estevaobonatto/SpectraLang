#!/usr/bin/env python3
"""Validate R-110 cross-module type, enum, and method resolution."""

from __future__ import annotations

import argparse
import subprocess
from pathlib import Path


EXPECTED_LINES = [
    "4",
    "15",
    "sku-001",
    "Widget",
    "42",
    "1",
    "0",
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
    valid_project = root / "tests" / "projects" / "valid" / "cross_module_types_methods"
    invalid_project = root / "tests" / "projects" / "invalid" / "cross_module_missing_method"

    valid = run_command(binary, root, "run", str(valid_project))
    if valid.returncode != 0:
        raise AssertionError(
            f"R-110 valid project failed with exit code {valid.returncode}:\n{valid.stdout}"
        )

    observed = [line.rstrip() for line in valid.stdout.splitlines() if line.strip()]
    if observed != EXPECTED_LINES:
        raise AssertionError(
            "R-110 stdout mismatch.\n"
            f"Expected:\n{chr(10).join(EXPECTED_LINES)}\n\n"
            f"Observed:\n{chr(10).join(observed)}\n"
        )

    invalid = run_command(binary, root, "compile", str(invalid_project))
    if invalid.returncode == 0:
        raise AssertionError("R-110 invalid project compiled successfully; expected method error")

    required_fragments = [
        "Method 'reset' not found for type 'Counter'",
        "candidate impl blocks in scope",
        "read",
    ]
    missing = [fragment for fragment in required_fragments if fragment not in invalid.stdout]
    if missing:
        raise AssertionError(
            "R-110 missing-method diagnostic did not include required fragments "
            f"{missing}.\nOutput:\n{invalid.stdout}"
        )

    print("validated R-110 cross-module type and method resolution")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
