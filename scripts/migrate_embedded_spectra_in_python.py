#!/usr/bin/env python3
"""Migrate Spectra source snippets embedded in Python validation scripts.

The repository has a number of validators that materialize temporary
``.spectra`` projects.  This tool edits only Python string literals that look
like Spectra source or canonical-syntax markers; Python code and Rust API
markers remain unchanged.
"""

from __future__ import annotations

import argparse
import ast
import io
import re
import tokenize
from pathlib import Path

from migrate_syntax_surface import migrate_source


STRING_PREFIX_RE = re.compile(r"(?i)(?P<prefix>[rubf]*)(?P<quote>'''|\"\"\"|'|\")")


def looks_like_spectra(token_text: str) -> bool:
    match = STRING_PREFIX_RE.match(token_text)
    candidate = token_text
    if match:
        candidate = token_text[match.end() : -len(match.group("quote"))]
    return bool(
        re.search(
            r"(?m)^\s*(?:module\s+[A-Za-z_]|import\s*\{)",
            candidate,
        )
    )


def looks_like_legacy_spectra_fragment(token_text: str) -> bool:
    return bool(
        ";" in token_text
        or re.search(r"\b(?:fn|struct|pub|elif|elseif|unless|of)\b", token_text)
        or "->" in token_text
    )


def migrate_string_token(token_text: str) -> str:
    match = STRING_PREFIX_RE.match(token_text)
    if not match:
        return token_text

    prefix = match.group("prefix")
    quote = match.group("quote")
    if "b" in prefix.lower():
        return token_text

    body_start = match.end()
    body_end = len(token_text) - len(quote)
    body = token_text[body_start:body_end]

    if "f" in prefix.lower():
        migrated_body = migrate_source(body)
        return f"{prefix}{quote}{migrated_body}{quote}"

    try:
        value = ast.literal_eval(token_text)
    except (SyntaxError, ValueError):
        return token_text
    if not isinstance(value, str):
        return token_text
    migrated = migrate_source(value)
    return repr(migrated)


def line_offsets(source: str) -> list[int]:
    offsets = [0]
    for line in source.splitlines(keepends=True):
        offsets.append(offsets[-1] + len(line))
    return offsets


def absolute_offset(offsets: list[int], position: tuple[int, int]) -> int:
    line, column = position
    return offsets[line - 1] + column


def migrate_python_file(path: Path) -> tuple[str, int]:
    source = path.read_text(encoding="utf-8")
    offsets = line_offsets(source)
    replacements: list[tuple[int, int, str]] = []
    fstring_start = getattr(tokenize, "FSTRING_START", None)
    fstring_middle = getattr(tokenize, "FSTRING_MIDDLE", None)
    fstring_end = getattr(tokenize, "FSTRING_END", None)
    fstring_candidates: list[bool] = []
    try:
        tokens = tokenize.generate_tokens(io.StringIO(source).readline)
        for token in tokens:
            if fstring_start is not None and token.type == fstring_start:
                fstring_candidates.append(False)
                continue
            if fstring_middle is not None and token.type == fstring_middle:
                if not fstring_candidates:
                    continue
                if not fstring_candidates[-1] and looks_like_spectra(token.string):
                    fstring_candidates[-1] = True
                if not fstring_candidates[-1] or not looks_like_legacy_spectra_fragment(token.string):
                    continue
                migrated = migrate_source(token.string)
            elif fstring_end is not None and token.type == fstring_end:
                if fstring_candidates:
                    fstring_candidates.pop()
                continue
            elif token.type == tokenize.STRING and looks_like_spectra(token.string):
                migrated = migrate_string_token(token.string)
            else:
                continue
            if migrated == token.string:
                continue
            replacements.append(
                (
                    absolute_offset(offsets, token.start),
                    absolute_offset(offsets, token.end),
                    migrated,
                )
            )
    except tokenize.TokenError:
        return source, 0

    migrated_source = source
    for start, end, replacement in reversed(replacements):
        migrated_source = migrated_source[:start] + replacement + migrated_source[end:]
    return migrated_source, len(replacements)


def python_files(root: Path) -> list[Path]:
    return sorted(
        path
        for path in (root / "scripts").glob("*.py")
        if path.name != Path(__file__).name
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--check", action="store_true", help="report files that need migration")
    args = parser.parse_args()

    changed_files = 0
    changed_literals = 0
    for path in python_files(args.root):
        migrated, replacements = migrate_python_file(path)
        if replacements == 0:
            continue
        changed_files += 1
        changed_literals += replacements
        if not args.check:
            path.write_text(migrated, encoding="utf-8", newline="")

    mode = "would migrate" if args.check else "migrated"
    print(f"{mode} {changed_literals} embedded Spectra string(s) in {changed_files} Python file(s)")
    return 1 if args.check and changed_files else 0


if __name__ == "__main__":
    raise SystemExit(main())
