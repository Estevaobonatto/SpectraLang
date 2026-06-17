from __future__ import annotations

import subprocess
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CARGO = r"C:\Users\estev\.cargo\bin\cargo.exe"


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def fail(message: str) -> None:
    print(f"R-2208 validation failed: {message}", file=sys.stderr)
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


def validate_json_surface() -> None:
    cargo = read("packages/spectra-api/Cargo.toml")
    require("serde_json" in cargo, "Cargo.toml must include serde_json")

    json_rs = read("packages/spectra-api/src/json.rs")
    for term in [
        "pub enum JsonValue",
        "pub struct JsonNumber",
        "pub enum JsonParseErrorKind",
        "pub struct JsonParseError",
        "pub enum JsonEncodeErrorKind",
        "pub struct JsonEncodeError",
        "parse_json",
        "encode_json",
        "encode_json_pretty",
        "json_kind_of",
        "byte_offset_for_line_column",
        "serde_json::from_str",
        "serde_json::to_string",
        "JSON_KIND_OBJECT",
        "JsonParseErrorKind::InvalidSyntax",
        "JsonParseErrorKind::UnexpectedEof",
        "JsonEncodeErrorKind::NonFiniteNumber",
    ]:
        require(term in json_rs, f"missing JSON implementation term {term}")

    for test in [
        "round_trip_primitives_arrays_maps_nested_and_null",
        "parser_handles_common_escape_sequences_and_unicode",
        "invalid_json_reports_typed_error_with_byte_offset",
        "encoder_rejects_invalid_numbers_and_non_finite_float_values",
        "encoder_output_is_rfc8259_json_for_supported_values",
        "host_kind_uses_full_parser_not_balanced_braces",
    ]:
        require(test in json_rs, f"missing R-2208 regression test {test}")


def validate_docs_and_planning() -> None:
    docs = read("docs/api/std-api-json.md")
    for term in [
        "std.api.json",
        "RFC 8259",
        "offset",
        "spectra.api.json.validate",
        "spectra.api.json.kind",
        "scripts/validate_r2208_json_codec.py",
    ]:
        require(term in docs, f"JSON API docs missing {term}")

    roadmap = parse_toml("roadmap/roadmap.toml")
    items = {item["id"]: item for item in roadmap["items"]}
    r2208 = items.get("R-2208")
    require(r2208 is not None, "R-2208 missing from roadmap")
    require(r2208.get("status") == "complete", "R-2208 must be marked complete")
    require(r2208.get("owner") == "web", "R-2208 owner must remain web")
    require(r2208.get("dependencies") == ["R-2202"], "R-2208 dependency must remain R-2202")
    acceptance = "\n".join(r2208.get("acceptance", []))
    for term in [
        "round-trip tests cover primitives, nested structures, arrays, maps, and null",
        "typed parse error with byte offset",
        "valid RFC 8259 JSON",
        "std.api.json.*",
        "docs/api/std-api-json.md",
        "cargo test -p spectra-api json --offline",
        "scripts/validate_r2208_json_codec.py",
    ]:
        require(term in acceptance, f"R-2208 acceptance must mention {term}")

    backlog = read("docs/roadmap-backlog.md")
    require("## R-2208 std.api.json Encoder and Decoder" in backlog, "backlog R-2208 missing")
    r2208_block = backlog.split("## R-2208 std.api.json Encoder and Decoder", 1)[1].split("## R-2209", 1)[0]
    for term in [
        "Status: `complete`",
        "packages/spectra-api/src/json.rs",
        "JsonValue",
        "JsonParseError",
        "JsonEncodeError",
        "docs/api/std-api-json.md",
        "validate_r2208_json_codec.py",
    ]:
        require(term in r2208_block, f"backlog R-2208 block must mention {term}")

    plan = read("docs/production-ai-implementation-plan.md")
    require(
        "R-2208` `std.api.json` encoder and decoder (complete;" in plan,
        "implementation plan must mark R-2208 complete",
    )


def validate_runner() -> None:
    runner = read("run_tests.ps1")
    require(
        "validate_r2208_json_codec.py" in runner,
        "run_tests.ps1 must run R-2208 validator",
    )
    require(
        'Teste = "validate_r2208_json_codec"' in runner,
        "run_tests.ps1 must record R-2208 result",
    )


def main() -> None:
    validate_json_surface()
    run_command([CARGO, "test", "-q", "-p", "spectra-api", "json", "--offline"])
    validate_docs_and_planning()
    validate_runner()
    print("validated R-2208 std.api.json encoder and decoder")


if __name__ == "__main__":
    main()
