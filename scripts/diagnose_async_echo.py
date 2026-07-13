"""Decompose async-echo cost without modifying Phase 31 baseline.

The fixtures are generated under target/ and executed through the real CLI.
This isolates process startup, reset, and task slot operations while preserving
the same 1,000 x 10 workload used by async-echo.
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import platform
import statistics
import subprocess
import shutil
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT_DIR = ROOT / "target" / "phase31" / "async-echo-diagnostics"
WARMUPS = 3
SAMPLES = 20
OUTER = 1000
INNER = 10

FIXTURES = {
    "startup": "pub fn main() -> int { return 0; }",
    "reset-only": f"""module async_echo_reset_only;
import std.concurrent as concurrent;
pub fn main() -> int {{
    let i = 0;
    while i < {OUTER * INNER} {{ concurrent.reset(); i = i + 1; }}
    return 0;
}}""",
    "spawn-only": f"""module async_echo_spawn_only;
import std.concurrent as concurrent;
pub fn main() -> int {{
    let i = 0;
    while i < {OUTER} {{
        concurrent.reset();
        let k = 0;
        while k < {INNER} {{ concurrent.task_spawn(k + 1); k = k + 1; }}
        i = i + 1;
    }}
    return 0;
}}""",
    "join-only": f"""module async_echo_join_only;
import std.concurrent as concurrent;
pub fn main() -> int {{
    let i = 0;
    while i < {OUTER} {{
        concurrent.reset();
        let k = 0;
        while k < {INNER} {{ concurrent.task_spawn(k + 1); k = k + 1; }}
        k = 1;
        while k <= {INNER} {{
            concurrent.task_join(k);
            k = k + 1;
        }}
        i = i + 1;
    }}
    return 0;
}}""",
    "spawn-join": f"""module async_echo_spawn_join;
import std.concurrent as concurrent;
pub fn main() -> int {{
    let i = 0;
    while i < {OUTER} {{
        let k = 0;
        while k < {INNER} {{
            let task = concurrent.task_spawn(k + 1);
            if concurrent.task_join(task) != k + 1 {{ return 1; }}
            k = k + 1;
        }}
        i = i + 1;
    }}
    return 0;
}}""",
    "fused": f"""module async_echo_fused;
import std.concurrent as concurrent;
pub fn main() -> int {{
    let i = 0;
    let total = 0;
    while i < {OUTER} {{
        let k = 0;
        while k < {INNER} {{
            total = total + concurrent.task_join(concurrent.task_spawn(k + 1));
            k = k + 1;
        }}
        i = i + 1;
    }}
    if total != {OUTER * 55} {{ return 1; }}
    return 0;
}}""",
    "full": f"""module async_echo_full;
