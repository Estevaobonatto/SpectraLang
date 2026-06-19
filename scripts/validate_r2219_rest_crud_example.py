#!/usr/bin/env python3
"""Validate R-2219 REST CRUD API example."""

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
    print(f"R-2219 validation failed: {message}", file=sys.stderr)
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


def validate_example_source() -> None:
    example = read("examples/api/01_rest_crud.spectra")
    required_terms = [
        "module rest_crud_api;",
        "import std.api.query as query;",
        "import std.api.form as form;",
        "#[derive(Serialize, Deserialize)]",
        "Todo::from_json",
        "created.to_json()",
        "query.parse(\"/todos?page=2&completed=false\")",
        "query.bind(todo_query, qs2)",
        "form.parse(\"title=Ship+API&completed=false\")",
        "form.bind(todo_form, fs2)",
        "get(routes, \"/todos\")",
        "post(routes, \"/todos\")",
        "get(routes, \"/todos/{id:\\\\d+}\")",
        "put(routes, \"/todos/{id:\\\\d+}\")",
        "delete(routes, \"/todos/{id:\\\\d+}\")",
        "match_param_int(read_match, \"id\")",
        "register_sync(route_id(list_route), list_response)",
        "dispatch_sync(create_handler",
        "block_on(serve(server, routes))",
        "local_port(server)",
        "shutdown(server)",
    ]
    for term in required_terms:
        require(term in example, f"REST CRUD example missing {term}")


def validate_docs() -> None:
    api_index = read("docs/api/README.md")
    require(
        "01_rest_crud.spectra" in api_index,
        "docs/api/README.md must reference the REST CRUD example",
    )


def validate_planning() -> None:
    roadmap = parse_toml("roadmap/roadmap.toml")
    items = {item["id"]: item for item in roadmap["items"]}
    r2219 = items.get("R-2219")
    require(r2219 is not None, "R-2219 missing from roadmap")
    require(r2219.get("status") == "complete", "R-2219 must be complete")
    require(r2219.get("owner") == "ecosystem", "R-2219 owner changed")
    require(r2219.get("dependencies") == ["R-2217", "R-2209"], "R-2219 dependencies changed")
    acceptance = "\n".join(r2219.get("acceptance", []))
    for term in [
        "examples/api/01_rest_crud.spectra",
        "public `std.api.*` surface",
        "real local server",
        "smoke test",
        "validate_r2219_rest_crud_example.py",
    ]:
        require(term in acceptance, f"R-2219 acceptance missing {term}")

    backlog = read("docs/roadmap-backlog.md")
    block = backlog.split("## R-2219 API Example: REST CRUD", 1)[1].split(
        "## R-2220", 1
    )[0]
    for term in [
        "Status: `complete`",
        "examples/api/01_rest_crud.spectra",
        "JSON derive",
        "path params",
        "query strings",
        "form binding",
        "scripts/validate_r2219_rest_crud_example.py",
    ]:
        require(term in block, f"backlog R-2219 missing {term}")

    plan = read("docs/production-ai-implementation-plan.md")
    require(
        "R-2219` API example: REST CRUD (complete;" in plan,
        "implementation plan must mark R-2219 complete",
    )

    runner = read("run_tests.ps1")
    require("validate_r2219_rest_crud_example.py" in runner, "run_tests.ps1 must run R-2219")
    require(
        'Teste = "validate_r2219_rest_crud_example"' in runner,
        "run_tests.ps1 must record R-2219",
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    default_binary = ROOT / "target" / "debug" / (
        "spectralang.exe" if sys.platform.startswith("win") else "spectralang"
    )
    parser.add_argument("--binary", type=Path, default=default_binary)
    args = parser.parse_args()

    run_command([cargo_cmd(), "build", "-q", "-p", "spectra-cli"], timeout=120)
    validate_example_source()
    validate_docs()
    run_command([str(args.binary.resolve()), "compile", "examples/api/01_rest_crud.spectra"])
    run_command([str(args.binary.resolve()), "run", "examples/api/01_rest_crud.spectra"])
    validate_planning()
    print("validated R-2219 REST CRUD API example")


if __name__ == "__main__":
    main()
