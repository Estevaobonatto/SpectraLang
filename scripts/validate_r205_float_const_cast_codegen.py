#!/usr/bin/env python3
"""Validate R-205 float const cast codegen."""

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


def run_compile_dump_ir(binary: Path, root: Path, source: Path) -> str:
    proc = subprocess.run(
        [str(binary), "compile", "--dump-ir", str(source)],
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=20,
    )
    if proc.returncode != 0:
        raise AssertionError(f"{source} failed to compile:\n{proc.stdout}")
    if "Verifier errors" in proc.stdout or "error[codegen]" in proc.stdout:
        raise AssertionError(f"{source} reached invalid backend codegen:\n{proc.stdout}")
    return proc.stdout


def run_check_json(binary: Path, root: Path, source: Path) -> dict[str, Any]:
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


def require_lowered_integer_consts(output: str) -> None:
    required = ("const.int 7", "const.int -3")
    for marker in required:
        if marker not in output:
            raise AssertionError(f"dumped IR missing {marker!r}:\n{output}")

    forbidden = ("ne %v0, %v1", "const.float 7.75", "const.float -3.25")
    for marker in forbidden:
        if marker in output:
            raise AssertionError(f"dumped IR still contains invalid float-cast path {marker!r}:\n{output}")


def require_semantic_invalid_cast(payload: dict[str, Any]) -> None:
    files = payload.get("files")
    if not isinstance(files, list) or not files:
        raise AssertionError(f"missing files in payload: {payload}")
    diagnostics = files[0].get("diagnostics")
    if not isinstance(diagnostics, list) or len(diagnostics) != 1:
        raise AssertionError(f"expected exactly one diagnostic, found: {diagnostics}")

    diagnostic = diagnostics[0]
    if diagnostic.get("phase") != "semantic":
        raise AssertionError(f"expected semantic diagnostic: {diagnostic}")

    message = diagnostic.get("message", "")
    source_is_float = any(token in message for token in ("float", "f16", "f32", "f64"))
    if "Cannot cast" not in message or not source_is_float or "string" not in message:
        raise AssertionError(f"unexpected invalid cast diagnostic: {diagnostic}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--root", default=".")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    binary = Path(args.binary).resolve()

    valid_source = root / "tests" / "validation" / "100_float_const_cast_codegen.spectra"
    invalid_source = root / "tests" / "errors" / "float_const_invalid_cast.spectra"

    dump_ir = run_compile_dump_ir(binary, root, valid_source)
    require_lowered_integer_consts(dump_ir)

    invalid_payload = run_check_json(binary, root, invalid_source)
    require_semantic_invalid_cast(invalid_payload)

    print("validated R-205 float const cast codegen")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