import std.concurrent as concurrent;
pub fn main() -> int {{
    let i = 0;
    while i < {OUTER} {{
        concurrent.reset();
        let k = 0;
        while k < {INNER} {{
            let task = concurrent.task_spawn(k + 1);
            if concurrent.task_join(task) != k + 1 {{ return 1; }}
            k = k + 1;
        }}
        i = i + 1;
    }}
    return 0;
}}""",
}


def run_once(cmd: list[str], timeout_s: int, diagnostics: bool = False) -> tuple[int, int, str, dict | None]:
    start = time.perf_counter_ns()
    env = os.environ.copy()
    if diagnostics:
        env["SPECTRA_CONCURRENT_DIAGNOSTICS"] = "1"
    try:
        proc = subprocess.run(
            cmd, cwd=ROOT, capture_output=True, text=True, check=False,
            timeout=timeout_s, env=env
        )
    except subprocess.TimeoutExpired as exc:
        return -1, time.perf_counter_ns() - start, str(exc), None
    output = ((proc.stdout or "") + "\n" + (proc.stderr or "")).strip()
    diagnostics_report = None
    for line in output.splitlines():
        if line.startswith("SPECTRA_CONCURRENT_DIAGNOSTICS="):
            try:
                diagnostics_report = json.loads(line.split("=", 1)[1])
            except json.JSONDecodeError:
                diagnostics_report = {"parse_error": True}
    return proc.returncode, time.perf_counter_ns() - start, output[-4000:], diagnostics_report


def measure(cmd: list[str], timeout_s: int, diagnostics: bool = False) -> dict:
    diagnostic_samples: list[dict] = []
    for _ in range(WARMUPS):
        rc, _, output, _ = run_once(cmd, timeout_s, diagnostics)
        if rc != 0:
            return {"ok": False, "exit_code": rc, "output_tail": output, "samples_ns": []}
    samples: list[int] = []
    for _ in range(SAMPLES):
        rc, elapsed, output, diagnostic = run_once(cmd, timeout_s, diagnostics)
        if rc != 0:
            return {"ok": False, "exit_code": rc, "output_tail": output, "samples_ns": samples}
        samples.append(elapsed)
        if diagnostic is not None:
            diagnostic_samples.append(diagnostic)
    median = int(statistics.median(samples))
    stddev = int(statistics.pstdev(samples)) if len(samples) > 1 else 0
    sorted_samples = sorted(samples)
    p95 = sorted_samples[max(0, int(len(sorted_samples) * 0.95) - 1)]
    return {
        "ok": True,
        "exit_code": 0,
        "samples_ns": samples,
        "median_ns": median,
        "p95_ns": p95,
        "stddev_ns": stddev,
        "stddev_pct": round(stddev / median * 100, 3) if median else 0.0,
        "ns_per_task_pair": round(median / (OUTER * INNER), 2),
        "output_tail": "",
        "diagnostics": diagnostic_samples[-1] if diagnostic_samples else None,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--profile", choices=("debug", "release"), required=True)
    parser.add_argument("--timeout", type=int, default=300)
    parser.add_argument("--out", default=str(OUT_DIR / "report.json"))
    parser.add_argument("--go-binary", help="Optional prebuilt Go benchmark binary.")
    args = parser.parse_args()

    binary = pathlib.Path(args.binary)
    if not binary.exists():
        print(f"async-echo diagnostics: binary not found: {binary}", file=sys.stderr)
        return 2
    inferred = "release" if "release" in binary.parts else "debug"
    if inferred != args.profile:
        print(f"async-echo diagnostics: profile {args.profile} does not match {binary}", file=sys.stderr)
        return 2

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    revision = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=False
    ).stdout.strip() or "unknown"
    report = {
        "schema": "spectra.phase31.async_echo_diagnostics.v1",
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "git_revision": revision,
        "spectra_binary": str(binary.resolve()),
        "profile": args.profile,
        "reference_runtime": "go",
        "host": {"platform": platform.platform(), "processor": platform.processor()},
        "workload": {"outer": OUTER, "inner": INNER, "task_pairs": OUTER * INNER},
        "expected_diagnostics": {
            "fused_fast_abi_calls": OUTER * INNER,
            "task_slots_created_by_fused_path": 0,
            "tasks_counted": OUTER * INNER,
        },
        "measurement_policy": {"warmup_runs": WARMUPS, "timed_runs": SAMPLES},
        "variants": {},
    }
    for name, source in FIXTURES.items():
        fixture = OUT_DIR / f"{name}.spectra"
        fixture.write_text(source + "\n", encoding="utf-8")
        cmd = [str(binary.resolve()), "run", str(fixture.resolve())]
        result = measure(cmd, args.timeout, diagnostics=name in {"spawn-only", "join-only", "spawn-join", "fused", "full"})
        result["command"] = cmd
        result["fixture"] = str(fixture.relative_to(ROOT))
        report["variants"][name] = result
        print(f"[async-echo] {name}: {result.get('median_ns', 'FAILED')} ns", flush=True)

    go_binary = pathlib.Path(args.go_binary) if args.go_binary else ROOT / "target" / "phase31" / "build" / "async-echo" / "go" / "bench.exe"
    if not go_binary.exists() and shutil.which("go"):
        go_binary.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(["go", "build", "-o", str(go_binary), str(ROOT / "benchmarks" / "cross-lang" / "async-echo" / "go" / "bench.go")], cwd=ROOT, check=True)
    if go_binary.exists():
        go_result = measure([str(go_binary.resolve())], args.timeout)
        report["go"] = {"binary": str(go_binary.resolve()), **go_result}
        full = report["variants"].get("full", {})
        if go_result.get("median_ns") and full.get("median_ns"):
            report["comparison"] = {
                "reference_runtime": "go",
                "spectra_median_ns": full["median_ns"],
                "go_median_ns": go_result["median_ns"],
                "gap_to_go_pct": round((full["median_ns"] / go_result["median_ns"] - 1.0) * 100.0, 3),
                "stddev_pct": full.get("stddev_pct", 0.0),
            }

    output = pathlib.Path(args.out)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    failed = [name for name, result in report["variants"].items() if not result["ok"]]
    print(f"async-echo diagnostics: report={output}")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
