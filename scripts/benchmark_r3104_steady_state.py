#!/usr/bin/env python3
"""Measure runtime steady state after removing JIT/process startup from runs.

The regular Phase 31 runner intentionally measures the user-visible ``run``
command.  R-3104 also needs a runtime-only signal so a codegen change is not
mistaken for an execution regression.  This harness compiles Spectra once to
an AOT executable, then applies the same warmup/sample policy to that
executable and to the Go/Rust controls.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import subprocess
import sys
import time
from pathlib import Path
from statistics import mean, median, pstdev
from typing import Any

try:
    from scripts.benchmark_r3104_codegen import source_tree_fingerprint
    from scripts.phase31_contract import SCENARIOS
    from scripts.phase31_run_all import build_go, build_rust
except ModuleNotFoundError:  # pragma: no cover
    from benchmark_r3104_codegen import source_tree_fingerprint  # type: ignore[no-redef]
    from phase31_contract import SCENARIOS  # type: ignore[no-redef]
    from phase31_run_all import build_go, build_rust  # type: ignore[no-redef]


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SCENARIOS = (
    "cpu-loop-sum",
    "cpu-fibs",
    "cpu-hashmap",
    "tensor-create",
    "tensor-elementwise",
    "tensor-matmul",
)
WARMUPS = 3
TIMED_RUNS = 20
INDEPENDENT_RUNS = 5
TIMEOUT_SECONDS = 300


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def run_command(command: list[str], *, cwd: Path | None = None) -> tuple[int, int, str]:
    started = time.perf_counter_ns()
    try:
        result = subprocess.run(
            command,
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=TIMEOUT_SECONDS,
            check=False,
        )
    except subprocess.TimeoutExpired as exc:
        elapsed = time.perf_counter_ns() - started
        return -1, elapsed, f"timeout after {TIMEOUT_SECONDS}s: {exc}"
    elapsed = time.perf_counter_ns() - started
    output = ((result.stdout or "") + "\n" + (result.stderr or "")).strip()
    return result.returncode, elapsed, output[-4000:]


def stats(samples: list[int]) -> dict[str, Any]:
    ordered = sorted(samples)
    if not ordered:
        return {"median_ns": 0, "p95_ns": 0, "mean_ns": 0, "stddev_ns": 0, "samples_ns": []}
    p95_index = max(0, math.ceil(len(ordered) * 0.95) - 1)
    return {
        "median_ns": int(median(ordered)),
        "p95_ns": int(ordered[p95_index]),
        "mean_ns": mean(ordered),
        "stddev_ns": pstdev(ordered) if len(ordered) > 1 else 0.0,
        "samples_ns": ordered,
    }


def measure_group(command: list[str], *, cwd: Path | None) -> dict[str, Any]:
    for _ in range(WARMUPS):
        rc, _, output = run_command(command, cwd=cwd)
        if rc != 0:
            return {
                "warmup_runs": WARMUPS,
                "timed_runs": TIMED_RUNS,
                "exit_code": rc,
                "failure_class": "timeout" if rc == -1 else "runtime",
                "output_tail": output,
                "samples_ns": [],
            }
    samples: list[int] = []
    output_tail = ""
    for _ in range(TIMED_RUNS):
        rc, elapsed, output = run_command(command, cwd=cwd)
        output_tail = output
        if rc != 0:
            return {
                "warmup_runs": WARMUPS,
                "timed_runs": TIMED_RUNS,
                "exit_code": rc,
                "failure_class": "timeout" if rc == -1 else "runtime",
                "output_tail": output,
                "samples_ns": samples,
            }
        samples.append(elapsed)
    return {
        **stats(samples),
        "warmup_runs": WARMUPS,
        "timed_runs": TIMED_RUNS,
        "exit_code": 0,
        "failure_class": None,
        "output_tail": output_tail,
    }


def compile_spectra(binary: Path, source: Path, output: Path) -> dict[str, Any]:
    output.parent.mkdir(parents=True, exist_ok=True)
    command = [str(binary), "compile", "--emit-exe", str(output), str(source)]
    rc, elapsed, output_tail = run_command(command, cwd=ROOT)
    return {
        "command": command,
        "exit_code": rc,
        "elapsed_ns": elapsed,
        "output_tail": output_tail,
        "output": output.relative_to(ROOT).as_posix(),
        "output_sha256": sha256_file(output) if rc == 0 and output.is_file() else None,
    }


def capture_scenario(binary: Path, scenario: str, out_root: Path, independent_runs: int) -> dict[str, Any]:
    source = ROOT / "benchmarks" / "cross-lang" / scenario / "spectra" / "bench.spectra"
    if not source.is_file():
        raise RuntimeError(f"missing Spectra fixture for {scenario}: {source}")
    spectra_output = out_root / scenario / "spectra.exe"
    compile_result = compile_spectra(binary, source, spectra_output)
    if compile_result["exit_code"] != 0:
        return {
            "source": source.relative_to(ROOT).as_posix(),
            "aot_compile": compile_result,
            "languages": {},
            "ratios": {},
            "correctness_passed": False,
        }

    commands = {
        "spectra": ([str(spectra_output)], ROOT),
        "go": ([str(build_go(scenario))], None),
        "rust": ([str(build_rust(scenario))], None),
    }
    language_groups: dict[str, list[dict[str, Any]]] = {language: [] for language in commands}
    for independent in range(independent_runs):
        for language, (command, cwd) in commands.items():
            measured = measure_group(command, cwd=cwd)
            measured["independent_run"] = independent + 1
            measured["command"] = command
            language_groups[language].append(measured)

    languages: dict[str, Any] = {}
    for language, groups in language_groups.items():
        successful = [group for group in groups if group.get("exit_code") == 0]
        medians = [group["median_ns"] for group in successful]
        languages[language] = {
            "groups": groups,
            "successful_independent_runs": len(successful),
            "median_of_group_medians_ns": int(median(medians)) if medians else 0,
            "group_medians_ns": medians,
        }
    spectra_median = languages["spectra"]["median_of_group_medians_ns"]
    go_median = languages["go"]["median_of_group_medians_ns"]
    rust_median = languages["rust"]["median_of_group_medians_ns"]
    return {
        "source": source.relative_to(ROOT).as_posix(),
        "aot_compile": compile_result,
        "languages": languages,
        "ratios": {
            "spectra_to_go": spectra_median / go_median if go_median else None,
            "spectra_to_rust": spectra_median / rust_median if rust_median else None,
        },
        "correctness_passed": all(
            group.get("exit_code") == 0
            for groups in language_groups.values()
            for group in groups
        ),
    }


def capture(*, binary: Path, scenarios: tuple[str, ...], output_root: Path, independent_runs: int) -> dict[str, Any]:
    binary = binary.resolve()
    if not binary.is_file():
        raise RuntimeError(f"release binary does not exist: {binary}")
    revision = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=False
    )
    if revision.returncode != 0 or not revision.stdout.strip():
        raise RuntimeError("unable to resolve current Git revision")
    results: dict[str, Any] = {}
    for scenario in scenarios:
        if scenario not in SCENARIOS:
            raise RuntimeError(f"unsupported R-3104 steady-state scenario: {scenario}")
        print(f"[r3104-steady] running {scenario}", flush=True)
        results[scenario] = capture_scenario(binary, scenario, output_root, independent_runs)
    return {
        "schema": "spectra.phase31.r3104_steady_state.v1",
        "task": "R-3104",
        "classification": "benchmark_and_ir_hypothesis",
        "profiling_causal_claim": False,
        "git_revision": revision.stdout.strip(),
        "source_tree_fingerprint": source_tree_fingerprint(ROOT),
        "profile": "release",
        "binary": binary.relative_to(ROOT).as_posix() if binary.is_relative_to(ROOT) else str(binary),
        "binary_sha256": sha256_file(binary),
        "benchmark_languages": ["spectra", "go", "rust"],
        "java_excluded": True,
        "scenarios": list(scenarios),
        "measurement_policy": {
            "warmup_runs": WARMUPS,
            "timed_runs": TIMED_RUNS,
            "independent_runs": independent_runs,
            "runtime_measurement": "precompiled_aot_executable",
        },
        "results": results,
    }


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--spectra-binary", type=Path, required=True)
    parser.add_argument("--out", type=Path, default=Path("target/phase31/r3104-steady-state.json"))
    parser.add_argument("--independent-runs", type=int, default=INDEPENDENT_RUNS)
    parser.add_argument("--all-scenarios", action="store_true")
    parser.add_argument("--scenarios", nargs="+", default=None)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if args.independent_runs < INDEPENDENT_RUNS:
        print("R-3104 steady-state benchmark: BLOCKED: requires at least 5 independent runs", file=sys.stderr)
        return 1
    scenarios = tuple(args.scenarios or (SCENARIOS if args.all_scenarios else DEFAULT_SCENARIOS))
    try:
        payload = capture(
            binary=args.spectra_binary,
            scenarios=scenarios,
            output_root=ROOT / "target" / "phase31" / "r3104-steady-state",
            independent_runs=args.independent_runs,
        )
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8", newline="\n")
    except (OSError, RuntimeError, ValueError) as exc:
        print(f"R-3104 steady-state benchmark: BLOCKED: {exc}", file=sys.stderr)
        return 1
    print(f"R-3104 steady-state captured: {len(payload['scenarios'])} scenarios")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
