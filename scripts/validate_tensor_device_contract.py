#!/usr/bin/env python3
"""Execute the Spectra tensor device-placement contract fixtures.

The broad PowerShell runner compiles every validation fixture. This focused
gate executes the device-placement fixtures as well, including the legacy
Phase 7 regression that previously probed an unimplemented reserved device
and surfaced the expected host-call error as a Windows illegal-instruction
exit code.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_NAMES = (
    "74_tensor_phase7_device.spectra",
    "75_tensor_phase7_gpu.spectra",
    "91_tensor_phase16_gpu_backend.spectra",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--binary",
        default=str(ROOT / "target" / "debug" / "spectralang.exe"),
        help="path to the SpectraLang CLI binary",
    )
    parser.add_argument(
        "--report",
        default=str(ROOT / "target" / "tensor-device" / "contract-report.json"),
        help="JSON report path",
    )
    parser.add_argument("--timeout-seconds", type=float, default=60.0)
    return parser.parse_args()


def resolve_path(value: str) -> Path:
    path = Path(value)
    return path if path.is_absolute() else (ROOT / path).resolve()


def run_fixture(binary: Path, fixture: Path, timeout_seconds: float) -> dict[str, object]:
    relative = fixture.relative_to(ROOT).as_posix()
    try:
        completed = subprocess.run(
            [str(binary), "run", str(fixture)],
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=timeout_seconds,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        return {
            "fixture": relative,
            "status": "timeout",
            "exit_code": 124,
            "detail": str(error),
        }

    output = (completed.stdout + "\n" + completed.stderr).strip()
    result: dict[str, object] = {
        "fixture": relative,
        "status": "passed" if completed.returncode == 0 else "failed",
        "exit_code": completed.returncode,
    }
    if output:
        result["detail"] = output[-2000:]
    return result


def main() -> int:
    args = parse_args()
    binary = resolve_path(args.binary)
    if not binary.is_file():
        print(f"binary not found: {binary}", file=sys.stderr)
        return 2

    fixtures = [ROOT / "tests" / "validation" / name for name in FIXTURE_NAMES]
    missing = [path.relative_to(ROOT).as_posix() for path in fixtures if not path.is_file()]
    if missing:
        print("missing fixtures:", *missing, sep="\n  ", file=sys.stderr)
        return 2

    cases = [run_fixture(binary, fixture, args.timeout_seconds) for fixture in fixtures]
    failed = [case for case in cases if case["status"] != "passed"]
    report = {
        "schema": "spectralang.tensor_device_contract.v1",
        "fixture_count": len(cases),
        "passed": len(cases) - len(failed),
        "failed": len(failed),
        "cases": cases,
    }
    report_path = resolve_path(args.report)
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    print(f"tensor device fixtures: {report['passed']}/{report['fixture_count']} passed")
    if failed:
        for case in failed:
            print(f"FAIL {case['fixture']} (exit={case['exit_code']})", file=sys.stderr)
        return 1
    print(f"report: {report_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
