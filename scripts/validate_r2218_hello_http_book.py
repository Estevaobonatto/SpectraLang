#!/usr/bin/env python3
"""Validate R-2218 Hello HTTP book chapter and example."""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def fail(message: str) -> None:
    print(f"R-2218 validation failed: {message}", file=sys.stderr)
    sys.exit(1)


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def parse_toml(path: str):
    with (ROOT / path).open("rb") as fh:
        return tomllib.load(fh)


def run_command(args: list[str], timeout: int = 90) -> str:
    completed = subprocess.run(
        args,
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=timeout,
        check=False,
    )
    if completed.returncode != 0:
        fail(f"command {' '.join(args)} failed:\n{completed.stdout}")
    return completed.stdout


def cargo_cmd() -> str:
    configured = os.environ.get("CARGO")
    if configured:
        return configured
    found = shutil.which("cargo")
    if found:
        return found
    windows_default = Path.home() / ".cargo" / "bin" / "cargo.exe"
    if windows_default.exists():
        return str(windows_default)
    return "cargo"


def validate_docs() -> None:
    chapter = read("docs/book/09-hello-http.md")
    book_index = read("docs/book/README.md")
    api_index = read("docs/api/README.md")

    for term in [
        "# 9. Hello HTTP",
        "examples/api/00_hello_http.spectra",
        "std.api.routing",
        "std.api.handler",
        "std.api.http",
        "std.api.server",
        "typed `Response`",
        "local assigned port",
        "scripts/validate_r2218_hello_http_book.py",
        "run_tests.ps1",
    ]:
        require(term in chapter, f"Hello HTTP chapter missing {term}")

    require("[Hello HTTP](09-hello-http.md)" in book_index, "book index missing Hello HTTP")
    require(
        "examples/api/00_hello_http.spectra" in book_index,
        "book index missing Hello HTTP example",
    )
    require("[Hello HTTP](../book/09-hello-http.md)" in api_index, "API index missing chapter link")

    for path in [
        "docs/api/std-api-server-lifecycle.md",
        "docs/api/std-api-handler.md",
        "docs/api/std-api-routing.md",
    ]:
        require("../book/09-hello-http.md" in read(path), f"{path} missing Hello HTTP link")


def validate_example() -> None:
    example = read("examples/api/00_hello_http.spectra")
    for term in [
        "module hello_http;",
        "get(routes, \"/hello\")",
        "with_header(text(\"Hello HTTP from Spectra\")",
        "register_sync(route_id(route), response)",
        "dispatch_sync(handler, request_value)",
        "block_on(serve(server, routes))",
        "local_port(server)",
        "shutdown(server)",
    ]:
        require(term in example, f"Hello HTTP example missing {term}")


def validate_planning() -> None:
    roadmap = parse_toml("roadmap/roadmap.toml")
    items = {item["id"]: item for item in roadmap["items"]}
    r2218 = items.get("R-2218")
    require(r2218 is not None, "R-2218 missing from roadmap")
    require(r2218.get("status") == "complete", "R-2218 must be complete")
    require(r2218.get("owner") == "ecosystem", "R-2218 owner changed")
    require(r2218.get("dependencies") == ["R-2217"], "R-2218 dependencies changed")
    acceptance = "\n".join(r2218.get("acceptance", []))
    for term in [
        "reachable from the book index",
        "docs/api/README.md",
        "scripts/validate_ai_book.py",
        "examples/api/00_hello_http.spectra",
        "validate_r2218_hello_http_book.py",
    ]:
        require(term in acceptance, f"R-2218 acceptance missing {term}")

    backlog = read("docs/roadmap-backlog.md")
    block = backlog.split("## R-2218 API Book Chapter: Hello HTTP", 1)[1].split(
        "## R-2219", 1
    )[0]
    for term in [
        "Status: `complete`",
        "docs/book/09-hello-http.md",
        "docs/api/README.md",
        "examples/api/00_hello_http.spectra",
        "scripts/validate_r2218_hello_http_book.py",
    ]:
        require(term in block, f"backlog R-2218 missing {term}")

    plan = read("docs/production-ai-implementation-plan.md")
    require(
        "R-2218` API book chapter: `Hello HTTP` (complete;" in plan,
        "implementation plan must mark R-2218 complete",
    )

    runner = read("run_tests.ps1")
    require("validate_r2218_hello_http_book.py" in runner, "run_tests.ps1 must run R-2218")
    require(
        'Teste = "validate_r2218_hello_http_book"' in runner,
        "run_tests.ps1 must record R-2218",
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    default_binary = ROOT / "target" / "debug" / (
        "spectralang.exe" if sys.platform.startswith("win") else "spectralang"
    )
    parser.add_argument("--binary", type=Path, default=default_binary)
    args = parser.parse_args()

    run_command([cargo_cmd(), "build", "-q", "-p", "spectra-cli"], timeout=120)
    validate_docs()
    validate_example()
    run_command(["python", "scripts/validate_ai_book.py"], timeout=60)
    run_command([str(args.binary.resolve()), "compile", "examples/api/00_hello_http.spectra"])
    run_command([str(args.binary.resolve()), "run", "examples/api/00_hello_http.spectra"])
    validate_planning()
    print("validated R-2218 Hello HTTP book chapter and example")


if __name__ == "__main__":
    main()
