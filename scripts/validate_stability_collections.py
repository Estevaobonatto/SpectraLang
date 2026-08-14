"""Validate the typed collection and iterator contract across JIT and AOT."""
from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCHEMA = "spectralang.stability.collections.v1"
POSITIVE = ROOT / "tests" / "validation" / "stability_set_iterator.spectra"
POSITIVE_TYPED_VALUES = ROOT / "tests" / "validation" / "stability_typed_collection_values.spectra"
NEGATIVE = ROOT / "tests" / "errors" / "stability_collection_type_mismatch.spectra"
NEGATIVE_GENERIC = ROOT / "tests" / "errors" / "stability_set_iterator_type_mismatch.spectra"


def run(command: list[str], timeout: int = 180) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=ROOT,
        capture_output=True,
        text=True,
        timeout=timeout,
        check=False,
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=Path("target/debug/spectralang.exe"))
    parser.add_argument("--report", type=Path, default=Path("target/stability/collections.json"))
    args = parser.parse_args()
    binary = args.binary if args.binary.is_absolute() else ROOT / args.binary
    report_path = args.report if args.report.is_absolute() else ROOT / args.report
    report_path.parent.mkdir(parents=True, exist_ok=True)

    report: dict[str, object] = {
        "schema": SCHEMA,
        "fixture": str(POSITIVE.relative_to(ROOT)).replace("\\", "/"),
        "typed_values_fixture": str(POSITIVE_TYPED_VALUES.relative_to(ROOT)).replace("\\", "/"),
        "negative_fixture": str(NEGATIVE.relative_to(ROOT)).replace("\\", "/"),
        "negative_generic_fixture": str(NEGATIVE_GENERIC.relative_to(ROOT)).replace("\\", "/"),
        "checks": {},
        "failures": [],
    }
    failures: list[str] = report["failures"]  # type: ignore[assignment]

    checked = run([str(binary), "check", "--json", str(POSITIVE)])
    report["checks"]["positive_check"] = {  # type: ignore[index]
        "exit_code": checked.returncode,
        "passed": checked.returncode == 0,
        "stdout_tail": checked.stdout[-2000:],
        "stderr_tail": checked.stderr[-2000:],
    }
    if checked.returncode != 0:
        failures.append("positive fixture check failed")

    jit = run([str(binary), "run", str(POSITIVE)])
    report["checks"]["jit"] = {  # type: ignore[index]
        "exit_code": jit.returncode,
        "passed": jit.returncode == 0,
        "stdout_tail": jit.stdout[-2000:],
        "stderr_tail": jit.stderr[-2000:],
    }
    if jit.returncode != 0:
        failures.append("positive fixture JIT failed")

    typed_values = run([str(binary), "run", str(POSITIVE_TYPED_VALUES)])
    report["checks"]["typed_values_jit"] = {  # type: ignore[index]
        "exit_code": typed_values.returncode,
        "passed": typed_values.returncode == 0,
        "stdout_tail": typed_values.stdout[-2000:],
        "stderr_tail": typed_values.stderr[-2000:],
    }
    if typed_values.returncode != 0:
        failures.append("typed string collection JIT failed")

    negative = run([str(binary), "check", "--json", str(NEGATIVE)])
    report["checks"]["negative_check"] = {  # type: ignore[index]
        "exit_code": negative.returncode,
        "passed": negative.returncode != 0,
        "stdout_tail": negative.stdout[-2000:],
        "stderr_tail": negative.stderr[-2000:],
    }
    if negative.returncode == 0:
        failures.append("collection type mismatch unexpectedly compiled")

    negative_generic = run([str(binary), "check", "--json", str(NEGATIVE_GENERIC)])
    report["checks"]["negative_generic_check"] = {  # type: ignore[index]
        "exit_code": negative_generic.returncode,
        "passed": negative_generic.returncode != 0,
        "stdout_tail": negative_generic.stdout[-2000:],
        "stderr_tail": negative_generic.stderr[-2000:],
    }
    if negative_generic.returncode == 0:
        failures.append("Set<T>/Iterator<T> mismatch unexpectedly compiled")

    with tempfile.TemporaryDirectory(prefix="spectralang-collections-", dir=ROOT / "target") as temp:
        executable = Path(temp) / "collections.exe"
        compile_result = run(
            [str(binary), "compile", "--emit-exe", str(executable), str(POSITIVE)],
            timeout=240,
        )
        aot_run = run([str(executable)], timeout=60) if compile_result.returncode == 0 else None
        report["checks"]["aot"] = {  # type: ignore[index]
            "compile_exit_code": compile_result.returncode,
            "run_exit_code": None if aot_run is None else aot_run.returncode,
            "passed": compile_result.returncode == 0 and aot_run is not None and aot_run.returncode == 0,
            "compile_stdout_tail": compile_result.stdout[-2000:],
            "compile_stderr_tail": compile_result.stderr[-2000:],
            "run_stdout_tail": "" if aot_run is None else aot_run.stdout[-2000:],
            "run_stderr_tail": "" if aot_run is None else aot_run.stderr[-2000:],
        }
        if compile_result.returncode != 0 or aot_run is None or aot_run.returncode != 0:
            failures.append("positive fixture AOT failed")

        typed_executable = Path(temp) / "typed-values.exe"
        typed_compile = run(
            [str(binary), "compile", "--emit-exe", str(typed_executable), str(POSITIVE_TYPED_VALUES)],
            timeout=240,
        )
        typed_aot_run = run([str(typed_executable)], timeout=60) if typed_compile.returncode == 0 else None
        report["checks"]["typed_values_aot"] = {  # type: ignore[index]
            "compile_exit_code": typed_compile.returncode,
            "run_exit_code": None if typed_aot_run is None else typed_aot_run.returncode,
            "passed": typed_compile.returncode == 0
            and typed_aot_run is not None
            and typed_aot_run.returncode == 0,
            "compile_stdout_tail": typed_compile.stdout[-2000:],
            "compile_stderr_tail": typed_compile.stderr[-2000:],
            "run_stdout_tail": "" if typed_aot_run is None else typed_aot_run.stdout[-2000:],
            "run_stderr_tail": "" if typed_aot_run is None else typed_aot_run.stderr[-2000:],
        }
        if typed_compile.returncode != 0 or typed_aot_run is None or typed_aot_run.returncode != 0:
            failures.append("typed string collection AOT failed")

    report["status"] = "passed" if not failures else "failed"
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"collections status={report['status']} report={report_path}")
    for failure in failures:
        print(f"failure: {failure}")
    return 0 if not failures else 1


if __name__ == "__main__":
    raise SystemExit(main())
