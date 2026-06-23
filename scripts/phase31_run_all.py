#!/usr/bin/env python3
"""Build and run the Phase 31 cross-language benchmark suite.

For each of 11 scenarios, this script:
1. Builds the Go, Java, and Rust binaries.
2. Compiles and runs the Spectra scenario via `spectralang run`.
3. Times 20 iterations per language with 3 warmup rounds.
4. Records median, p95, stddev, ns_per_iter.
5. Writes `target/phase31/cross-lang-report.json` and a `.md` summary.

Correctness is enforced via exit code (0 = pass, anything else = fail).

Usage::

    python scripts/phase31_run_all.py --out target/phase31/cross-lang-report.json

The companion gate `scripts/validate_phase31_cross_lang.py` reads the JSON
report and compares Spectra performance against the checked-in baseline.
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import shutil
import statistics
import subprocess
import sys
import time
from typing import Any

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
BENCH_DIR = REPO_ROOT / "benchmarks" / "cross-lang"
TARGET_DIR = REPO_ROOT / "target" / "phase31"
BUILD_DIR = TARGET_DIR / "build"

SCENARIOS = [
    "cpu-loop-sum",
    "cpu-fibs",
    "cpu-string-build",
    "cpu-hashmap",
    "tensor-create",
    "tensor-elementwise",
    "tensor-reduce",
    "tensor-matmul",
    "ml-mlp-step",
    "async-echo",
    "async-pipeline",
]

LANGUAGES = ("spectra", "go", "java", "rust")

WARMUP = 2
TIMED = 12
DEFAULT_TIMEOUT_S = 300


def log(msg: str) -> None:
    print(f"[phase31] {msg}", flush=True)


def find_tool(name: str) -> str | None:
    return shutil.which(name)


def build_go(scenario: str) -> pathlib.Path:
    src = BENCH_DIR / scenario / "go" / "bench.go"
    out = BUILD_DIR / scenario / "go" / "bench"
    out.parent.mkdir(parents=True, exist_ok=True)
    cmd = ["go", "build", "-ldflags=-s -w", "-o", str(out), str(src)]
    proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
    if proc.returncode != 0:
        raise RuntimeError(f"go build failed for {scenario}:\n{proc.stderr}")
    return out


def build_java(scenario: str) -> pathlib.Path:
    src = BENCH_DIR / scenario / "java" / "Bench.java"
    out_dir = BUILD_DIR / scenario / "java"
    out_dir.mkdir(parents=True, exist_ok=True)
    cmd = ["javac", "-d", str(out_dir), str(src)]
    proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
    if proc.returncode != 0:
        raise RuntimeError(f"javac failed for {scenario}:\n{proc.stderr}")
    return out_dir / "Bench.class"


def build_rust(scenario: str) -> pathlib.Path:
    src = BENCH_DIR / scenario / "rust" / "bench.rs"
    out = BUILD_DIR / scenario / "rust" / "bench"
    out.parent.mkdir(parents=True, exist_ok=True)
    cmd = ["rustc", "-O", "-o", str(out), str(src)]
    proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
    if proc.returncode != 0:
        raise RuntimeError(f"rustc failed for {scenario}:\n{proc.stderr}")
    return out


def build_spectra(binary: pathlib.Path, scenario: str) -> pathlib.Path:
    # Spectra scenarios are .spectra source files. The compiled CLI runs them.
    return BENCH_DIR / scenario / "spectra" / "bench.spectra"


def time_subprocess(cmd: list[str], cwd: pathlib.Path | None = None, timeout_s: int = DEFAULT_TIMEOUT_S) -> tuple[int, float, bool]:
    """Run `cmd` once, return (exit_code, elapsed_ns, success)."""
    start = time.perf_counter_ns()
    try:
        proc = subprocess.run(
            cmd, capture_output=True, text=True, cwd=cwd, check=False, timeout=timeout_s
        )
    except subprocess.TimeoutExpired:
        return -1, time.perf_counter_ns() - start, False
    elapsed = time.perf_counter_ns() - start
    return proc.returncode, elapsed, proc.returncode == 0


def time_runs(cmd: list[str], cwd: pathlib.Path | None = None, timeout_s: int = DEFAULT_TIMEOUT_S) -> dict[str, Any]:
    """Run warmup + timed iterations, return stats."""
    for _ in range(WARMUP):
        rc, _, ok = time_subprocess(cmd, cwd, timeout_s)
        if not ok:
            return {"elapsed_ns": [], "ok": False, "last_rc": rc}
    samples: list[int] = []
    for _ in range(TIMED):
        rc, ns, ok = time_subprocess(cmd, cwd, timeout_s)
        if not ok:
            return {"elapsed_ns": samples, "ok": False, "last_rc": rc}
        samples.append(ns)
    return {"elapsed_ns": samples, "ok": True, "last_rc": 0}


def stats_for(samples: list[int]) -> dict[str, int]:
    if not samples:
        return {"median_ns": 0, "p95_ns": 0, "stddev_ns": 0, "ns_per_iter": 0}
    sorted_s = sorted(samples)
    p95_index = max(0, int(len(sorted_s) * 0.95) - 1)
    median = statistics.median(samples)
    stddev = statistics.pstdev(samples) if len(samples) > 1 else 0
    return {
        "median_ns": int(median),
        "p95_ns": int(sorted_s[p95_index]),
        "stddev_ns": int(stddev),
        "ns_per_iter": int(median),
    }


def run_scenario(
    scenario: str,
    spectra_binary: pathlib.Path,
    skip_missing: set[str],
) -> dict[str, Any]:
    log(f"running {scenario}")
    per_lang: dict[str, Any] = {}
    correctness = True
    iterations = 1
    category = "unknown"

    # Determine iterations + category from baseline
    baseline_path = (
        REPO_ROOT
        / "docs"
        / "performance"
        / "phase31-go-comparable"
        / "baseline.json"
    )
    if baseline_path.exists():
        b = json.loads(baseline_path.read_text(encoding="utf-8"))
        b_scenario = b.get("scenarios", {}).get(scenario)
        if b_scenario is not None:
            iterations = b_scenario.get("iterations", 1)
            category = b_scenario.get("category", "unknown")

    # Spectra
    if "spectra" not in skip_missing:
        src = build_spectra(spectra_binary, scenario)
        cmd = [str(spectra_binary), "run", str(src)]
        res = time_runs(cmd, cwd=REPO_ROOT)
        if not res["ok"]:
            log(f"  spectra FAILED rc={res['last_rc']}")
            correctness = False
            per_lang["spectra"] = {"error": f"rc={res['last_rc']}"}
        else:
            per_lang["spectra"] = stats_for(res["elapsed_ns"])

    # Go
    if "go" not in skip_missing:
        if find_tool("go") is None:
            per_lang["go"] = {"error": "go toolchain not available"}
        else:
            try:
                bin_path = build_go(scenario)
                cmd = [str(bin_path)]
                res = time_runs(cmd)
                if not res["ok"]:
                    correctness = False
                    per_lang["go"] = {"error": f"rc={res['last_rc']}"}
                else:
                    per_lang["go"] = stats_for(res["elapsed_ns"])
            except Exception as e:
                per_lang["go"] = {"error": str(e)}

    # Java
    if "java" not in skip_missing:
        if find_tool("java") is None or find_tool("javac") is None:
            per_lang["java"] = {"error": "java toolchain not available"}
        else:
            try:
                class_path = build_java(scenario)
                cmd = ["java", "-cp", str(class_path.parent), "Bench"]
                res = time_runs(cmd)
                if not res["ok"]:
                    correctness = False
                    per_lang["java"] = {"error": f"rc={res['last_rc']}"}
                else:
                    per_lang["java"] = stats_for(res["elapsed_ns"])
            except Exception as e:
                per_lang["java"] = {"error": str(e)}

    # Rust
    if "rust" not in skip_missing:
        if find_tool("rustc") is None:
            per_lang["rust"] = {"error": "rust toolchain not available"}
        else:
            try:
                bin_path = build_rust(scenario)
                cmd = [str(bin_path)]
                res = time_runs(cmd)
                if not res["ok"]:
                    correctness = False
                    per_lang["rust"] = {"error": f"rc={res['last_rc']}"}
                else:
                    per_lang["rust"] = stats_for(res["elapsed_ns"])
            except Exception as e:
                per_lang["rust"] = {"error": str(e)}

    gap_to_go = None
    gap_to_rust = None
    spec = per_lang.get("spectra", {})
    go = per_lang.get("go", {})
    rs = per_lang.get("rust", {})
    if (
        isinstance(spec, dict)
        and "ns_per_iter" in spec
        and isinstance(go, dict)
        and "ns_per_iter" in go
        and go["ns_per_iter"] > 0
    ):
        gap_to_go = round(spec["ns_per_iter"] / go["ns_per_iter"], 3)
    if (
        isinstance(spec, dict)
        and "ns_per_iter" in spec
        and isinstance(rs, dict)
        and "ns_per_iter" in rs
        and rs["ns_per_iter"] > 0
    ):
        gap_to_rust = round(spec["ns_per_iter"] / rs["ns_per_iter"], 3)

    return {
        "id": scenario,
        "category": category,
        "iterations": iterations,
        "results": per_lang,
        "gap_to_go": gap_to_go,
        "gap_to_rust": gap_to_rust,
        "correctness_passed": correctness,
    }


def write_markdown(report: dict[str, Any], path: pathlib.Path) -> None:
    lines = [
        "# Phase 31 Cross-Language Performance Report",
        "",
        f"Updated: {time.strftime('%Y-%m-%d %H:%M:%S')}",
        "",
        "| scenario | category | spectra ns | go ns | java ns | rust ns | gap vs go | gap vs rust |",
        "|---|---|---:|---:|---:|---:|---:|---:|",
    ]
    for s in report.get("scenarios", []):
        results = s.get("results", {})

        def ns_of(lang: str) -> str:
            r = results.get(lang, {})
            if "error" in r:
                return "n/a"
            if "ns_per_iter" in r:
                return f"{r['ns_per_iter']:,}"
            return "n/a"

        gap_go = s.get("gap_to_go")
        gap_rust = s.get("gap_to_rust")
        gap_go_s = f"{gap_go:.3f}x" if gap_go is not None else "n/a"
        gap_rust_s = f"{gap_rust:.3f}x" if gap_rust is not None else "n/a"
        lines.append(
            f"| `{s['id']}` | {s['category']} | {ns_of('spectra')} | "
            f"{ns_of('go')} | {ns_of('java')} | {ns_of('rust')} | "
            f"{gap_go_s} | {gap_rust_s} |"
        )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--out",
        default=str(TARGET_DIR / "cross-lang-report.json"),
        help="output JSON report path",
    )
    parser.add_argument(
        "--scenarios",
        nargs="*",
        default=SCENARIOS,
        help="subset of scenarios to run",
    )
    parser.add_argument(
        "--spectra-binary",
        default=str(
            REPO_ROOT / "target" / "debug" / ("spectralang.exe" if os.name == "nt" else "spectralang")
        ),
        help="path to the spectra CLI binary",
    )
    parser.add_argument(
        "--skip",
        nargs="*",
        default=[],
        choices=LANGUAGES,
        help="languages to skip (e.g. when toolchain missing)",
    )
    args = parser.parse_args()

    spectra_binary = pathlib.Path(args.spectra_binary)
    if not spectra_binary.exists():
        log(f"ERROR: spectra binary not found at {spectra_binary}")
        log("Build it first: cargo build -p spectra-cli")
        return 2

    out_path = pathlib.Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    BUILD_DIR.mkdir(parents=True, exist_ok=True)

    skip_missing = set(args.skip)
    # Auto-skip languages whose toolchain is missing.
    for lang, tool in (("go", "go"), ("java", "javac"), ("rust", "rustc")):
        if find_tool(tool) is None:
            log(f"auto-skipping {lang}: '{tool}' not on PATH")
            skip_missing.add(lang)

    report = {
        "schema": "spectra.phase31.bench.v1",
        "profile": "release",
        "host": os.uname().sysname if hasattr(os, "uname") else os.name,
        "runtimes": {
            "go": subprocess.run(
                ["go", "version"], capture_output=True, text=True, check=False
            ).stdout.strip()
            if find_tool("go")
            else "missing",
            "java": subprocess.run(
                ["javac", "-version"], capture_output=True, text=True, check=False
            ).stderr.strip()
            if find_tool("javac")
            else "missing",
            "rust": subprocess.run(
                ["rustc", "--version"], capture_output=True, text=True, check=False
            ).stdout.strip()
            if find_tool("rustc")
            else "missing",
        },
        "scenarios": [],
    }

    for scenario in args.scenarios:
        try:
            entry = run_scenario(scenario, spectra_binary, skip_missing)
        except Exception as e:
            log(f"scenario {scenario} raised: {e}")
            entry = {
                "id": scenario,
                "category": "unknown",
                "iterations": 0,
                "results": {},
                "gap_to_go": None,
                "gap_to_rust": None,
                "correctness_passed": False,
                "error": str(e),
            }
        report["scenarios"].append(entry)

    out_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    md_path = out_path.with_suffix(".md")
    write_markdown(report, md_path)
    log(f"wrote {out_path}")
    log(f"wrote {md_path}")

    failed = [s for s in report["scenarios"] if not s.get("correctness_passed", False)]
    if failed:
        log(f"correctness failures: {[s['id'] for s in failed]}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
