#!/usr/bin/env python3
"""Apply the readable Spectra syntax migration to Rust raw source fixtures."""

from __future__ import annotations

import argparse
import re
from pathlib import Path

from migrate_syntax_surface import migrate_source


RAW_START = re.compile(r"r(?P<hashes>#{0,255})\"")


def migrate_rust_text(source: str) -> str:
    output: list[str] = []
    cursor = 0

    while True:
        match = RAW_START.search(source, cursor)
        if not match:
            output.append(source[cursor:])
            break

        output.append(source[cursor : match.end()])
        hashes = match.group("hashes")
        terminator = '"' + hashes
        body_start = match.end()
        body_end = source.find(terminator, body_start)
        if body_end == -1:
            output.append(source[body_start:])
            break

        body = source[body_start:body_end]
        if re.search(r"(?m)^\s*module\s+[A-Za-z_]", body):
            output.append(migrate_source(body))
        else:
            output.append(body)
        output.append(terminator)
        cursor = body_end + len(terminator)

    return "".join(output)


def rust_files(root: Path) -> list[Path]:
    return sorted(
        path
        for path in root.rglob("*.rs")
        if not any(part in {"target", "build", "dist", "out", ".git"} for part in path.parts)
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    changed = 0
    for path in rust_files(args.root):
        original = path.read_text(encoding="utf-8")
        migrated = migrate_rust_text(original)
        if migrated == original:
            continue
        changed += 1
        if not args.check:
            path.write_text(migrated, encoding="utf-8", newline="")

    mode = "would migrate" if args.check else "migrated"
    print(f"{mode} {changed} Rust source file(s)")
    return 1 if args.check and changed else 0


if __name__ == "__main__":
    raise SystemExit(main())
