#!/usr/bin/env python3
"""Validate R-2005 core std/runtime host-status hardening."""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def cargo_bin() -> str:
    explicit = os.environ.get("CARGO")
    if explicit:
        return explicit
    discovered = shutil.which("cargo")
    if discovered:
        return discovered
    windows_default = Path.home() / ".cargo" / "bin" / "cargo.exe"
    if windows_default.exists():
        return str(windows_default)
    return "cargo"


def run_checked(command: list[str], *, timeout: int = 180) -> str:
    proc = subprocess.run(
        command,
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=timeout,
    )
    if proc.returncode != 0:
        sys.stdout.write(proc.stdout)
        raise SystemExit(proc.returncode)
    return proc.stdout


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", default=str(ROOT / "target" / "debug" / "spectralang.exe"))
    args = parser.parse_args()

    binary = Path(args.binary)
    if not binary.exists():
        raise SystemExit(f"spectralang binary not found: {binary}")

    rust_output = run_checked(
        [
            cargo_bin(),
            "test",
            "-p",
            "spectra-runtime",
            "r2005_",
            "--",
            "--test-threads=1",
        ],
        timeout=240,
    )
    if "panicked at" in rust_output:
        sys.stdout.write(rust_output)
        raise SystemExit("R-2005 Rust hardening tests emitted a panic")

    spectra_case = ROOT / "tests" / "validation" / "145_runtime_host_status_hardening.spectra"
    run_checked([str(binary), "run", str(spectra_case)], timeout=120)

    print("validated R-2005 runtime host-status hardening")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
