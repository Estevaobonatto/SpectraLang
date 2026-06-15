#!/usr/bin/env python3
"""Validate R-1204 std.option/std.result unwrap hostcall safety."""

from __future__ import annotations

import subprocess
import sys


def main() -> int:
    proc = subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "spectra-runtime",
            "stdlib::tests::option_result_unwrap_wrong_variant_returns_host_status",
        ],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=120,
    )
    if proc.returncode != 0:
        sys.stdout.write(proc.stdout)
        return proc.returncode
    if "panicked at" in proc.stdout or "panic" in proc.stdout.lower():
        raise AssertionError(proc.stdout)
    print("validated R-1204 std unwrap hostcall safety")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
