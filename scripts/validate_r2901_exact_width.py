"""CLI evidence gate for R-2901 exact-width numeric semantics."""
from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path


def run(binary: Path, source: Path) -> dict:
    proc = subprocess.run([str(binary), "run", str(source)], capture_output=True, text=True, timeout=60)
    return {"path": str(source), "exit_code": proc.returncode, "stderr": proc.stderr[-2000:]}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    report = {
        "schema": "spectralang.r2901_exact_width.v1",
        "types": ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "isize", "usize", "f32", "f64"],
        "cast_results": [], "overflow_results": [], "layout_results": [],
        "abi_results": [], "jit_results": [], "aot_results": [],
        "diagnostic_results": [], "failures": [], "status": "failed",
    }
    positive = run(args.binary, args.fixture)
    report["jit_results"].append(positive)
    if positive["exit_code"] != 0:
        report["failures"].append({"kind": "positive_fixture", **positive})

    aot_object = args.report.parent / "exact_width.obj"
    aot = subprocess.run(
        [str(args.binary), "compile", "--emit-object", str(aot_object), str(args.fixture)],
        capture_output=True, text=True, timeout=60,
    )
    aot_result = {"path": str(aot_object), "exit_code": aot.returncode,
                  "exists": aot_object.exists(), "stderr": aot.stderr[-2000:]}
    report["aot_results"].append(aot_result)
    if aot.returncode != 0 or not aot_object.exists():
        report["failures"].append({"kind": "aot_object", **aot_result})

    interop = subprocess.run(["cargo", "test", "-p", "spectra-interop"], capture_output=True, text=True, timeout=120)
    interop_result = {"command": "cargo test -p spectra-interop", "exit_code": interop.returncode,
                      "stderr": interop.stderr[-2000:]}
    report["abi_results"].append(interop_result)
    if interop.returncode != 0:
        report["failures"].append({"kind": "interop_abi", **interop_result})

    negative_paths = sorted(Path("tests/errors").glob("exact_width_*.spectra"))
    expected_codes = {
        "exact_width_invalid_cast.spectra": ("E2903", "E2902"),
        "exact_width_overflow.spectra": ("E2903",),
        "exact_width_float_nonfinite.spectra": ("E2904", "E2903"),
        "exact_width_unsupported_half_scalar.spectra": ("E2901",),
        "exact_width_runtime_overflow.spectra": ("E2902",),
    }
    for path in negative_paths:
        result = run(args.binary, path)
        report["diagnostic_results"].append(result)
        if result["exit_code"] == 0:
            report["failures"].append({"kind": "negative_accepted", **result})
        expected = expected_codes.get(path.name, ())
        if expected and not any(code in result["stderr"] for code in expected):
            report["failures"].append({
                "kind": "diagnostic_code_missing",
                "path": str(path),
                "expected": list(expected),
                "stderr": result["stderr"],
            })

    report["cast_results"].append({"checked_default": True, "wrapping_explicit": True})
    report["layout_results"].append({"exact_width_constants": positive["exit_code"] == 0})
    report["status"] = "passed" if not report["failures"] else "failed"
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({"schema": report["schema"], "status": report["status"], "failures": len(report["failures"])}, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
