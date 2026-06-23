#!/usr/bin/env python3
"""Run the Phase 31 cross-language suite 3 times and pick the median result
as the next candidate baseline.

Output:
- target/phase31/stable-report.json: median numbers across 3 runs
- target/phase31/stable-run-N.json (N=1,2,3): each individual run

Usage::

    python scripts/phase31_lock_baseline.py
"""

from __future__ import annotations

import argparse
import json
import pathlib
import statistics
import subprocess
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
RUNNER = REPO_ROOT / "scripts" / "phase31_run_all.py"
TARGET = REPO_ROOT / "target" / "phase31"
N_RUNS = 3


def median_ns(samples: list[int]) -> int:
    return int(statistics.median(samples)) if samples else 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--n", type=int, default=N_RUNS, help="number of runs")
    args = parser.parse_args()
    TARGET.mkdir(parents=True, exist_ok=True)

    runs = []
    for i in range(1, args.n + 1):
        out = TARGET / f"stable-run-{i}.json"
        cmd = [
            sys.executable,
            str(RUNNER),
            "--out",
            str(out),
        ]
        print(f"[phase31-stable] run {i}/{args.n}: {' '.join(cmd)}", flush=True)
        rc = subprocess.run(cmd, check=False).returncode
        if rc != 0:
            print(f"[phase31-stable] run {i} returned {rc}", file=sys.stderr)
            return rc or 1
        runs.append(json.loads(out.read_text(encoding="utf-8")))

    # Aggregate by scenario.
    by_scenario: dict[str, list[dict]] = {}
    for r in runs:
        for s in r.get("scenarios", []):
            by_scenario.setdefault(s["id"], []).append(s)

    aggregated: list[dict] = []
    for sid, entries in by_scenario.items():
        spectra_nses = [
            e.get("results", {}).get("spectra", {}).get("ns_per_iter", 0)
            for e in entries
            if "ns_per_iter" in e.get("results", {}).get("spectra", {})
        ]
        aggregated.append({
            "id": sid,
            "category": entries[0].get("category", "unknown"),
            "iterations": entries[0].get("iterations", 0),
            "results": entries[0].get("results", {}),
            "gap_to_go": entries[0].get("gap_to_go"),
            "gap_to_rust": entries[0].get("gap_to_rust"),
            "correctness_passed": all(
                e.get("correctness_passed", False) for e in entries
            ),
            "spectra_ns_per_iter_samples": spectra_nses,
            "spectra_ns_per_iter_median": median_ns(spectra_nses),
        })

    stable = {
        "schema": "spectra.phase31.stable.v1",
        "n_runs": len(runs),
        "scenarios": aggregated,
    }
    out_path = TARGET / "stable-report.json"
    out_path.write_text(json.dumps(stable, indent=2) + "\n", encoding="utf-8")
    print(f"[phase31-stable] wrote {out_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
