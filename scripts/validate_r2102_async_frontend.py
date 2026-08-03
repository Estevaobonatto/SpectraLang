#!/usr/bin/env python3
"""Validation gate for R-2102 async frontend syntax."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def run(command: list[str]) -> None:
    print(f"[R-2102] {' '.join(command)}")
    completed = subprocess.run(command, cwd=ROOT, text=True)
    if completed.returncode != 0:
        raise SystemExit(completed.returncode)


def require_contains(path: Path, needles: list[str]) -> None:
    text = path.read_text(encoding="utf-8")
    missing = [needle for needle in needles if needle not in text]
    if missing:
        for needle in missing:
            print(f"[R-2102] missing marker in {path}: {needle}", file=sys.stderr)
        raise SystemExit(1)


def main() -> int:
    require_contains(
        ROOT / "compiler" / "src" / "token.rs",
        ["Keyword::Async", "Keyword::Await", '"async"', '"await"'],
    )
    require_contains(
        ROOT / "compiler" / "src" / "ast" / "mod.rs",
        ["pub is_async: bool", "AsyncBlock(Block)", "Lambda {\n        is_async: bool"],
    )
    require_contains(
        ROOT / "compiler" / "tests" / "snapshots" / "parser_ast.snap",
        ["async func Private fetch() returns unit", "async_block(", "async_lambda("],
    )
    require_contains(
        ROOT / "docs" / "diagnostics" / "error-code-reference.md",
        ["`P005`", "`P006`"],
    )
    run(["cargo", "test", "-q", "-p", "spectra-compiler", "async"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
