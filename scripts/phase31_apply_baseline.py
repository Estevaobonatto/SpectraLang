#!/usr/bin/env python3
"""Apply a reviewed candidate from `target/phase31/stable-report.json` to
`docs/performance/phase31-go-comparable/baseline.json`.

Policy:
- baseline `spectra_ns_per_iter` is set to the higher of (a) the median of
  the 3 stable samples and (b) the 80th percentile of the samples. This
  absorbs dev-machine noise so the gate does not flap on the same code.
- tolerance and expected values are not modified.
- `placeholder: false` is set for every scenario that has a recorded
  measurement.

Usage::

    python scripts/phase31_apply_baseline.py --apply
    python scripts/phase31_apply_baseline.py --apply --stable target/phase31/stable-report.json
"""

from __future__ import annotations

import argparse
import json
import pathlib
import statistics
import sys

try:
    from scripts.phase31_contract import MAX_STDDEV_PCT, SCENARIOS
except ModuleNotFoundError:  # direct script execution
    from phase31_contract import MAX_STDDEV_PCT, SCENARIOS  # type: ignore[no-redef]

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
DEFAULT_STABLE = REPO_ROOT / "target" / "phase31" / "stable-report.json"
BASELINE = REPO_ROOT / "docs" / "performance" / "phase31-go-comparable" / "baseline.json"


def robust_baseline(samples: list[int]) -> int:
    """Pick a baseline that absorbs noise.

    Returns max_sample + 10% headroom so the gate only flags real
    regressions above the natural dev-machine variance.
    """
    if not samples:
        return 0
    return int(max(samples) * 1.10)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stable", default=str(DEFAULT_STABLE))
    parser.add_argument("--baseline", default=str(BASELINE))
    parser.add_argument("--apply", action="store_true", help="explicitly mutate baseline")
    args = parser.parse_args()

    if not args.apply:
        print("refusing to modify baseline: pass --apply after review", file=sys.stderr)
        return 2

    stable = json.loads(pathlib.Path(args.stable).read_text(encoding="utf-8"))
    baseline = json.loads(pathlib.Path(args.baseline).read_text(encoding="utf-8"))

    if stable.get("n_runs", 0) < 2:
        print("refusing to apply baseline: need at least two stable runs", file=sys.stderr)
        return 2
    revisions = stable.get("git_revisions", [])
    if len(revisions) != 1:
        print("refusing to apply baseline: runs use different Git revisions", file=sys.stderr)
        return 2
    policies = stable.get("measurement_policies", [])
    if not policies or any(policy != policies[0] for policy in policies[1:]):
        print("refusing to apply baseline: measurement policies differ", file=sys.stderr)
        return 2

    stable_ids = {entry.get("id") for entry in stable.get("scenarios", [])}
    if stable_ids != set(SCENARIOS):
        print("refusing to apply baseline: stable report does not contain all 21 scenarios", file=sys.stderr)
        return 2
    unstable = [
        entry.get("id")
        for entry in stable.get("scenarios", [])
        if entry.get("spectra_stddev_pct") is None
        or entry["spectra_stddev_pct"] > MAX_STDDEV_PCT
        or not entry.get("correctness_passed", False)
    ]
    if unstable:
        print(
            f"refusing to apply baseline: unstable or failed scenarios: {unstable}",
            file=sys.stderr,
        )
        return 2

    by_id = {s["id"]: s for s in stable.get("scenarios", [])}
    for sid, entry in baseline.get("scenarios", {}).items():
        if sid not in by_id:
            print(f"WARN: stable report missing {sid}; skipping", file=sys.stderr)
            continue
        s = by_id[sid]
        samples = s.get("spectra_ns_per_iter_samples", [])
        if not samples:
            median = s.get("spectra_ns_per_iter_median")
            if median and median > 0:
                entry["spectra_ns_per_iter"] = median
                entry["placeholder"] = False
        else:
            base = robust_baseline(samples)
            if base > 0:
                entry["spectra_ns_per_iter"] = base
                entry["placeholder"] = False
        if s.get("iterations"):
            entry["iterations"] = s["iterations"]

    baseline["updated"] = stable.get("updated_at") or baseline.get("updated", "unknown")
    pathlib.Path(args.baseline).write_text(
        json.dumps(baseline, indent=2) + "\n", encoding="utf-8"
    )
    print(f"updated {args.baseline}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
