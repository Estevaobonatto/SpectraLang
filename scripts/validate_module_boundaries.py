"""Enforce the production-module size boundary from K-10.

The current repository still contains several historical monoliths.  This
validator makes that fact measurable and prevents a release report from
silently treating the decomposition requirement as complete.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCHEMA = "spectralang.module_boundaries.v1"
SOURCE_ROOTS = (
    "compiler/src",
    "midend/src",
    "backend/src",
    "runtime/src",
    "packages/spectra-api/src",
    "packages/spectra-db/src",
    "tools/spectra-cli/src",
    "tools/spectra-lsp/src",
)
MAX_LINES = 1000


def scan(root: Path) -> dict[str, object]:
    files: list[dict[str, object]] = []
    for raw_root in SOURCE_ROOTS:
        source_root = root / raw_root
        if not source_root.is_dir():
            continue
        for path in sorted(source_root.rglob("*.rs")):
            lines = len(path.read_text(encoding="utf-8").splitlines())
            if lines > MAX_LINES:
                files.append(
                    {
                        "path": path.relative_to(root).as_posix(),
                        "lines": lines,
                        "limit": MAX_LINES,
                        "status": "decomposition_required",
                    }
                )
    return {
        "schema": SCHEMA,
        "max_lines": MAX_LINES,
        "source_roots": list(SOURCE_ROOTS),
        "violations": files,
        "violation_count": len(files),
        "status": "passed" if not files else "partial",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=".")
    parser.add_argument("--report", default=None)
    parser.add_argument("--strict", action="store_true")
    args = parser.parse_args()
    root = Path(args.root).resolve()
    report = scan(root)
    if args.report:
        report_path = (root / args.report).resolve()
        report_path.parent.mkdir(parents=True, exist_ok=True)
        report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2, sort_keys=True))
    return 1 if args.strict and report["status"] != "passed" else 0


if __name__ == "__main__":
    raise SystemExit(main())
