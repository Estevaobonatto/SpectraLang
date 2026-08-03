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


def failed_group(rc: int, output: str, samples: list[int]) -> dict[str, Any]:
    return {
        "warmup_runs": WARMUPS,
        "timed_runs": TIMED_RUNS,
        "exit_code": rc,
        "failure_class": "timeout" if rc == -1 else "runtime",
        "output_tail": output,
        "samples_ns": samples,
    }


def measure_paired_group(
    control_command: list[str],
    candidate_command: list[str],
    *,
    cwd: Path | None,
    order_seed: int,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Measure control and candidate interleaved to reduce host jitter."""
    control_samples: list[int] = []
    candidate_samples: list[int] = []
    control_output = ""
    candidate_output = ""

    def run_pair(first_is_control: bool) -> tuple[tuple[int, int, str], tuple[int, int, str]]:
        first = control_command if first_is_control else candidate_command
        second = candidate_command if first_is_control else control_command
        first_result = run_command(first, cwd=cwd)
        second_result = run_command(second, cwd=cwd)
        return (
            (first_result, second_result)
            if first_is_control
            else (second_result, first_result)
        )

    for warmup in range(WARMUPS):
        control_result, candidate_result = run_pair((order_seed + warmup) % 2 == 0)
        if control_result[0] != 0 or candidate_result[0] != 0:
            return (
                failed_group(control_result[0], control_result[2], control_samples),
                failed_group(candidate_result[0], candidate_result[2], candidate_samples),
            )

    for sample in range(TIMED_RUNS):
        control_result, candidate_result = run_pair((order_seed + sample) % 2 == 0)
        control_output = control_result[2]
        candidate_output = candidate_result[2]
        if control_result[0] != 0 or candidate_result[0] != 0:
            return (
                failed_group(control_result[0], control_output, control_samples),
                failed_group(candidate_result[0], candidate_output, candidate_samples),
            )
        control_samples.append(control_result[1])
        candidate_samples.append(candidate_result[1])

    def successful_group(samples: list[int], command: list[str], output: str) -> dict[str, Any]:
        return {
            **stats(samples),
            "warmup_runs": WARMUPS,
            "timed_runs": TIMED_RUNS,
            "exit_code": 0,
            "failure_class": None,
            "output_tail": output,
            "command": command,
        }

    return (
        successful_group(control_samples, control_command, control_output),
        successful_group(candidate_samples, candidate_command, candidate_output),
    )


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


def summarize_language_groups(groups: list[dict[str, Any]]) -> dict[str, Any]:
    successful = [group for group in groups if group.get("exit_code") == 0]
    medians = [group["median_ns"] for group in successful]
    return {
        "groups": groups,
        "successful_independent_runs": len(successful),
        "median_of_group_medians_ns": int(median(medians)) if medians else 0,
        "group_medians_ns": medians,
    }


def capture_scenario(
    binary: Path,
    control_binary: Path,
    scenario: str,
    out_root: Path,
    independent_runs: int,
) -> dict[str, Any]:
    source = ROOT / "benchmarks" / "cross-lang" / scenario / "spectra" / "bench.spectra"
    if not source.is_file():
        raise RuntimeError(f"missing Spectra fixture for {scenario}: {source}")
    candidate_output = out_root / "candidate" / scenario / "spectra.exe"
    control_output = out_root / "control" / scenario / "spectra.exe"
    candidate_compile = compile_spectra(binary, source, candidate_output)
    control_compile = compile_spectra(control_binary, source, control_output)
    if candidate_compile["exit_code"] != 0 or control_compile["exit_code"] != 0:
        return {
            "source": source.relative_to(ROOT).as_posix(),
            "aot_compile": candidate_compile,
            "control_aot_compile": control_compile,
            "languages": {},
            "control_languages": {},
            "ratios": {},
            "correctness_passed": False,
            "control_correctness_passed": False,
        }

    commands = {
        "spectra": ([str(candidate_output)], ROOT),
        "go": ([str(build_go(scenario))], None),
        "rust": ([str(build_rust(scenario))], None),
    }
    control_command = ([str(control_output)], ROOT)
    language_groups: dict[str, list[dict[str, Any]]] = {language: [] for language in commands}
    control_groups: list[dict[str, Any]] = []
    for independent in range(independent_runs):
        control_measured, candidate_measured = measure_paired_group(
            control_command[0],
            commands["spectra"][0],
            cwd=ROOT,
            order_seed=independent,
        )
        control_measured["independent_run"] = independent + 1
        control_measured["command"] = control_command[0]
        control_groups.append(control_measured)
        candidate_measured["independent_run"] = independent + 1
        candidate_measured["command"] = commands["spectra"][0]
        language_groups["spectra"].append(candidate_measured)
        for language, (command, cwd) in commands.items():
            if language == "spectra":
                continue
            measured = measure_group(command, cwd=cwd)
            measured["independent_run"] = independent + 1
            measured["command"] = command
            language_groups[language].append(measured)

    languages = {
        language: summarize_language_groups(groups)
        for language, groups in language_groups.items()
    }
    control_languages = {"spectra": summarize_language_groups(control_groups)}
    spectra_median = languages["spectra"]["median_of_group_medians_ns"]
    go_median = languages["go"]["median_of_group_medians_ns"]
    rust_median = languages["rust"]["median_of_group_medians_ns"]
    control_spectra_median = control_languages["spectra"]["median_of_group_medians_ns"]
    return {
        "source": source.relative_to(ROOT).as_posix(),
        "aot_compile": candidate_compile,
        "control_aot_compile": control_compile,
        "languages": languages,
        "control_languages": control_languages,
        "ratios": {
            "spectra_to_go": spectra_median / go_median if go_median else None,
            "spectra_to_rust": spectra_median / rust_median if rust_median else None,
            "candidate_spectra_to_control_spectra": (
                spectra_median / control_spectra_median if control_spectra_median else None
            ),
        },
        "correctness_passed": all(
            group.get("exit_code") == 0
            for groups in language_groups.values()
            for group in groups
        ),
        "control_correctness_passed": all(
            group.get("exit_code") == 0 for group in control_groups
        ),
    }


def capture(
    *,
    binary: Path,
    control_binary: Path,
    control_source_root: Path | None,
    scenarios: tuple[str, ...],
    output_root: Path,
    independent_runs: int,
) -> dict[str, Any]:
    binary = binary.resolve()
    control_binary = control_binary.resolve()
    if not binary.is_file():
        raise RuntimeError(f"release binary does not exist: {binary}")
    if not control_binary.is_file():
        raise RuntimeError(f"clean control binary does not exist: {control_binary}")
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
        results[scenario] = capture_scenario(
            binary, control_binary, scenario, output_root, independent_runs
        )
    control_source_fingerprint = (
        source_tree_fingerprint(control_source_root.resolve())
        if control_source_root is not None
        else ""
    )
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
        "control": {
            "binary": control_binary.relative_to(ROOT).as_posix()
            if control_binary.is_relative_to(ROOT)
            else str(control_binary),
            "binary_sha256": sha256_file(control_binary),
            "source_tree_fingerprint": control_source_fingerprint,
        },
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
    parser.add_argument("--control-binary", type=Path, required=True)
    parser.add_argument("--control-source-root", type=Path)
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
            control_binary=args.control_binary,
            control_source_root=args.control_source_root,
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
