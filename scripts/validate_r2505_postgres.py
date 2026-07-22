"""Independent PostgreSQL 16 certification gate for R-2505.

The configured path uses a real PostgreSQL server, psql for an independent
version check, Rust integration tests, and a separate OTLP collector process.
Without a URL this gate is deliberately skipped; it never substitutes SQLite.
"""
from __future__ import annotations

import argparse
import json
import os
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[1]


def scrub(value: object, secret: str | None) -> object:
    if isinstance(value, str):
        text = value
        if secret:
            text = text.replace(secret, "<redacted>")
        return text.replace("postgres://", "postgres://<redacted>@") if "postgres://" in text else text
    if isinstance(value, list):
        return [scrub(item, secret) for item in value]
    if isinstance(value, dict):
        return {key: scrub(item, secret) for key, item in value.items()}
    return value


def run(command: list[str], env: dict[str, str], secret: str | None, timeout: int = 300) -> dict:
    completed = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True, timeout=timeout)
    return scrub({
        "command": command,
        "exit_code": completed.returncode,
        "stdout_tail": completed.stdout[-4000:],
        "stderr_tail": completed.stderr[-4000:],
    }, secret)


def varint(data: bytes, offset: int) -> tuple[int, int]:
    value = 0
    shift = 0
    while offset < len(data):
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if not byte & 0x80:
            return value, offset
        shift += 7
        if shift > 63:
            raise ValueError("varint too large")
    raise ValueError("truncated varint")


def fields(data: bytes):
    offset = 0
    while offset < len(data):
        key, offset = varint(data, offset)
        number, wire = key >> 3, key & 7
        if wire == 0:
            value, offset = varint(data, offset)
        elif wire == 2:
            size, offset = varint(data, offset)
            value, offset = data[offset:offset + size], offset + size
        elif wire == 1:
            value, offset = data[offset:offset + 8], offset + 8
        elif wire == 5:
            value, offset = data[offset:offset + 4], offset + 4
        else:
            raise ValueError(f"unsupported protobuf wire type {wire}")
        yield number, wire, value


def nested(data: bytes, field: int) -> list[bytes]:
    return [value for number, wire, value in fields(data) if number == field and wire == 2]


def decode_attributes(data: bytes) -> dict[str, tuple[str, object]]:
    result: dict[str, tuple[str, object]] = {}
    for item in nested(data, 9):
        key = next((value.decode() for number, wire, value in fields(item) if number == 1 and wire == 2), "")
        any_value = next((value for number, wire, value in fields(item) if number == 2 and wire == 2), b"")
        for number, wire, value in fields(any_value):
            if number == 1 and wire == 2:
                result[key] = ("string", value.decode())
            elif number == 2 and wire == 0:
                result[key] = ("bool", bool(value))
            elif number == 3 and wire == 0:
                result[key] = ("int", value)
    return result


def decode_spans(payload: bytes) -> list[dict]:
    spans: list[dict] = []
    for resource in nested(payload, 1):
        for scope in nested(resource, 2):
            for span_data in nested(scope, 2):
                values = {number: value for number, wire, value in fields(span_data) if wire == 2}
                integer = {number: value for number, wire, value in fields(span_data) if wire == 0}
                spans.append({
                    "trace_id": values.get(1, b"").hex(),
                    "span_id": values.get(2, b"").hex(),
                    "parent_id": values.get(4, b"").hex(),
                    "name": values.get(5, b"").decode(errors="replace"),
                    "kind": integer.get(6, 0),
                    "start": integer.get(7, 0),
                    "end": integer.get(8, 0),
                    "attributes": decode_attributes(span_data),
                })
    return spans


