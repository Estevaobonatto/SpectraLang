#!/usr/bin/env python3
"""Migrate the repository source corpus to Spectra's readable syntax surface.

This is deliberately a small lexical migration tool rather than a collection
of regular expressions over arbitrary text.  Strings, character literals, and
line comments are copied verbatim; only language tokens are rewritten.
"""

from __future__ import annotations

import argparse
import re
from pathlib import Path


WORD_RE = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
NAMED_IMPORT_RE = re.compile(
    r"^(?P<indent>\s*)(?P<visibility>(?:pub|public)\s+)?"
    r"import\s*\{(?P<names>[^{}]*)\}\s+from\s+"
    r"(?P<path>[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*)"
    r"\s*;?(?P<comment>\s*//.*)?$"
)

WORD_REPLACEMENTS = {
    "fn": "func",
    "struct": "record",
    "pub": "public",
    "elif": "else if",
    "elseif": "else if",
    "of": "in",
}

# An intermediate marker lets us identify old match arrows without touching
# existing `then` words or control-flow examples using `case ... => ...`.
MATCH_ARROW = "__SPECTRA_MATCH_ARROW__"
SWITCH_CASE_THEN_RE = re.compile(r"(?m)^(?P<indent>\s*)case (?P<pattern>.+?)\s+then(?P<tail>\s*\{)")
SWITCH_DEFAULT_THEN_RE = re.compile(
    r"(?m)^(?P<indent>\s*)(?:when\s+)?else\s+then(?P<tail>\s*\{)"
)


def migrate_named_import_line(line: str) -> str:
    match = NAMED_IMPORT_RE.match(line.rstrip("\r\n"))
    if not match:
        return line

    visibility = match.group("visibility") or ""
    visibility = "public " if visibility.strip() in {"pub", "public"} else ""
    comment = match.group("comment") or ""
    names = ", ".join(part.strip() for part in match.group("names").split(","))
    return (
        f"{match.group('indent')}{visibility}from {match.group('path')} "
        f"import {names}{comment}\n"
    )


def migrate_multiline_named_imports(source: str) -> str:
    lines = source.splitlines(keepends=True)
    migrated: list[str] = []
    index = 0
    start_re = re.compile(
        r"^(?P<indent>\s*)(?P<visibility>(?:pub|public)\s+)?import\s*\{\s*$"
    )
    end_re = re.compile(
        r"^\s*\}\s*from\s+(?P<path>[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*)\s*;?\s*$"
    )

    while index < len(lines):
        start = start_re.match(lines[index].rstrip("\r\n"))
        if not start:
            migrated.append(lines[index])
            index += 1
            continue

        names: list[str] = []
        cursor = index + 1
        end = None
        while cursor < len(lines):
            closing = end_re.match(lines[cursor].rstrip("\r\n"))
            if closing:
                end = closing
                break
            item = lines[cursor].strip()
            if item:
                names.append(item.rstrip(","))
            cursor += 1

        if end is None:
            migrated.append(lines[index])
            index += 1
            continue

        visibility = "public " if (start.group("visibility") or "").strip() in {"pub", "public"} else ""
        joined_names = ", ".join(names)
        migrated.append(
            f"{start.group('indent')}{visibility}from {end.group('path')} "
            f"import {joined_names}\n"
        )
        index = cursor + 1

    return "".join(migrated)


