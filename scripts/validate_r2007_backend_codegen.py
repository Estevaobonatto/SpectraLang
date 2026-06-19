#!/usr/bin/env python3
"""Validate the R-2007 backend/codegen robustness gate."""

from __future__ import annotations

import argparse
import os
import pathlib
import shutil
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
EDGE_SOURCE = ROOT / "tests" / "validation" / "146_backend_codegen_edge_control_flow.spectra"


def resolve_cargo() -> str:
    explicit = os.environ.get("CARGO")
    if explicit:
        return explicit
    cargo = shutil.which("cargo")
    if cargo:
        return cargo
    for candidate in (
        pathlib.Path.home() / ".cargo" / "bin" / "cargo.exe",
        pathlib.Path.home() / ".cargo" / "bin" / "cargo",
    ):
        if candidate.exists():
            return str(candidate)
    raise FileNotFoundError("cargo was not found in PATH, CARGO, or ~/.cargo/bin")


def run(command: list[str], *, env: dict[str, str] | None = None) -> None:
    proc = subprocess.run(
        command,
        cwd=ROOT,
        text=True,
        capture_output=True,
        env=env,
        check=False,
    )
    if proc.returncode != 0:
        sys.stderr.write(proc.stdout)
        sys.stderr.write(proc.stderr)
        raise SystemExit(proc.returncode or 1)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--binary",
        default=str(ROOT / "target" / "debug" / "spectralang.exe"),
        help="spectralang binary used for the valid-source edge codegen regression",
    )
    args = parser.parse_args()

    cargo = resolve_cargo()
    run([cargo, "test", "-p", "spectra-backend", "r2007_", "--", "--test-threads=1"])

    env = os.environ.copy()
    env["RUSTFLAGS"] = "-Dwarnings"
    run([cargo, "check", "-p", "spectra-backend", "-p", "spectra-cli", "--all-targets"], env=env)

    binary = pathlib.Path(args.binary)
    if not binary.exists():
        run([cargo, "build", "-p", "spectra-cli"])
    run([str(binary), "run", str(EDGE_SOURCE)])

    print("R-2007 backend/codegen robustness ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