def free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", default="target/debug/spectralang.exe")
    parser.add_argument("--database-url", default=None)
    parser.add_argument("--fixture", default="tests/validation/195_postgres_driver.spectra")
    parser.add_argument("--report", required=True)
    args = parser.parse_args()
    url = args.database_url or os.environ.get("SPECTRA_POSTGRES_URL")
    report = {
        "schema": "spectralang.r2505_postgres.v2",
        "environment": {"postgresql_required": True, "required_major": 16, "configured": bool(url)},
        "connection": {}, "pool": {}, "prepared_statements": {}, "query_builder": {}, "crud": {},
        "transactions": {}, "savepoints": {}, "copy_in": {}, "copy_out": {}, "listen_notify": {},
        "async_non_blocking": {}, "cancellation": {}, "tracing": {}, "http_parent": {},
        "security": {}, "diagnostics": {}, "failures": [], "status": "skipped_environment" if not url else "failed",
    }
    secret = url
    env = os.environ.copy()
    if not url:
        report["environment"]["reason"] = "SPECTRA_POSTGRES_URL is not configured; PostgreSQL 16 CI lane is required."
    else:
        parsed = urlsplit(url)
        if parsed.scheme not in {"postgres", "postgresql"} or not parsed.hostname:
            report["failures"].append("invalid PostgreSQL URL")
        else:
            env["SPECTRA_POSTGRES_URL"] = url
            env["PGPASSWORD"] = parsed.password or ""
            psql = ["psql", "--no-psqlrc", "-X", "-h", parsed.hostname, "-p", str(parsed.port or 5432), "-U", parsed.username or "", "-d", parsed.path.lstrip("/"), "-At", "-c", "SHOW server_version_num"]
            version = run(psql, env, secret, 60)
            report["environment"]["version_probe"] = version
            version_text = str(version.get("stdout_tail", "")).strip()
            if version["exit_code"] != 0 or not version_text.startswith("16"):
                report["failures"].append("PostgreSQL 16 version probe failed")

            integration = run(["cargo", "test", "-p", "spectra-db", "--test", "postgres_integration", "--", "--test-threads=1"], env, secret)
            report["connection"]["rust_integration"] = integration
            if integration["exit_code"] != 0:
                report["failures"].append("PostgreSQL integration tests failed")
            for section in ("pool", "prepared_statements", "query_builder", "crud", "transactions", "savepoints", "copy_in", "copy_out", "listen_notify", "async_non_blocking", "cancellation"):
                report[section] = {"status": "covered_by_postgres_integration", "exit_code": integration["exit_code"]}

            collector_port = free_port()
            collector_output = Path(args.report).with_suffix(".collector.json")
            collector_output.parent.mkdir(parents=True, exist_ok=True)
            collector = subprocess.Popen([sys.executable, "tests/fixtures/r2505/collector.py", "--port", str(collector_port), "--output", str(collector_output)], cwd=ROOT)
            try:
                time.sleep(0.25)
                env["SPECTRA_R2505_OTLP_ENDPOINT"] = f"http://127.0.0.1:{collector_port}/v1/traces"
                http_parent = run(["cargo", "test", "-p", "spectra-api", "--test", "postgres_http_parent", "--", "--ignored", "--test-threads=1"], env, secret, 180)
                report["http_parent"] = http_parent
                report["tracing"] = {"collector_process": "started", "http_parent_test": http_parent}
                if http_parent["exit_code"] != 0:
                    report["failures"].append("HTTP parent PostgreSQL tracing test failed")
            finally:
                collector.terminate()
                try:
                    collector.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    collector.kill()
            collector_data = json.loads(collector_output.read_text(encoding="utf-8")) if collector_output.exists() else {}
            spans = []
            for encoded in collector_data.get("payloads", []):
                try:
                    spans.extend(decode_spans(bytes.fromhex(encoded)))
                except (ValueError, UnicodeError) as error:
                    report["failures"].append(f"invalid OTLP protobuf: {error}")
            report["tracing"]["collector"] = {"requests": collector_data.get("requests", 0), "invalid_requests": collector_data.get("invalid_requests", 0), "spans": spans}
            postgres_spans = [span for span in spans if span["name"] == "db.postgres.query"]
            http_spans = [span for span in spans if span["name"] == "http.server"]
            if not postgres_spans or not http_spans:
                report["failures"].append("collector did not receive HTTP and PostgreSQL spans")
            elif not any(span["parent_id"] == parent["span_id"] and span["trace_id"] == parent["trace_id"] for span in postgres_spans for parent in http_spans):
                report["failures"].append("PostgreSQL span is not a child of the HTTP span")

            fixture = Path(args.fixture)
            with tempfile.TemporaryDirectory(prefix="spectra-r2505-") as temporary:
                generated = Path(temporary) / fixture.name
                generated.write_text(fixture.read_text(encoding="utf-8").replace("__SPECTRA_POSTGRES_URL__", url), encoding="utf-8")
                cli = run([str(ROOT / args.binary), "run", str(generated)], env, secret, 180)
            report["connection"]["cli_fixture"] = cli
            if cli["exit_code"] != 0:
                report["failures"].append("PostgreSQL Spectra fixture failed")
            report["security"] = {"secret_not_in_report": secret not in json.dumps(report), "credentials_not_exported": secret not in json.dumps(spans)}
            report["status"] = "passed" if not report["failures"] else "failed"

    output = Path(args.report)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(scrub(report, secret), indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(scrub(report, secret), indent=2, sort_keys=True))
    return 0 if report["status"] in {"passed", "skipped_environment"} else 1


if __name__ == "__main__":
    raise SystemExit(main())