def rewrite_tokens(source: str) -> str:
    output: list[str] = []
    index = 0
    length = len(source)
    square_depth = 0

    while index < length:
        char = source[index]

        # Line comments are not language tokens for this migration.
        if char == "/" and index + 1 < length and source[index + 1] == "/":
            end = source.find("\n", index)
            if end == -1:
                output.append(source[index:])
                break
            output.append(source[index:end])
            index = end
            continue

        # Copy strings and character literals exactly, including f-string
        # contents.  The lexer accepts escaped delimiters, which this scanner
        # handles the same way.
        if char in {'"', "'"}:
            delimiter = char
            start = index
            index += 1
            escaped = False
            while index < length:
                current = source[index]
                index += 1
                if escaped:
                    escaped = False
                elif current == "\\":
                    escaped = True
                elif current == delimiter:
                    break
            output.append(source[start:index])
            continue

        if source.startswith("->", index):
            previous = output[-1][-1:] if output else ""
            following = source[index + 2] if index + 2 < length else ""
            left = "returns" if previous.isspace() else " returns"
            right = "" if following.isspace() else " "
            output.append(left + right)
            index += 2
            continue

        if source.startswith("=>", index):
            output.append(MATCH_ARROW)
            index += 2
            continue

        if source.startswith("..", index):
            cursor = index + 2
            while cursor < length and source[cursor] in " \t":
                cursor += 1
            if cursor < length and source[cursor] == "=":
                output.append("..=")
                index = cursor + 1
                continue

        if char == "!" and not source.startswith("!=", index):
            output.append("not ")
            index += 1
            continue

        if char == ";":
            if square_depth > 0:
                output.append(char)
                index += 1
                continue

            # A semicolon immediately before a newline is simply removed.  A
            # same-line statement separator becomes a real line break.  Keep
            # an inline comment attached to the preceding statement.
            next_char = source[index + 1] if index + 1 < length else ""
            remainder = source[index + 1 :]
            comment_match = re.match(r"[ \t]*//", remainder)
            if comment_match:
                output.append("")
            else:
                output.append("" if next_char in {"", "\r", "\n"} else "\n")
            index += 1
            continue

        word = WORD_RE.match(source, index)
        if word:
            value = word.group(0)
            replacement = WORD_REPLACEMENTS.get(value, value)
            if value == "unless":
                replacement = "if not"
            output.append(replacement)
            index = word.end()
            continue

        if char == "[":
            square_depth += 1
        elif char == "]" and square_depth > 0:
            square_depth -= 1

        output.append(char)
        index += 1

    return "".join(output)


def normalize_match_arrows(source: str) -> str:
    lines: list[str] = []
    for raw_line in source.splitlines(keepends=True):
        newline = "\n" if raw_line.endswith("\n") else ""
        line = raw_line[:-1] if newline else raw_line
        marker_index = line.find(MATCH_ARROW)
        if marker_index == -1:
            lines.append(line + newline)
            continue

        before = line[:marker_index]
        after = line[marker_index + len(MATCH_ARROW) :]
        stripped = before.strip()
        indent = before[: len(before) - len(before.lstrip())]

        # `switch` arms use a colon; match arms use the readable `then` word.
        if stripped.startswith("case "):
            lines.append(indent + before[len(indent) :] + ":" + after + newline)
            continue
        if stripped.startswith("else "):
            lines.append(indent + before[len(indent) :] + ":" + after + newline)
            continue
        if stripped.startswith(("when ", "otherwise ")):
            lines.append(indent + before[len(indent) :] + "then" + after + newline)
            continue

        pattern = stripped
        if pattern == "_":
            migrated = f"{indent}otherwise then{after}"
        elif pattern in {"default", "otherwise"}:
            migrated = f"{indent}otherwise then{after}"
        else:
            migrated = f"{indent}when {pattern} then{after}"
        lines.append(migrated + newline)

    return "".join(lines).replace(MATCH_ARROW, "then")


def migrate_source(source: str) -> str:
    source = migrate_multiline_named_imports(source)
    normalized_lines = [migrate_named_import_line(line) for line in source.splitlines(keepends=True)]
    migrated = rewrite_tokens("".join(normalized_lines))
    migrated = normalize_match_arrows(migrated)
    migrated = SWITCH_CASE_THEN_RE.sub(
        lambda match: f"{match.group('indent')}case {match.group('pattern')}:"
        f"{match.group('tail')}",
        migrated,
    )
    migrated = SWITCH_DEFAULT_THEN_RE.sub(
        lambda match: f"{match.group('indent')}else:{match.group('tail')}",
        migrated,
    )
    return migrated


def source_files(root: Path) -> list[Path]:
    return sorted(
        path
        for suffix in ("*.spectra", "*.spc")
        for path in root.rglob(suffix)
        if not any(part in {"target", "build", "dist", "out", ".git"} for part in path.parts)
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--check", action="store_true", help="report files that need migration")
    args = parser.parse_args()

    changed = 0
    for path in source_files(args.root):
        original = path.read_text(encoding="utf-8")
        migrated = migrate_source(original)
        if migrated == original:
            continue
        changed += 1
        if not args.check:
            path.write_text(migrated, encoding="utf-8", newline="")

    mode = "would migrate" if args.check else "migrated"
    print(f"{mode} {changed} Spectra source file(s)")
    return 1 if args.check and changed else 0


if __name__ == "__main__":
    raise SystemExit(main())
