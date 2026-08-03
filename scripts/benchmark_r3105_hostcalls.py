#!/usr/bin/env python3
"""Measure the dedicated R-3105 generic-hostcall batch microbenchmark.

The candidate and the clean-HEAD control are compiled once to AOT executables.
Runtime measurements then interleave both executables in five independent
groups, with three warmups and twenty samples per group.  The benchmark emits
the backend's opt-in batch-plan counters so the result cannot claim a speedup
without proving that a batch was actually generated.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import subprocess
import sys
import time
from pathlib import Path
from statistics import mean, median, pstdev
from typing import Any

try:
    from scripts.benchmark_r3104_codegen import source_tree_fingerprint
except ModuleNotFoundError:  # pragma: no cover
    from benchmark_r3104_codegen import source_tree_fingerprint  # type: ignore[no-redef]


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SOURCE = ROOT / "benchmarks" / "cross-lang" / "hostcall-batch" / "spectra" / "bench.spectra"
WARMUPS = 3
TIMED_RUNS = 20
INDEPENDENT_RUNS = 5
TIMEOUT_SECONDS = 300
EXPECTED_EXIT_CODE = 0
EXPECTED_BATCH_SPEEDUP_RATIO = 0.90
STATS_RE = re.compile(
    r"r3105_hostcall_stats_(?P<label>\w+)\s+"
    r"batched_sites=(?P<batched_sites>\d+)\s+"
    r"batched_hostcalls=(?P<batched_hostcalls>\d+)\s+"
    r"fallback_hostcalls=(?P<fallback_hostcalls>\d+)\s+"
    r"argument_arena_bytes=(?P<argument_arena_bytes>\d+)\s+"
    r"result_arena_bytes=(?P<result_arena_bytes>\d+)"
)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git_value(root: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", *args], cwd=root, capture_output=True, text=True, check=False
    )
    if result.returncode != 0 or not result.stdout.strip():
        raise RuntimeError(f"unable to resolve git {' '.join(args)} in {root}")
    return result.stdout.strip()


def require_clean_control(root: Path) -> None:
    status = subprocess.run(
        ["git", "status", "--porcelain", "--untracked-files=all"],
        cwd=root,
        capture_output=True,
        text=True,
        check=False,
    )
    if status.returncode != 0:
        raise RuntimeError(f"unable to inspect clean control worktree: {root}")
    if status.stdout.strip():
        raise RuntimeError("control source root must be a clean worktree")


def run_command(
    command: list[str], *, cwd: Path | None = None, env: dict[str, str] | None = None
) -> tuple[int, int, str]:
    started = time.perf_counter_ns()
    try:
        result = subprocess.run(
            command,
            cwd=cwd,
            env=env,
            capture_output=True,
            text=True,
            timeout=TIMEOUT_SECONDS,
            check=False,
        )
    except subprocess.TimeoutExpired as exc:
        return -1, time.perf_counter_ns() - started, f"timeout after {TIMEOUT_SECONDS}s: {exc}"
    elapsed = time.perf_counter_ns() - started
    output = ((result.stdout or "") + "\n" + (result.stderr or "")).strip()
    return result.returncode, elapsed, output[-4000:]


def timing_stats(samples: list[int]) -> dict[str, Any]:
    ordered = sorted(samples)
    if not ordered:
        return {
            "median_ns": 0,
            "mean_ns": 0,
            "stddev_ns": 0,
            "min_ns": 0,
            "max_ns": 0,
            "samples_ns": [],
        }
    return {
        "median_ns": int(median(ordered)),
        "mean_ns": mean(ordered),
        "stddev_ns": pstdev(ordered) if len(ordered) > 1 else 0.0,
        "min_ns": min(ordered),
        "max_ns": max(ordered),
        "samples_ns": ordered,
    }


def parse_stats(output: str, label: str) -> dict[str, int] | None:
    matches = [match.groupdict() for match in STATS_RE.finditer(output) if match.group("label") == label]
    if not matches:
        return None
    values = matches[-1]
    return {key: int(value) for key, value in values.items() if key != "label"}


def compile_aot(binary: Path, source: Path, output: Path) -> dict[str, Any]:
    output.parent.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env["SPECTRA_R3105_STATS"] = "1"
    command = [str(binary), "compile", "--emit-exe", str(output), str(source)]
    rc, elapsed, output_tail = run_command(command, cwd=ROOT, env=env)
    return {
        "command": command,
        "exit_code": rc,
        "elapsed_ns": elapsed,
        "output_tail": output_tail,
        "output": output.relative_to(ROOT).as_posix() if output.is_relative_to(ROOT) else str(output),
        "output_sha256": sha256_file(output) if rc == 0 and output.is_file() else None,
        "batch_stats": parse_stats(output_tail, "aot"),
    }


def failed_group(rc: int, output: str, samples: list[int]) -> dict[str, Any]:
    return {
        "warmup_runs": WARMUPS,
        "timed_runs": TIMED_RUNS,
        "exit_code": rc,
        "failure_class": "timeout" if rc == -1 else "runtime",
        "output_tail": output,
        "samples_ns": samples,
        "timings": timing_stats(samples),
    }


def measure_paired_group(
    control_command: list[str], candidate_command: list[str], *, order_seed: int
) -> tuple[dict[str, Any], dict[str, Any]]:
    control_samples: list[int] = []
    candidate_samples: list[int] = []
    control_output = ""
    candidate_output = ""

    def run_pair(first_is_control: bool) -> tuple[tuple[int, int, str], tuple[int, int, str]]:
        first = control_command if first_is_control else candidate_command
        second = candidate_command if first_is_control else control_command
        first_result = run_command(first, cwd=ROOT)
        second_result = run_command(second, cwd=ROOT)
        return (
            (first_result, second_result)
            if first_is_control
            else (second_result, first_result)
        )

    for warmup in range(WARMUPS):
        control_result, candidate_result = run_pair((order_seed + warmup) % 2 == 0)
        if control_result[0] != EXPECTED_EXIT_CODE or candidate_result[0] != EXPECTED_EXIT_CODE:
            return (
                failed_group(control_result[0], control_result[2], control_samples),
                failed_group(candidate_result[0], candidate_result[2], candidate_samples),
            )

    for sample in range(TIMED_RUNS):
        control_result, candidate_result = run_pair((order_seed + sample) % 2 == 0)
        control_output = control_result[2]
        candidate_output = candidate_result[2]
        if control_result[0] != EXPECTED_EXIT_CODE or candidate_result[0] != EXPECTED_EXIT_CODE:
            return (
                failed_group(control_result[0], control_output, control_samples),
                failed_group(candidate_result[0], candidate_output, candidate_samples),
            )
        control_samples.append(control_result[1])
        candidate_samples.append(candidate_result[1])

    def successful_group(samples: list[int], command: list[str], output: str) -> dict[str, Any]:
        return {
            "warmup_runs": WARMUPS,
            "timed_runs": TIMED_RUNS,
            "exit_code": EXPECTED_EXIT_CODE,
            "failure_class": None,
            "output_tail": output,
            "command": command,
            "timings": timing_stats(samples),
        }

    return (
        successful_group(control_samples, control_command, control_output),
        successful_group(candidate_samples, candidate_command, candidate_output),
    )


def summarize(groups: list[dict[str, Any]]) -> dict[str, Any]:
    medians = [group["timings"]["median_ns"] for group in groups if group.get("exit_code") == 0]
    return {
        "groups": groups,
        "successful_independent_runs": len(medians),
        "group_medians_ns": medians,
        "median_of_group_medians_ns": int(median(medians)) if medians else 0,
    }


def capture(
    *,
    binary: Path,
    control_binary: Path,
    control_source_root: Path,
    source: Path,
    out: Path,
    independent_runs: int,
) -> dict[str, Any]:
    binary = binary.resolve()
    control_binary = control_binary.resolve()
    source = source.resolve()
    control_source_root = control_source_root.resolve()
    if not binary.is_file():
        raise RuntimeError(f"candidate release binary does not exist: {binary}")
    if not control_binary.is_file():
        raise RuntimeError(f"clean control binary does not exist: {control_binary}")
    if not source.is_file():
        raise RuntimeError(f"missing hostcall benchmark source: {source}")
    require_clean_control(control_source_root)
    candidate_revision = git_value(ROOT, "rev-parse", "HEAD")
    control_revision = git_value(control_source_root, "rev-parse", "HEAD")

    output_root = out.parent / "r3105-hostcall-benchmark"
    candidate_output = output_root / "candidate" / "hostcall-batch.exe"
    control_output = output_root / "control" / "hostcall-batch.exe"
    candidate_compile = compile_aot(binary, source, candidate_output)
    control_compile = compile_aot(control_binary, source, control_output)
    candidate_stats = candidate_compile.get("batch_stats")
    if candidate_compile["exit_code"] != 0 or control_compile["exit_code"] != 0:
        raise RuntimeError("candidate/control AOT compilation failed")
    if not isinstance(candidate_stats, dict):
        raise RuntimeError("candidate AOT compile did not emit R-3105 batch statistics")

    candidate_command = [str(candidate_output)]
    control_command = [str(control_output)]
    candidate_groups: list[dict[str, Any]] = []
    control_groups: list[dict[str, Any]] = []
    for independent in range(independent_runs):
        print(f"[r3105-hostcall] group {independent + 1}/{independent_runs}", flush=True)
        control_group, candidate_group = measure_paired_group(
            control_command, candidate_command, order_seed=independent
        )
        control_group["independent_run"] = independent + 1
        candidate_group["independent_run"] = independent + 1
        control_groups.append(control_group)
        candidate_groups.append(candidate_group)

    control_summary = summarize(control_groups)
    candidate_summary = summarize(candidate_groups)
    candidate_median = candidate_summary["median_of_group_medians_ns"]
    control_median = control_summary["median_of_group_medians_ns"]
    ratio = candidate_median / control_median if control_median else None
    return {
        "schema": "spectra.phase31.r3105_hostcall_benchmark.v1",
        "task": "R-3105",
        "classification": "benchmark_and_ir_hypothesis",
        "profiling_causal_claim": False,
        "git_revision": candidate_revision,
        "profile": "release",
        "source": source.relative_to(ROOT).as_posix() if source.is_relative_to(ROOT) else str(source),
        "source_sha256": sha256_file(source),
        "source_tree_fingerprint": source_tree_fingerprint(ROOT),
        "binary": binary.relative_to(ROOT).as_posix() if binary.is_relative_to(ROOT) else str(binary),
        "binary_sha256": sha256_file(binary),
        "control": {
            "git_revision": control_revision,
            "source_tree_fingerprint": source_tree_fingerprint(control_source_root),
            "source_root": str(control_source_root),
            "binary": str(control_binary),
            "binary_sha256": sha256_file(control_binary),
        },
        "benchmark_languages": ["spectra"],
        "java_excluded": True,
        "measurement_policy": {
            "warmup_runs": WARMUPS,
            "timed_runs": TIMED_RUNS,
            "independent_runs": independent_runs,
            "aggregation": "median_of_group_medians",
            "runtime_measurement": "precompiled_aot_executable",
        },
        "candidate_compile": candidate_compile,
        "control_compile": control_compile,
        "candidate_batch_stats": candidate_stats,
        "control_batch_stats": control_compile.get("batch_stats"),
        "candidate_runtime": candidate_summary,
        "control_runtime": control_summary,
        "candidate_to_control_ratio": ratio,
        "required_max_ratio": EXPECTED_BATCH_SPEEDUP_RATIO,
        "speedup_gate_passed": isinstance(ratio, (int, float)) and ratio <= EXPECTED_BATCH_SPEEDUP_RATIO,
        "correctness_passed": candidate_summary["successful_independent_runs"] == independent_runs,
        "control_correctness_passed": control_summary["successful_independent_runs"] == independent_runs,
    }


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", "--spectra-binary", type=Path, required=True)
    parser.add_argument("--control-binary", type=Path, required=True)
    parser.add_argument("--control-source-root", type=Path, required=True)
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--out", type=Path, default=Path("target/phase31/r3105-hostcall-benchmark.json"))
    parser.add_argument("--independent-runs", type=int, default=INDEPENDENT_RUNS)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if args.independent_runs < INDEPENDENT_RUNS:
        print("R-3105 hostcall benchmark: BLOCKED: requires five independent groups", file=sys.stderr)
        return 1
    try:
        payload = capture(
            binary=args.binary,
            control_binary=args.control_binary,
            control_source_root=args.control_source_root,
            source=args.source,
            out=args.out,
            independent_runs=args.independent_runs,
        )
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8", newline="\n")
    except (OSError, RuntimeError, ValueError) as exc:
        print(f"R-3105 hostcall benchmark: BLOCKED: {exc}", file=sys.stderr)
        return 1
    ratio = payload.get("candidate_to_control_ratio")
    print(f"R-3105 hostcall benchmark captured: candidate/control={ratio!r}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
