#!/usr/bin/env python3
"""Run Phase 31 repeatedly and write a read-only candidate baseline report.

Output:
- target/phase31/stable-report.json: median numbers across 3 runs
- target/phase31/stable-run-N.json (N=1,2,3): each individual run

This script never edits baseline.json. Applying a candidate requires explicit
review through phase31_apply_baseline.py --apply.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import statistics
import subprocess
import sys

try:
    from scripts.phase31_contract import SCENARIOS
except ModuleNotFoundError:  # direct script execution
    from phase31_contract import SCENARIOS  # type: ignore[no-redef]

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
RUNNER = REPO_ROOT / "scripts" / "phase31_run_all.py"
TARGET = REPO_ROOT / "target" / "phase31"
N_RUNS = 3


def median_ns(samples: list[int]) -> int:
    return int(statistics.median(samples)) if samples else 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--n", type=int, default=N_RUNS, help="number of runs")
    parser.add_argument("--binary", default="target/debug/spectralang.exe")
    parser.add_argument("--profile", choices=("debug", "release"), default="debug")
    parser.add_argument("--independent-runs", type=int, default=3)
    parser.add_argument("--confirm-regressions", type=int, default=2)
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
            "--spectra-binary",
            args.binary,
            "--spectra-profile",
            args.profile,
            "--independent-runs",
            str(args.independent_runs),
            "--confirm-regressions",
            str(args.confirm_regressions),
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
            "spectra_stddev_pct": (
                statistics.pstdev(spectra_nses) / median_ns(spectra_nses) * 100.0
                if len(spectra_nses) > 1 and median_ns(spectra_nses) > 0
                else None
            ),
        })

    stable = {
        "schema": "spectra.phase31.stable.v1",
        "n_runs": len(runs),
        "profile": runs[0].get("profile"),
        "spectra_binary": runs[0].get("spectra_binary"),
        "git_revisions": sorted({r.get("git_revision") for r in runs}),
        "measurement_policies": [r.get("measurement_policy") for r in runs],
        "scenarios": aggregated,
    }
    out_path = TARGET / "stable-report.json"
    out_path.write_text(json.dumps(stable, indent=2) + "\n", encoding="utf-8")
    print(f"[phase31-stable] wrote {out_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
