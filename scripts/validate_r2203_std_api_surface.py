from __future__ import annotations

import re
import subprocess
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
REQUIRED_MODULES = [
    "std.api",
    "std.api.http",
    "std.api.server",
    "std.api.client",
    "std.api.json",
    "std.api.tls",
    "std.api.routing",
    "std.api.errors",
]
REQUIRED_FUNCTIONS = [
    "std.api.http.method_name",
    "std.api.http.method_allows_body",
    "std.api.http.method_is_safe",
    "std.api.http.status_reason",
    "std.api.http.status_class",
    "std.api.http.status_is_success",
    "std.api.http.header_name_is_valid",
    "std.api.http.header_value_is_valid",
    "std.api.http.request",
    "std.api.http.request_new",
    "std.api.http.request_method",
    "std.api.http.response",
    "std.api.http.response_new",
    "std.api.http.response_status",
    "std.api.http.header",
    "std.api.http.status",
    "std.api.server.new",
    "std.api.server.serve",
    "std.api.server.state",
    "std.api.server.shutdown",
    "std.api.client.new",
    "std.api.client.request",
    "std.api.client.timeout_ms",
    "std.api.json.validate",
    "std.api.json.kind",
    "std.api.json.encode",
    "std.api.json.decode",
    "std.api.tls.config_new",
    "std.api.tls.config_mode",
    "std.api.tls.server_config",
    "std.api.tls.client_config",
    "std.api.routing.router",
    "std.api.routing.router_new",
    "std.api.routing.route_count",
    "std.api.routing.get",
    "std.api.routing.post",
    "std.api.routing.put",
    "std.api.routing.patch",
    "std.api.routing.delete",
    "std.api.errors.last_code",
    "std.api.errors.last_message",
]
REQUIRED_TYPES = [
    "std.api.http.Request",
    "std.api.http.Response",
    "std.api.http.Method",
    "std.api.http.Status",
    "std.api.http.Header",
    "std.api.http.Headers",
    "std.api.http.Cookie",
    "std.api.http.Body",
    "std.api.server.Server",
    "std.api.client.Client",
    "std.api.json.JsonValue",
    "std.api.tls.TlsConfig",
    "std.api.routing.Route",
    "std.api.routing.Router",
    "std.api.errors.ApiError",
]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def fail(message: str) -> None:
    print(f"R-2203 validation failed: {message}", file=sys.stderr)
    sys.exit(1)


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def run_command(args: list[str]) -> str:
    completed = subprocess.run(
        args,
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    if completed.returncode != 0:
        fail(f"command {' '.join(args)} failed:\n{completed.stdout}")
    return completed.stdout


def parse_toml(path: str):
    with (ROOT / path).open("rb") as fh:
        return tomllib.load(fh)


def validate_public_surface() -> None:
    builtins = read("compiler/src/semantic/builtin_modules.rs")
    semantic = read("compiler/src/semantic/mod.rs")
    lsp = read("tools/spectra-lsp/src/main.rs")
    snapshot = read("compiler/tests/snapshots/std_api_public_function_table.snap")
    fixture = read("tests/semantic/std_api_surface.spectra")

    for module in REQUIRED_MODULES:
        require(module in builtins, f"{module} missing from builtin surface")
        require(module in semantic, f"{module} missing from semantic namespace seed")
        require(f"module {module}" in snapshot, f"{module} missing from snapshot")

    for function in REQUIRED_FUNCTIONS:
        require(function in builtins, f"{function} missing from builtin function constants")
        require(function in snapshot, f"{function} missing from function table snapshot")

    for ty in REQUIRED_TYPES:
        require(ty in builtins, f"{ty} missing from builtin type constants")
        require(ty in snapshot, f"{ty} missing from type table snapshot")

    for sample_call in [
        "from std.api.http",
        "from std.api.json",
        "from std.api.tls",
        "from std.api.routing",
        "from std.api.errors",
        "request_new",
        "method_name",
        "client_config",
        "router_new",
        "last_code",
    ]:
        require(sample_call in fixture, f"{sample_call} missing from semantic fixture")

    require(
        "STD_API_PUBLIC_FUNCTIONS" in lsp and "std_api_completion_items" in lsp,
        "LSP completion must be driven by the std.api public function table",
    )
    require(
        "std_api_surface_resolves_qualified_and_aliased_calls" in read("compiler/tests/stage_smoke.rs"),
        "compiler semantic regression test missing",
    )
    require(
        "std_api_public_function_table_is_snapshotted" in read("compiler/tests/snapshot_tests.rs"),
        "public function table snapshot test missing",
    )
    require(
        "std_api_completion_items_cover_modules_types_and_functions" in lsp,
        "LSP completion regression test missing",
    )


def validate_cli(binary: Path) -> None:
    binary_arg = str(binary)
    run_command([binary_arg, "check", "tests/semantic/std_api_surface.spectra"])
    run_command([binary_arg, "fmt", "--check", "tests/semantic/std_api_surface.spectra"])
    experimental = run_command([binary_arg, "--list-experimental"])
    require(
        "Experimental language features: none" in experimental,
        "--list-experimental must keep reporting no active syntax gates",
    )
    require(
        "std.api" not in experimental,
        "std.api must be stable surface, not an experimental syntax gate",
    )


def validate_planning() -> None:
    roadmap = parse_toml("roadmap/roadmap.toml")
    items = {item["id"]: item for item in roadmap["items"]}
    r2203 = items.get("R-2203")
    require(r2203 is not None, "R-2203 missing from roadmap")
    require(r2203.get("status") == "complete", "R-2203 must be marked complete")
    require(r2203.get("owner") == "semantic", "R-2203 owner must remain semantic")
    require(r2203.get("dependencies") == ["R-2202"], "R-2203 must depend only on R-2202")
    acceptance = "\n".join(r2203.get("acceptance", []))
    for term in [
        "std.api.*",
        "formatter",
        "LSP completion",
        "--list-experimental",
        "qualified std.api.* calls",
        "snapshot tests",
        "scripts/validate_r2203_std_api_surface.py",
    ]:
        require(term in acceptance, f"R-2203 acceptance must mention {term}")

    backlog = read("docs/roadmap-backlog.md")
    require("## R-2203 std.api Surface in Semantic Analysis" in backlog, "backlog R-2203 missing")
    r2203_block = backlog.split("## R-2203 std.api Surface in Semantic Analysis", 1)[1].split("## R-2204", 1)[0]
    for term in [
        "Status: `complete`",
        "compiler/src/semantic/builtin_modules.rs",
        "tests/semantic/std_api_surface.spectra",
        "std_api_public_function_table.snap",
        "validate_r2203_std_api_surface.py",
    ]:
        require(term in r2203_block, f"backlog R-2203 block must mention {term}")

    plan = read("docs/production-ai-implementation-plan.md")
    require(
        "R-2203` `std.api.*` semantic and tooling surface (complete;" in plan,
        "implementation plan must mark R-2203 complete",
    )


def validate_runner() -> None:
    runner = read("run_tests.ps1")
    require(
        "validate_r2203_std_api_surface.py" in runner,
        "run_tests.ps1 must run R-2203 validator",
    )
    require(
        'Teste = "validate_r2203_std_api_surface"' in runner,
        "run_tests.ps1 must record R-2203 result",
    )


def main() -> None:
    binary = ROOT / "target" / "debug" / ("spectralang.exe" if sys.platform.startswith("win") else "spectralang")
    if not binary.exists():
        fail(f"missing CLI binary {binary}; run cargo build -p spectra-cli first")
    validate_public_surface()
    validate_cli(binary)
    validate_planning()
    validate_runner()
    print("validated R-2203 std.api semantic and tooling surface")


if __name__ == "__main__":
    main()
