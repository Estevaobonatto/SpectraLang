#!/usr/bin/env python3
"""Comprehensive cross-language benchmark runner for SpectraLang.

Runs all 31 benchmark scenarios (21 existing + 10 new) in 3 languages
(Spectra, Go, Rust), times them, and produces a JSON + Markdown report.

Usage:
    python scripts/benchmark_full_report.py [--warmup N] [--timed N] [--out DIR]
"""
from __future__ import annotations

import argparse
import json
import os
import pathlib
import statistics
import subprocess
import sys
import time
from typing import Any

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
BENCH_DIR = REPO_ROOT / "benchmarks" / "cross-lang"
SPECTRA_BIN = REPO_ROOT / "target" / "release" / "spectralang.exe"
OUT_DIR = REPO_ROOT / "target" / "benchmark-full"
BUILD_DIR = OUT_DIR / "build"

ALL_SCENARIOS = [
    # 21 existing (Phase 31)
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
    "sort-int",
    "binary-search",
    "sieve",
    "matrix-transpose",
    "string-reverse",
    "count-primes",
    "gcd",
    "pow-fast",
    "word-count",
    "digit-sum",
    # 10 new (R-3101 add-on suite)
    "quicksort",
    "mergesort",
    "json-parse",
    "base64-encode",
    "lru-cache",
    "hashmap-churn",
    "matrix-multiply-naive",
    "collatz",
    "concurrent-fanout",
    "producer-consumer-bounded",
]
LANGUAGES = ("spectra", "go", "rust")
WARMUP_DEFAULT = 1
TIMED_DEFAULT = 5
DEFAULT_TIMEOUT_S = 300


def log(msg: str) -> None:
    print(f"[bench-full] {msg}", flush=True)


def find_tool(name: str) -> str | None:
    from shutil import which
    found = which(name)
    if found:
        return found
    # On Windows, shutil.which requires the .exe extension to be in PATH.
    # Fall back to the known cargo bin location.
    fallback = pathlib.Path.home() / ".cargo" / "bin" / f"{name}.exe"
    if fallback.exists():
        return str(fallback)
    return None


def build_go(scenario: str) -> pathlib.Path:
    src = BENCH_DIR / scenario / "go" / "bench.go"
    out = BUILD_DIR / scenario / "go" / "bench.exe"
    out.parent.mkdir(parents=True, exist_ok=True)
    proc = subprocess.run(
        ["go", "build", "-ldflags=-s -w", "-o", str(out), str(src)],
        capture_output=True, text=True, check=False,
    )
    if proc.returncode != 0:
        raise RuntimeError(f"go build failed for {scenario}:\n{proc.stderr}")
    return out


def build_rust(scenario: str) -> pathlib.Path:
    src = BENCH_DIR / scenario / "rust" / "bench.rs"
    out = BUILD_DIR / scenario / "rust" / "bench.exe"
    out.parent.mkdir(parents=True, exist_ok=True)
    rustc = find_tool("rustc")
    if rustc is None:
        raise RuntimeError("rustc not found")
    proc = subprocess.run(
        [rustc, "-O", "-o", str(out), str(src)],
        capture_output=True, text=True, check=False,
    )
    if proc.returncode != 0:
        raise RuntimeError(f"rustc failed for {scenario}:\n{proc.stderr}")
    return out


def time_subprocess(cmd: list[str], cwd: pathlib.Path | None = None,
                    timeout_s: int = DEFAULT_TIMEOUT_S) -> tuple[int, int, bool]:
    """Run `cmd` once, return (exit_code, elapsed_ns, success)."""
    start = time.perf_counter_ns()
    try:
        proc = subprocess.run(
            cmd, capture_output=True, text=True, cwd=cwd, check=False, timeout=timeout_s,
        )
        elapsed = time.perf_counter_ns() - start
        return proc.returncode, elapsed, proc.returncode == 0
    except subprocess.TimeoutExpired:
        return -1, time.perf_counter_ns() - start, False


def time_runs(cmd: list[str], cwd: pathlib.Path | None, warmup: int, timed: int,
              timeout_s: int) -> dict[str, Any]:
    for _ in range(warmup):
        rc, _, ok = time_subprocess(cmd, cwd, timeout_s)
        if not ok:
            return {"elapsed_ns": [], "ok": False, "last_rc": rc}
    samples: list[int] = []
    for _ in range(timed):
        rc, ns, ok = time_subprocess(cmd, cwd, timeout_s)
        if not ok:
            return {"elapsed_ns": samples, "ok": False, "last_rc": rc}
        samples.append(ns)
    return {"elapsed_ns": samples, "ok": True, "last_rc": 0}


