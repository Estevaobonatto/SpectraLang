#!/usr/bin/env python3
"""Validate R-106 language feature maturity policy synchronization."""

from __future__ import annotations

import argparse
import re
import subprocess
from pathlib import Path


REQUIRED_SECTIONS = ("Stable", "Beta", "Experimental", "Deferred")
REQUIRED_EXPERIMENTAL: tuple[str, ...] = ()
PROMOTED_STABLE = ("switch", "do-while", "loop", "if not")


def extract_section(text: str, name: str) -> str:
    pattern = rf"^### {re.escape(name)}\s*$([\s\S]*?)(?=^### |\Z)"
    match = re.search(pattern, text, re.MULTILINE)
    return match.group(1) if match else ""


def extract_doc_experimental(text: str) -> list[str]:
    section = extract_section(text, "Experimental")
    found: list[str] = []
    for line in section.splitlines():
        match = re.match(r"\s*-\s+`([^`]+)`\s*$", line)
        if match:
            feature = match.group(1)
            if not feature.startswith("--") and " " not in feature:
                found.append(feature)
    return found


def extract_doc_stable(text: str) -> str:
    return extract_section(text, "Stable")


def extract_source_experimental(source: str) -> list[str]:
    match = re.search(
        r"KNOWN_EXPERIMENTAL_FEATURES:\s*&\[&str\]\s*=\s*&\[(?P<body>[^\]]*)\]",
        source,
        re.MULTILINE,
    )
    if not match:
        return []
    return re.findall(r'"([^"]+)"', match.group("body"))


def extract_cli_experimental(binary: Path) -> list[str]:
    proc = subprocess.run(
        [str(binary), "--list-experimental"],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    if proc.returncode != 0:
        raise RuntimeError(f"--list-experimental exited with {proc.returncode}: {proc.stdout}")
    found: list[str] = []
    for line in proc.stdout.splitlines():
        match = re.match(r"\s*-\s+(.+?)\s*$", line)
        if match:
            found.append(match.group(1))
    return found


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=".")
    parser.add_argument("--binary", default="target/debug/spectralang.exe")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    binary = (root / args.binary).resolve()
    policy_path = root / "docs" / "language-feature-maturity.md"
    main_path = root / "tools" / "spectra-cli" / "src" / "main.rs"

    errors: list[str] = []
    if not policy_path.is_file():
        errors.append(f"missing maturity policy: {policy_path}")
    if not main_path.is_file():
        errors.append(f"missing CLI source: {main_path}")
    if not binary.is_file():
        errors.append(f"missing CLI binary: {binary}")
    if errors:
        for error in errors:
            print(f"ERROR: {error}")
        return 1

    policy = policy_path.read_text(encoding="utf-8")
    source = main_path.read_text(encoding="utf-8")

    for section in REQUIRED_SECTIONS:
        body = extract_section(policy, section)
        if not body.strip():
            errors.append(f"maturity policy missing non-empty section: {section}")

    doc_features = extract_doc_experimental(policy)
    stable_section = extract_doc_stable(policy)
    source_features = extract_source_experimental(source)
    try:
        cli_features = extract_cli_experimental(binary)
    except RuntimeError as exc:
        errors.append(str(exc))
        cli_features = []

    expected = list(REQUIRED_EXPERIMENTAL)
    if doc_features != expected:
        errors.append(f"docs experimental features mismatch: {doc_features} != {expected}")
    if source_features != expected:
        errors.append(f"source experimental features mismatch: {source_features} != {expected}")
    if cli_features != expected:
        errors.append(f"CLI experimental features mismatch: {cli_features} != {expected}")

    for feature in PROMOTED_STABLE:
        if f"`{feature}`" not in stable_section:
            errors.append(f"promoted stable feature missing from Stable section: {feature}")

    required_phrases = (
        "experimental`: available only behind an explicit feature gate when active",
        "there are currently no active experimental syntax gates",
        "update CLI help or `--list-experimental` if the change affects experimental gating",
    )
    for phrase in required_phrases:
        if phrase not in policy:
            errors.append(f"maturity policy missing phrase: {phrase}")

    if errors:
        for error in errors:
            print(f"ERROR: {error}")
        return 1

    print("validated R-106 feature maturity policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
