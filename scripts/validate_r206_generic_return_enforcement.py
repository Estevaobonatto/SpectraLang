#!/usr/bin/env python3
"""Validate R-206 generic return type enforcement."""

from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path
from typing import Any


def extract_json_payload(output: str) -> dict[str, Any]:
    start = output.find("{")
    if start < 0:
        raise AssertionError(f"compiler output did not contain JSON:\n{output}")
    return json.loads(output[start:])


def run_check(binary: Path, root: Path, source: Path) -> dict[str, Any]:
    proc = subprocess.run(
        [str(binary), "check", "--json", str(source)],
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=20,
    )
    if proc.returncode == 0:
        raise AssertionError(f"{source} unexpectedly passed:\n{proc.stdout}")
    if "Verifier errors" in proc.stdout or '"phase":"backend"' in proc.stdout:
        raise AssertionError(f"{source} reached backend instead of semantic analysis:\n{proc.stdout}")
    return extract_json_payload(proc.stdout)


def require_generic_return_mismatch(
    payload: dict[str, Any],
    *,
    expected: str,
) -> None:
    files = payload.get("files")
    if not isinstance(files, list) or not files:
        raise AssertionError(f"missing files in payload: {payload}")
    diagnostics = files[0].get("diagnostics")
    if not isinstance(diagnostics, list) or len(diagnostics) != 1:
        raise AssertionError(f"expected exactly one diagnostic, found: {diagnostics}")

    diagnostic = diagnostics[0]
    if diagnostic.get("phase") != "semantic":
        raise AssertionError(f"expected semantic phase: {diagnostic}")
    if diagnostic.get("code") != "E004":
        raise AssertionError(f"expected E004 return mismatch: {diagnostic}")

    message = diagnostic.get("message", "")
    if f"expected {expected}" not in message or "found T" not in message:
        raise AssertionError(f"unexpected generic return mismatch message: {diagnostic}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--root", default=".")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    binary = Path(args.binary).resolve()

    string_payload = run_check(
        binary,
        root,
        root / "tests" / "errors" / "generic_return_annotation_mismatch.spectra",
    )
    require_generic_return_mismatch(string_payload, expected="string")

    int_payload = run_check(
        binary,
        root,
        root / "tests" / "errors" / "generic_return_type_mismatch_codegen_guard.spectra",
    )
    require_generic_return_mismatch(int_payload, expected="int")

    print("validated R-206 generic return type enforcement")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