def stats_for(samples: list[int]) -> dict[str, int]:
    if not samples:
        return {"median_ns": 0, "p95_ns": 0, "stddev_ns": 0, "ns_per_iter": 0,
                "min_ns": 0, "max_ns": 0}
    s = sorted(samples)
    p95_index = max(0, int(len(s) * 0.95) - 1)
    median = statistics.median(s)
    stddev = statistics.pstdev(s) if len(s) > 1 else 0
    return {
        "median_ns": int(median),
        "p95_ns": int(s[p95_index]),
        "stddev_ns": int(stddev),
        "ns_per_iter": int(median),
        "min_ns": int(min(s)),
        "max_ns": int(max(s)),
    }


def run_scenario(scenario: str, warmup: int, timed: int,
                 skip_missing: set[str]) -> dict[str, Any]:
    log(f"running {scenario}")
    per_lang: dict[str, Any] = {}
    correctness = True

    # Spectra
    if "spectra" not in skip_missing:
        if not SPECTRA_BIN.exists():
            per_lang["spectra"] = {"error": f"spectra binary not found at {SPECTRA_BIN}"}
        else:
            src = BENCH_DIR / scenario / "spectra" / "bench.spectra"
            if not src.exists():
                per_lang["spectra"] = {"error": f"spectra source not found at {src}"}
            else:
                cmd = [str(SPECTRA_BIN), "run", str(src)]
                res = time_runs(cmd, REPO_ROOT, warmup, timed, DEFAULT_TIMEOUT_S)
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
                res = time_runs(cmd, None, warmup, timed, DEFAULT_TIMEOUT_S)
                if not res["ok"]:
                    correctness = False
                    per_lang["go"] = {"error": f"rc={res['last_rc']}"}
                else:
                    per_lang["go"] = stats_for(res["elapsed_ns"])
            except Exception as e:
                per_lang["go"] = {"error": str(e)}

    # Rust
    if "rust" not in skip_missing:
        if find_tool("rustc") is None:
            per_lang["rust"] = {"error": "rustc toolchain not available"}
        else:
            try:
                bin_path = build_rust(scenario)
                cmd = [str(bin_path)]
                res = time_runs(cmd, None, warmup, timed, DEFAULT_TIMEOUT_S)
                if not res["ok"]:
                    correctness = False
                    per_lang["rust"] = {"error": f"rc={res['last_rc']}"}
                else:
                    per_lang["rust"] = stats_for(res["elapsed_ns"])
            except Exception as e:
                per_lang["rust"] = {"error": str(e)}

    # Compute gaps vs Go / Rust
    def gap_to(lang: str) -> float | None:
        spec = per_lang.get("spectra", {})
        other = per_lang.get(lang, {})
        if not isinstance(spec, dict) or "ns_per_iter" not in spec:
            return None
        if not isinstance(other, dict) or "ns_per_iter" not in other:
            return None
        if other["ns_per_iter"] <= 0:
            return None
        return round(spec["ns_per_iter"] / other["ns_per_iter"], 3)

    return {
        "id": scenario,
        "results": per_lang,
        "gap_to_go": gap_to("go"),
        "gap_to_rust": gap_to("rust"),
        "correctness_passed": correctness,
    }


