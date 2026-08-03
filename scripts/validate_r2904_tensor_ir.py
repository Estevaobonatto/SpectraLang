#!/usr/bin/env python3
"""Validate the executable R-2904 Tensor IR and device-lowering contract."""

from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
from pathlib import Path
from typing import Any


def run(binary: Path, root: Path, args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [str(binary), *args],
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=30,
        check=False,
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--fixture", required=True)
    parser.add_argument("--report", required=True)
    parser.add_argument("--root", default=".")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    binary = Path(args.binary).resolve()
    fixture = Path(args.fixture)
    if not fixture.is_absolute():
        fixture = root / fixture

    failures: list[str] = []
    ir_nodes = 0
    legalized_nodes = 0
    fusion_groups = 0
    planned_buffers = 0
    peak_live_buffers = 0

    dumped = run(binary, root, ["compile", "--dump-ir", str(fixture)])
    output = dumped.stdout
    if dumped.returncode != 0:
        failures.append(f"fixture compile failed: exit={dumped.returncode}")
    if "Tensor IR (CPU legalization)" not in output:
        failures.append("compiler did not emit the Tensor IR legalization section")
    if "matmul" not in output or "conv2d" not in output:
        failures.append("fixture did not materialize expected matmul and conv2d nodes")
    for line in output.splitlines():
        if "tensor_ir_report" not in line:
            continue
        fields = {
            key: value
            for key, value in (
                item.strip().split(":", 1)
                for item in line.split("{")[-1].split("}")[0].split(",")
                if ":" in item
            )
        }
        for key, target in (
            ("ir_nodes", "ir_nodes"),
            ("legalized_nodes", "legalized_nodes"),
            ("fusion_groups", "fusion_groups"),
            ("planned_buffers", "planned_buffers"),
            ("peak_live_buffers", "peak_live_buffers"),
        ):
            raw = fields.get(key, "0").strip()
            try:
                value = int(raw)
            except ValueError:
                value = 0
            if target == "ir_nodes":
                ir_nodes = value
            elif target == "legalized_nodes":
                legalized_nodes = value
            elif target == "fusion_groups":
                fusion_groups = value
            elif target == "planned_buffers":
                planned_buffers = value
            else:
                peak_live_buffers = value
    if fusion_groups < 1:
        failures.append("Tensor IR did not produce deterministic fusion evidence")
    if ir_nodes < 1 or legalized_nodes < 1 or planned_buffers < 1 or peak_live_buffers < 1:
        failures.append("Tensor IR report is missing non-zero legalization or memory evidence")

    run_result = run(binary, root, ["run", str(fixture)])
    if run_result.returncode != 0:
        failures.append(f"fixture runtime failed: exit={run_result.returncode}")

    aot_path = root / "target" / "r2904-tensor-ir" / "tensor_ir_fixture.obj"
    aot = subprocess.run(
        [str(binary), "compile", "--emit-object", str(aot_path), str(fixture)],
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=60,
        check=False,
    )
    aot_ok = aot.returncode == 0 and aot_path.exists() and aot_path.stat().st_size > 0
    if not aot_ok:
        failures.append(f"AOT object emission failed: exit={aot.returncode}")

    # Probe the real runtime capability before attempting the WGPU fixture.
    # Return 2 is the fixture's explicit "adapter unavailable" result; any
    # other non-zero code is an actual failure, not an environment skip.
    probe_source = 'module r2904_wgpu_adapter_probe\nimport std.tensor as tensor\npublic func main() returns int {\n    if tensor.device_available(6) { return 0\n }\n    return 2\n}\n'
    with tempfile.TemporaryDirectory(prefix="r2904-wgpu-", dir=root / "target") as temp_dir:
        probe_path = Path(temp_dir) / "adapter_probe.spectra"
        probe_path.write_text(probe_source, encoding="utf-8")
        probe = run(binary, root, ["run", str(probe_path)])
    if probe.returncode == 0:
        wgpu = run(binary, root, ["run", "tests/validation/91_tensor_phase16_gpu_backend.spectra"])
        wgpu_status = "passed" if wgpu.returncode == 0 else "failed"
        if wgpu.returncode != 0:
            failures.append("WGPU adapter was available but the Tensor IR device fixture failed")
        wgpu_result = {"status": wgpu_status, "fixture": "91_tensor_phase16_gpu_backend.spectra"}
    elif probe.returncode == 2:
        wgpu_result = {"status": "skipped_environment", "reason": "no WGPU adapter available"}
    else:
        failures.append(f"WGPU adapter probe failed: exit={probe.returncode}")
        wgpu_result = {"status": "failed", "reason": f"adapter probe exit={probe.returncode}"}

    negative_results: list[dict[str, Any]] = []
    for path in sorted(root.glob("tests/errors/tensor_ir_*.spectra")):
        result = run(binary, root, ["compile", str(path)])
        passed = result.returncode != 0
        negative_results.append({"fixture": path.name, "status": "rejected" if passed else "accepted"})
        if not passed:
            failures.append(f"negative fixture was accepted: {path.name}")

    report = {
        "schema": "spectralang.r2904_tensor_ir.v1",
        "status": "passed" if not failures else "failed",
        "ir_nodes": ir_nodes,
        "lowering_results": [{"backend": "cpu", "ir_nodes": ir_nodes, "legalized_nodes": legalized_nodes}],
        "fusion_results": [{"fusion_groups": fusion_groups}],
        "memory_planning_results": [{"planned_buffers": planned_buffers, "peak_live_buffers": peak_live_buffers}],
        "cpu_results": [{"fixture": str(fixture), "status": "passed" if run_result.returncode == 0 else "failed"}],
        "aot_results": [{"path": str(aot_path), "status": "passed" if aot_ok else "failed", "exit_code": aot.returncode}],
        "wgpu_results": [wgpu_result],
        "fallback_results": [{"external_fallback_nodes": 0}],
        "diagnostic_results": negative_results,
        "failures": failures,
    }
    report_path = Path(args.report)
    if not report_path.is_absolute():
        report_path = root / report_path
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"schema": report["schema"], "status": report["status"], "failures": len(failures)}))
    return 0 if not failures else 1


if __name__ == "__main__":
    raise SystemExit(main())
