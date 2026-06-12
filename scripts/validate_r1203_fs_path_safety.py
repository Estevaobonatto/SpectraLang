#!/usr/bin/env python3
"""Validate R-1203 filesystem host call path safety."""

from __future__ import annotations

import argparse
import shutil
import subprocess
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--root", default=".")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    binary = Path(args.binary).resolve()
    test_root = root / "target" / "r1203-fs-path-safety"
    source = root / "tests" / "validation" / "111_fs_path_safety.spectra"

    if test_root.exists():
        shutil.rmtree(test_root)

    proc = subprocess.run(
        [str(binary), "run", str(source)],
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=20,
    )
    if proc.returncode != 0:
        raise AssertionError(
            f"{source} failed with exit code {proc.returncode}:\n{proc.stdout}"
        )

    artifact = test_root / "nested" / "run" / "artifact.txt"
    if not artifact.is_file():
        raise AssertionError(f"nested artifact was not created: {artifact}")
    if artifact.read_text(encoding="utf-8") != "overwrite":
        raise AssertionError(f"artifact content was not overwritten correctly: {artifact}")

    blocked_child = test_root / "blocker" / "child.txt"
    if blocked_child.exists():
        raise AssertionError(f"blocked child path should not have been created: {blocked_child}")

    shutil.rmtree(test_root, ignore_errors=True)
    print("validated R-1203 filesystem host call path safety")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