def write_markdown(report: dict[str, Any], path: pathlib.Path) -> None:
    lines: list[str] = []
    lines.append("# SpectraLang Cross-Language Benchmark Report")
    lines.append("")
    lines.append(f"Updated: {time.strftime('%Y-%m-%d %H:%M:%S')}")
    lines.append("")
    lines.append(f"Spectra binary: `target/release/spectralang.exe` (release profile)")
    lines.append(f"Warmup runs: {report['warmup']}, Timed runs: {report['timed']}")
    lines.append(f"Scenarios: {len(report['scenarios'])}")
    lines.append("")
    lines.append("## Summary table (median ns/iter, lower is better)")
    lines.append("")
    lines.append("| # | scenario | spectra ns | go ns | rust ns | vs go | vs rust |")
    lines.append("|---|---|---:|---:|---:|---:|---:|")
    for i, s in enumerate(report["scenarios"], 1):
        r = s["results"]
        def cell(k):
            v = r.get(k, {})
            if isinstance(v, dict) and "ns_per_iter" in v:
                return f"{v['ns_per_iter']:,}"
            return "—"
        def g(k):
            v = s.get(k)
            return f"{v:.2f}x" if v is not None else "—"
        lines.append(f"| {i} | `{s['id']}` | {cell('spectra')} | {cell('go')} | {cell('rust')} | {g('gap_to_go')} | {g('gap_to_rust')} |")
    lines.append("")
    lines.append("## Spectra vs Go baseline (sorted by gap)")
    lines.append("")
    lines.append("| scenario | spectra ns | go ns | gap |")
    lines.append("|---|---:|---:|---:|")
    gap_rows = []
    for s in report["scenarios"]:
        gap = s.get("gap_to_go")
        spec = s["results"].get("spectra", {})
        go = s["results"].get("go", {})
        if gap is not None and isinstance(spec, dict) and isinstance(go, dict):
            gap_rows.append((s["id"], spec["ns_per_iter"], go["ns_per_iter"], gap))
    gap_rows.sort(key=lambda x: x[3])
    for sid, spec_ns, go_ns, gap in gap_rows:
        lines.append(f"| `{sid}` | {spec_ns:,} | {go_ns:,} | {gap:.2f}x |")
    lines.append("")
    lines.append("## Detailed per-scenario stats")
    lines.append("")
    lines.append("| scenario | lang | median ns | p95 ns | min ns | max ns | stddev ns |")
    lines.append("|---|---|---:|---:|---:|---:|---:|")
    for s in report["scenarios"]:
        for lang in LANGUAGES:
            r = s["results"].get(lang, {})
            if isinstance(r, dict) and "median_ns" in r:
                lines.append(f"| `{s['id']}` | {lang} | {r['median_ns']:,} | {r['p95_ns']:,} | {r['min_ns']:,} | {r['max_ns']:,} | {r['stddev_ns']:,} |")
            else:
                err = r.get("error", "—") if isinstance(r, dict) else "—"
                lines.append(f"| `{s['id']}` | {lang} | ERR: {err} | | | | |")
    lines.append("")
    path.write_text("\n".join(lines), encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--warmup", type=int, default=WARMUP_DEFAULT)
    parser.add_argument("--timed", type=int, default=TIMED_DEFAULT)
    parser.add_argument("--out", type=str, default=str(OUT_DIR))
    parser.add_argument("--scenarios", type=str, nargs="*", default=None,
                        help="Subset of scenarios to run (default: all 31)")
    parser.add_argument("--skip", type=str, nargs="*", default=[],
                        help="Languages to skip (spectra, go, rust)")
    args = parser.parse_args()

    out_dir = pathlib.Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)
    build_dir = out_dir / "build"
    build_dir.mkdir(parents=True, exist_ok=True)

    scenarios = args.scenarios or ALL_SCENARIOS
    skip_missing = set(args.skip)

    # Pre-build spectra if missing
    if "spectra" not in skip_missing and not SPECTRA_BIN.exists():
        log(f"spectra binary missing at {SPECTRA_BIN}, building...")
        subprocess.run(["cargo", "build", "-p", "spectra-cli", "--release"],
                       cwd=REPO_ROOT, check=True)

    log(f"Running {len(scenarios)} scenarios x 3 languages, warmup={args.warmup}, timed={args.timed}")
    started = time.time()
    results = []
    for s in scenarios:
        results.append(run_scenario(s, args.warmup, args.timed, skip_missing))
    elapsed = time.time() - started

    report = {
        "warmup": args.warmup,
        "timed": args.timed,
        "elapsed_s": round(elapsed, 1),
        "scenarios": results,
    }

    json_path = out_dir / "benchmark-full-report.json"
    json_path.write_text(json.dumps(report, indent=2), encoding="utf-8")
    log(f"JSON: {json_path}")

    md_path = out_dir / "benchmark-full-report.md"
    write_markdown(report, md_path)
    log(f"Markdown: {md_path}")

    # Print summary
    total = len(results)
    passed = sum(1 for s in results if s["correctness_passed"])
    print()
    print(f"=== {total} scenarios, {passed} passed correctness ===")
    print(f"Total elapsed: {elapsed:.1f}s")
    return 0 if passed == total else 1


if __name__ == "__main__":
    sys.exit(main())
