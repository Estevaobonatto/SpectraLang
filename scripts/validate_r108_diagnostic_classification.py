#!/usr/bin/env python3
"""Validate R-108 diagnostic classification hardening."""

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


def run_check(binary: Path, source: Path) -> dict[str, Any]:
    proc = subprocess.run(
        [str(binary), "check", "--json", str(source)],
        cwd=source.parents[2],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=20,
    )
    if proc.returncode == 0:
        raise AssertionError(f"{source} unexpectedly passed:\n{proc.stdout}")
    return extract_json_payload(proc.stdout)


def diagnostics(payload: dict[str, Any]) -> list[dict[str, Any]]:
    files = payload.get("files")
    if not isinstance(files, list) or not files:
        raise AssertionError(f"missing files in payload: {payload}")
    result = files[0].get("diagnostics")
    if not isinstance(result, list) or not result:
        raise AssertionError(f"missing diagnostics in payload: {payload}")
    return result


def require_single_semantic_diagnostic(
    payload: dict[str, Any],
    *,
    expected_code: str,
    message_parts: tuple[str, ...],
    hint_parts: tuple[str, ...] = (),
) -> None:
    observed = diagnostics(payload)
    if len(observed) != 1:
        raise AssertionError(f"expected exactly one diagnostic, found {len(observed)}: {observed}")

    diagnostic = observed[0]
    if diagnostic.get("phase") != "semantic":
        raise AssertionError(f"expected semantic phase, found: {diagnostic}")
    if diagnostic.get("code") != expected_code:
        raise AssertionError(f"expected code {expected_code}, found: {diagnostic}")

    message = diagnostic.get("message", "")
    for part in message_parts:
        if part not in message:
            raise AssertionError(f"message missing {part!r}: {diagnostic}")

    hint = diagnostic.get("hint", "") or ""
    for part in hint_parts:
        if part not in hint:
            raise AssertionError(f"hint missing {part!r}: {diagnostic}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--root", default=".")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    binary = Path(args.binary).resolve()

    trait_payload = run_check(
        binary,
        root / "tests" / "errors" / "trait_bound_missing_method_stress.spectra",
    )
    require_single_semantic_diagnostic(
        trait_payload,
        expected_code="E010",
        message_parts=(
            "Type 'Plain' does not satisfy trait bound 'T: Score'",
            "function 'evaluate'",
        ),
        hint_parts=("Implement trait 'Score' for type 'Plain'",),
    )

    alias_payload = run_check(
        binary,
        root / "tests" / "errors" / "std_alias_unknown_member.spectra",
    )
    require_single_semantic_diagnostic(
        alias_payload,
        expected_code="E011",
        message_parts=(
            "Module 'math' does not export member 'not_a_function'",
        ),
        hint_parts=("Available exports from 'math'", "sqrt_f", "sin_f"),
    )

    print("validated R-108 diagnostic classification hardening")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
