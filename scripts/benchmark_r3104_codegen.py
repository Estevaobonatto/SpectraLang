#!/usr/bin/env python3
"""Capture the controlled R-3104 compile/codegen timing comparison."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import time
from pathlib import Path
from statistics import mean, median, pstdev
from typing import Any

try:
    from scripts.phase31_contract import SCENARIOS
except ModuleNotFoundError:  # pragma: no cover
    from phase31_contract import SCENARIOS  # type: ignore[no-redef]


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SCENARIOS = (
    "cpu-loop-sum",
    "cpu-fibs",
    "cpu-hashmap",
    "tensor-create",
    "tensor-elementwise",
    "tensor-matmul",
)
TIMING_RE = re.compile(
    r"^\s*(?:-\s*)?(front-end\s+total|lowering(?:\s+total)?|codegen(?:\s+total)?)\s*:?\s*"
    r"([0-9]+(?:\.[0-9]+)?)\s*(ns|µs|Âµs|us|ms|s)",
    re.MULTILINE | re.IGNORECASE,
)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def duration_ns(value: str, unit: str) -> float:
    unit = unit.lower().replace("â", "").replace("Â", "")
    multiplier = {"ns": 1.0, "µs": 1_000.0, "us": 1_000.0, "ms": 1_000_000.0, "s": 1_000_000_000.0}[unit]
    return float(value) * multiplier


def parse_timings(output: str) -> dict[str, float]:
    output = re.sub(r"\x1b\[[0-9;]*m", "", output)
    parsed: dict[str, float] = {}
    for name, value, unit in TIMING_RE.findall(output):
        normalized = "total" if name.lower().startswith("front-end") else name.lower().split()[0]
        parsed.setdefault(normalized, duration_ns(value, unit))
    missing = {"lowering", "codegen", "total"} - set(parsed)
    if missing:
        raise RuntimeError(f"compile --timings output is missing: {', '.join(sorted(missing))}")
    return parsed


def source_tree_fingerprint(root: Path) -> str:
    """Hash the exact revision plus the current worktree source changes.

    A Git revision alone is insufficient for a before/after comparison while
    an optimization is being developed in a dirty worktree.  Include tracked
    diffs and the contents of non-ignored untracked files so a control binary
    built from a clean checkout cannot be confused with the candidate tree.
    """
    revision = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=root, capture_output=True, check=False
    )
    if revision.returncode != 0:
        raise RuntimeError("unable to resolve source-tree Git revision")
    diff = subprocess.run(
        ["git", "diff", "--binary", "HEAD", "--", "."],
        cwd=root,
        capture_output=True,
        check=False,
    )
    if diff.returncode != 0:
        raise RuntimeError("unable to capture source-tree Git diff")
    untracked = subprocess.run(
        ["git", "ls-files", "--others", "--exclude-standard", "-z"],
        cwd=root,
        capture_output=True,
        check=False,
    )
    if untracked.returncode != 0:
        raise RuntimeError("unable to enumerate untracked source files")
    digest = hashlib.sha256()
    digest.update(revision.stdout.strip())
    digest.update(b"\0tracked-diff\0")
    digest.update(diff.stdout)
    paths = [Path(raw.decode("utf-8")) for raw in untracked.stdout.split(b"\0") if raw]
    for relative in sorted(paths, key=lambda item: item.as_posix()):
        if relative.parts and relative.parts[0].lower() == "target":
            continue
        if relative.as_posix() in {
            "docs/performance/phase31-go-comparable/evidence-r3104-codegen.json",
            "docs/performance/phase31-go-comparable/evidence-r3104-codegen.md",
        }:
            continue
        path = root / relative
        if not path.is_file():
            continue
        digest.update(b"\0untracked\0")
        digest.update(relative.as_posix().encode("utf-8"))
        digest.update(b"\0")
        digest.update(path.read_bytes())
    return digest.hexdigest()


def run_compile(binary: Path, source: Path, root: Path) -> dict[str, float]:
    started = time.perf_counter_ns()
    result = subprocess.run(
        [str(binary), "compile", "--timings", str(source)],
        cwd=root,
        capture_output=True,
        text=True,
        timeout=300,
        check=False,
    )
    elapsed = float(time.perf_counter_ns() - started)
    output = (result.stdout or "") + (result.stderr or "")
    if result.returncode != 0:
        raise RuntimeError(f"compile failed for {source}: exit={result.returncode}: {output[-2000:]}")
    timings = parse_timings(output)
    timings["process_total"] = elapsed
    return timings


def summarize(samples: list[dict[str, float]]) -> dict[str, Any]:
    metrics: dict[str, Any] = {}
    for name in ("lowering", "codegen", "total", "process_total"):
        values = [sample[name] for sample in samples]
        metrics[name] = {
            "median_ns": median(values),
            "mean_ns": mean(values),
            "stddev_ns": pstdev(values) if len(values) > 1 else 0.0,
            "min_ns": min(values),
            "max_ns": max(values),
        }
    return metrics


def capture(
    *, root: Path, source_root: Path | None = None, binary: Path, label: str, profile: str,
    scenarios: tuple[str, ...], warmups: int, samples: int
) -> dict[str, Any]:
    source_root = (source_root or root).resolve()
    binary = binary.resolve()
    if not binary.is_file():
        raise RuntimeError(f"release binary does not exist: {binary}")
    revision_result = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=source_root, capture_output=True, text=True, check=False
    )
    if revision_result.returncode != 0 or not revision_result.stdout.strip():
        raise RuntimeError("unable to resolve current Git revision")
    revision = revision_result.stdout.strip()
    scenario_results: dict[str, Any] = {}
    for scenario in scenarios:
        if scenario not in SCENARIOS:
            raise RuntimeError(f"unsupported R-3104 benchmark scenario: {scenario}")
        source = source_root / "benchmarks" / "cross-lang" / scenario / "spectra" / "bench.spectra"
        if not source.is_file():
            raise RuntimeError(f"missing Spectra fixture for {scenario}: {source}")
        for _ in range(warmups):
            run_compile(binary, source, root)
        measured = [run_compile(binary, source, root) for _ in range(samples)]
        scenario_results[scenario] = {
            "source": source.relative_to(source_root).as_posix(),
            "source_sha256": sha256_file(source),
            "warmup_runs": warmups,
            "timed_runs": samples,
            "timings": summarize(measured),
        }
    return {
        "schema": "spectra.phase31.r3104_codegen_timing.v1",
        "task": "R-3104",
        "label": label,
        "classification": "benchmark_and_ir_hypothesis",
        "profiling_causal_claim": False,
        "git_revision": revision,
        "source_tree_fingerprint": source_tree_fingerprint(source_root),
        "profile": profile,
        "binary": binary.relative_to(root).as_posix() if binary.is_relative_to(root) else str(binary),
        "binary_sha256": sha256_file(binary),
        "measurement_policy": {"warmup_runs": warmups, "timed_runs": samples},
        "scenarios": list(scenarios),
        "results": scenario_results,
    }


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--source-root", type=Path, default=ROOT)
    parser.add_argument("--profile", default="release")
    parser.add_argument("--label", choices=("before", "after"), required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--warmups", type=int, default=3)
    parser.add_argument("--samples", type=int, default=20)
    parser.add_argument("--scenarios", nargs="+", default=list(DEFAULT_SCENARIOS))
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if args.warmups < 3 or args.samples < 20:
        print("R-3104 codegen benchmark: BLOCKED: requires at least 3 warmups and 20 samples", file=sys.stderr)
        return 1
    try:
        payload = capture(
            root=ROOT,
            source_root=args.source_root,
            binary=args.binary,
            label=args.label,
            profile=args.profile,
            scenarios=tuple(args.scenarios),
            warmups=args.warmups,
            samples=args.samples,
        )
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8", newline="\n")
    except (OSError, RuntimeError, ValueError) as exc:
        print(f"R-3104 codegen benchmark: BLOCKED: {exc}", file=sys.stderr)
        return 1
    print(f"R-3104 {args.label} timing captured: {len(payload['scenarios'])} scenarios")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
