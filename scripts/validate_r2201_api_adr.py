#!/usr/bin/env python3
"""Validate R-2201 API library architecture ADR coverage."""

from __future__ import annotations

import pathlib
import re
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
ADR_PATH = ROOT / "docs" / "adr" / "0011-api-library-architecture.md"
ROADMAP_PATH = ROOT / "roadmap" / "roadmap.toml"
BACKLOG_PATH = ROOT / "docs" / "roadmap-backlog.md"
PLAN_PATH = ROOT / "docs" / "production-ai-implementation-plan.md"


REQUIRED_SECTIONS = [
    "Context",
    "Decision",
    "Repository Layout",
    "Public API Surface",
    "Host-Call Boundary",
    "HTTP Version Strategy",
    "TLS Model",
    "Async Dependencies",
    "Ownership",
    "Migration Path",
    "Consequences",
    "Rejected Alternatives",
    "Acceptance Evidence",
]

REQUIRED_TERMS = [
    "Status: Accepted",
    "Roadmap item: R-2201",
    "spectra.api",
    "std.api.*",
    "spectra-api",
    "packages/spectra-api",
    "packages/spectra-api/spectra.toml",
    "runtime/src/api/",
    "docs/api/",
    "examples/api/",
    "spectra.api.*",
    "std.api.http",
    "std.api.server",
    "std.api.client",
    "std.api.json",
    "std.api.tls",
    "std.api.routing",
    "Request",
    "Response",
    "Method",
    "Status",
    "Header",
    "Cookie",
    "Task<T>",
    "Stream<bytes>",
    "HTTP/1.1",
    "HTTP/2",
    "HTTP/3",
    "rustls",
    "allow_plaintext",
    "ALPN",
    "spectra-runtime",
    "mio",
    "no public Tokio runtime dependency",
    "std.serve",
    "R-2807",
]


def fail(message: str) -> int:
    print(f"R-2201 validation failed: {message}", file=sys.stderr)
    return 1


def require_file(path: pathlib.Path) -> str:
    if not path.exists():
        raise FileNotFoundError(path)
    return path.read_text(encoding="utf-8")


def block_for_item(roadmap: str, item_id: str) -> str | None:
    match = re.search(
        rf'id = "{re.escape(item_id)}".*?(?=\n\[\[items\]\]|\Z)',
        roadmap,
        flags=re.DOTALL,
    )
    return match.group(0) if match else None


def main() -> int:
    try:
        adr = require_file(ADR_PATH)
        roadmap = require_file(ROADMAP_PATH)
        backlog = require_file(BACKLOG_PATH)
        plan = require_file(PLAN_PATH)
    except FileNotFoundError as exc:
        return fail(f"required file is missing: {exc}")

    for section in REQUIRED_SECTIONS:
        if f"## {section}" not in adr:
            return fail(f"missing ADR section: {section}")

    normalized = re.sub(r"\s+", " ", adr)
    for term in REQUIRED_TERMS:
        if term not in adr and term not in normalized:
            return fail(f"missing required ADR decision term: {term}")

    r2201_block = block_for_item(roadmap, "R-2201")
    if not r2201_block:
        return fail("roadmap.toml R-2201 block not found")
    for expected in [
        'status = "complete"',
        "docs/adr/0011-api-library-architecture.md",
        "spectra.api",
        "std.api.*",
        "rustls",
        "HTTP versions",
        "async dependencies",
        "scripts/validate_r2201_api_adr.py",
    ]:
        if expected not in r2201_block:
            return fail(f"R-2201 roadmap block missing: {expected}")

    r2202_block = block_for_item(roadmap, "R-2202")
    if not r2202_block:
        return fail("roadmap.toml R-2202 block not found")
    if 'owner = "web"' not in r2202_block:
        return fail("R-2202 must be owned by web after the API architecture ADR")
    if 'dependencies = ["R-2201"]' not in r2202_block:
        return fail("R-2202 must depend on R-2201")

    backlog_match = re.search(
        r"## R-2201 ADR: API Library Architecture.*?(?=\n## R-2202|\Z)",
        backlog,
        flags=re.DOTALL,
    )
    if not backlog_match:
        return fail("backlog R-2201 section not found")
    backlog_block = backlog_match.group(0)
    for expected in [
        "Status: `complete`",
        "docs/adr/0011-api-library-architecture.md",
        "packages/spectra-api",
        "std.api.*",
        "spectra.api.*",
        "rustls",
        "scripts/validate_r2201_api_adr.py",
    ]:
        if expected not in backlog_block:
            return fail(f"backlog R-2201 section missing: {expected}")

    r2202_backlog = re.search(
        r"## R-2202 spectra-api Rust Crate and Host Call Registration.*?(?=\n## R-2203|\Z)",
        backlog,
        flags=re.DOTALL,
    )
    if not r2202_backlog or "Owner: `web`" not in r2202_backlog.group(0):
        return fail("backlog R-2202 must be owned by web")

    for expected in [
        "`R-2201` ADR: API Library Architecture (complete",
        "docs/adr/0011-api-library-architecture.md",
        "rustls",
        "spectra.api",
    ]:
        if expected not in plan:
            return fail(f"implementation plan missing: {expected}")

    print("validated R-2201 API library architecture ADR")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
