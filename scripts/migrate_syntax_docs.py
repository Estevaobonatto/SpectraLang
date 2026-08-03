#!/usr/bin/env python3
"""Migrate fenced Spectra examples in Markdown documentation."""

from __future__ import annotations

import argparse
from pathlib import Path

from migrate_syntax_surface import migrate_source


def migrate_markdown(source: str) -> str:
    lines = source.splitlines(keepends=True)
    output: list[str] = []
    index = 0
    while index < len(lines):
        line = lines[index]
        if line.strip().lower() != "```spectra":
            output.append(line)
            index += 1
            continue

        output.append(line)
        index += 1
        block: list[str] = []
        while index < len(lines) and lines[index].strip() != "```":
            block.append(lines[index])
            index += 1
        output.append(migrate_source("".join(block)))
        if index < len(lines):
            output.append(lines[index])
            index += 1
    return "".join(output)


def markdown_files(root: Path) -> list[Path]:
    return sorted(
        path
        for path in root.rglob("*.md")
        if not any(part in {"target", "build", "dist", "out", ".git"} for part in path.parts)
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    changed = 0
    for path in markdown_files(args.root / "docs"):
        original = path.read_text(encoding="utf-8")
        migrated = migrate_markdown(original)
        if migrated == original:
            continue
        changed += 1
        if not args.check:
            path.write_text(migrated, encoding="utf-8", newline="")

    mode = "would migrate" if args.check else "migrated"
    print(f"{mode} {changed} Markdown file(s)")
    return 1 if args.check and changed else 0


if __name__ == "__main__":
    raise SystemExit(main())
