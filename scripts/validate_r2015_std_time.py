#!/usr/bin/env python3
"""Validate R-2015 std.time production surface."""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import tomllib
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


def require_text(path: Path, needles: list[str]) -> None:
    text = path.read_text(encoding="utf-8")
    missing = [needle for needle in needles if needle not in text]
    if missing:
        raise SystemExit(f"{path} missing required text: {', '.join(missing)}")


def validate_roadmap() -> None:
    roadmap_path = ROOT / "roadmap" / "roadmap.toml"
    data = tomllib.loads(roadmap_path.read_text(encoding="utf-8"))
    items = {item["id"]: item for item in data["items"]}
    if "R-2015" not in items:
        raise SystemExit("roadmap missing R-2015")
    item = items["R-2015"]
    expected = {
        "phase": "phase_20",
        "owner": "runtime",
        "priority": "P0",
        "status": "complete",
        "risk": "high",
    }
    for key, value in expected.items():
        if item.get(key) != value:
            raise SystemExit(f"R-2015 {key} expected {value}, got {item.get(key)}")
    acceptance = "\n".join(item.get("acceptance", []))
    for needle in [
        "monotonic",
        "Duration",
        "Instant",
        "UtcDateTime",
        "tests/validation/150_std_time_production.spectra",
    ]:
        if needle not in acceptance:
            raise SystemExit(f"R-2015 acceptance missing {needle}")

    r2013_deps = set(items["R-2013"].get("dependencies", []))
    if "R-2015" not in r2013_deps:
        raise SystemExit("R-2013 must depend on R-2015")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", default=str(ROOT / "target" / "debug" / "spectralang.exe"))
    args = parser.parse_args()

    binary = Path(args.binary)
    if not binary.exists():
        raise SystemExit(f"spectralang binary not found: {binary}")

    validate_roadmap()
    require_text(
        ROOT / "docs" / "roadmap-backlog.md",
        [
            "## R-2015 std.time Production Time Surface",
            "tests/validation/150_std_time_production.spectra",
            "duration_secs_value",
        ],
    )
    require_text(
        ROOT / "docs" / "reference" / "05-stdlib.md",
        ["Duration", "Instant", "UtcDateTime", "monotonic_millis", "unix_to_utc"],
    )
    require_text(
        ROOT / "docs" / "AI-AGENT-REFERENCE.md",
        ["duration_ms", "duration_secs_value", "instant_elapsed_ms", "utc_year"],
    )
    require_text(
        ROOT / "docs" / "runtime" / "standard-library.md",
        ["spectra.std.time.monotonic_millis", "spectra.std.time.unix_to_utc"],
    )
    require_text(
        ROOT / "compiler" / "tests" / "snapshots" / "std_time_public_function_table.snap",
        ["std.time.Duration", "std.time.utc_second"],
    )

    run_checked(
        [
            cargo_bin(),
            "test",
            "-p",
            "spectra-runtime",
            "std_time",
            "--",
            "--test-threads=1",
        ],
        timeout=240,
    )
    run_checked(
        [
            cargo_bin(),
            "test",
            "-p",
            "spectra-compiler",
            "std_time",
        ],
        timeout=180,
    )
    run_checked(
        [str(binary), "run", str(ROOT / "tests" / "validation" / "150_std_time_production.spectra")],
        timeout=120,
    )

    print("validated R-2015 std.time production surface")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
