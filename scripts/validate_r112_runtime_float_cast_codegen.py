#!/usr/bin/env python3
"""Validate R-112 runtime float-to-int cast codegen."""

from __future__ import annotations

import argparse
import subprocess
from pathlib import Path


RUNTIME_CASES = [
    Path("tests/validation/59_import_surface.spectra"),
    Path("tests/validation/106_import_alias_named_std_stress.spectra"),
    Path("tests/validation/67_tensor_float_surface.spectra"),
]


def run_command(binary: Path, root: Path, args: list[str], timeout: int = 10) -> str:
    proc = subprocess.run(
        [str(binary), *args],
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=timeout,
    )
    if proc.returncode != 0:
        raise AssertionError(
            f"command failed with exit code {proc.returncode}: {' '.join(args)}\n{proc.stdout}"
        )
    if "Verifier errors" in proc.stdout or "error[codegen]" in proc.stdout:
        raise AssertionError(f"backend verifier error in {' '.join(args)}:\n{proc.stdout}")
    return proc.stdout


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--root", default=".")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    binary = Path(args.binary).resolve()

    for source in RUNTIME_CASES:
        run_command(binary, root, ["run", str(root / source)])
        ir = run_command(binary, root, ["compile", "--dump-ir", str(root / source)])
        if "cast(float -> int)" not in ir:
            raise AssertionError(f"expected runtime float-to-int cast in IR for {source}")

    print("validated R-112 runtime float-to-int cast codegen")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
