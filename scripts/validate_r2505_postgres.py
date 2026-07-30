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
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]


def sensitive_values(secret: str | None) -> list[str]:
    if not secret:
        return []
    values = [secret]
    password = urlsplit(secret).password or ""
    if len(password) >= 8:
        values.append(password)
    return sorted(set(values), key=len, reverse=True)


def contains_sensitive_value(text: str, secret: str | None) -> bool:
    return any(value in text for value in sensitive_values(secret))


def scrub(value: object, secret: str | None) -> object:
    if isinstance(value, str):
        text = value
        for sensitive in sensitive_values(secret):
            text = text.replace(sensitive, "<redacted>")
        return text.replace("postgres://", "postgres://<redacted>@") if "postgres://" in text else text
    if isinstance(value, list):
        return [scrub(item, secret) for item in value]
    if isinstance(value, dict):
        return {key: scrub(item, secret) for key, item in value.items()}
    return value


def run(command: list[str], env: dict[str, str], secret: str | None, timeout: int = 300) -> dict:
    completed = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True, timeout=timeout)
    raw = json.dumps(command) + completed.stdout + completed.stderr
    credential_leaked = contains_sensitive_value(raw, secret)
    if secret:
        parsed = urlsplit(secret)
        password = parsed.password or ""
        username = parsed.username or ""
        credential_leaked = credential_leaked or any(
            marker and marker in raw
            for marker in (
                f":{password}@",
                f"password={password}",
                f"password: {password}",
                f"PGPASSWORD={password}",
                f"user={username}",
                f"username={username}",
            )
        )
    return scrub({
        "command": command,
        "exit_code": completed.returncode,
        "stdout_tail": completed.stdout[-4000:],
        "stderr_tail": completed.stderr[-4000:],
        "secret_leaked": credential_leaked,
    }, secret)


def passed_named_test(result: dict) -> bool:
    output = f"{result.get('stdout_tail', '')}\n{result.get('stderr_tail', '')}"
    return result.get("exit_code") == 0 and "1 passed" in output


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
                fixed64 = {
                    number: int.from_bytes(value, "little")
                    for number, wire, value in fields(span_data)
                    if wire == 1
                }
                spans.append({
                    "trace_id": values.get(1, b"").hex(),
                    "span_id": values.get(2, b"").hex(),
                    "parent_id": values.get(4, b"").hex(),
                    "name": values.get(5, b"").decode(errors="replace"),
                    "kind": integer.get(6, 0),
                    "start": fixed64.get(7, 0),
                    "end": fixed64.get(8, 0),
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
    parser.add_argument(
        "--require-database",
        action="store_true",
        help="fail instead of skipping when PostgreSQL 16 is unavailable",
    )
    parser.add_argument(
        "--version-probe-docker-container",
        default=None,
        help="use psql inside this PostgreSQL container for the independent local version probe",
    )
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
        if args.require_database:
            report["status"] = "failed"
            report["failures"].append("PostgreSQL 16 is required but SPECTRA_POSTGRES_URL is not configured")
    else:
        parsed = urlsplit(url)
        if parsed.scheme not in {"postgres", "postgresql"} or not parsed.hostname:
            report["failures"].append("invalid PostgreSQL URL")
        else:
            env["SPECTRA_POSTGRES_URL"] = url
            env["PGPASSWORD"] = parsed.password or ""
            if args.version_probe_docker_container:
                docker_port = run(
                    [
                        "docker", "port", args.version_probe_docker_container,
                        "5432/tcp",
                    ],
                    env,
                    secret,
                    30,
                )
                report["environment"]["docker_port_probe"] = docker_port
                expected_port = parsed.port or 5432
                mapped_to_endpoint = (
                    parsed.hostname in {"127.0.0.1", "localhost", "::1"}
                    and docker_port["exit_code"] == 0
                    and any(
                        line.strip().endswith(f":{expected_port}")
                        for line in str(docker_port.get("stdout_tail", "")).splitlines()
                    )
                )
                if not mapped_to_endpoint:
                    report["failures"].append(
                        "Docker version probe container is not mapped to the configured database endpoint"
                    )
                psql = [
                    "docker", "exec", args.version_probe_docker_container,
                    "psql", "--no-psqlrc", "-X", "-U", parsed.username or "",
                    "-d", parsed.path.lstrip("/"), "-At", "-c",
                    "SHOW server_version_num",
                ]
            else:
                psql = ["psql", "--no-psqlrc", "-X", "-h", parsed.hostname, "-p", str(parsed.port or 5432), "-U", parsed.username or "", "-d", parsed.path.lstrip("/"), "-At", "-c", "SHOW server_version_num"]
            version = run(psql, env, secret, 60)
            report["environment"]["version_probe"] = version
            version_text = str(version.get("stdout_tail", "")).strip()
            if version["exit_code"] != 0 or not version_text.startswith("16"):
                report["failures"].append("PostgreSQL 16 version probe failed")

            named_tests = {
                "crud": "real_postgres_crud_and_transaction_when_configured",
                "transactions": "real_postgres_transactions_copy_and_notify_when_configured",
                "pool": "real_postgres_pool_and_async_bridge_when_configured",
                "cancellation": "real_postgres_async_cancel_is_non_blocking_when_configured",
                "copy": "real_postgres_copy_streams_100k_rows_when_configured",
            }
            evidence: dict[str, dict] = {}
            for capability, test_name in named_tests.items():
                result = run(
                    [
                        "cargo", "test", "-p", "spectra-db", "--test",
                        "postgres_integration", test_name, "--", "--exact",
                        "--test-threads=1",
                    ],
                    env,
                    secret,
                )
                result["named_test"] = test_name
                result["proved"] = passed_named_test(result)
                evidence[capability] = result
                if not result["proved"]:
                    report["failures"].append(f"PostgreSQL {capability} named test did not prove one passing test")

            public_task_bridge = run(
                [
                    "cargo", "test", "-p", "spectra-api", "--test",
                    "postgres_task_bridge",
                    "public_postgres_task_cancel_is_non_blocking_and_connection_is_reusable",
                    "--", "--exact", "--ignored", "--test-threads=1",
                ],
                env,
                secret,
                180,
            )
            public_task_bridge["named_test"] = (
                "public_postgres_task_cancel_is_non_blocking_and_connection_is_reusable"
            )
            public_task_bridge["proved"] = passed_named_test(public_task_bridge)
            if not public_task_bridge["proved"]:
                report["failures"].append(
                    "public PostgreSQL Task cancellation/non-blocking test did not prove one passing test"
                )

            report["connection"]["rust_integration"] = evidence["crud"]
            report["prepared_statements"] = evidence["crud"]
            report["query_builder"] = evidence["crud"]
            report["crud"] = evidence["crud"]
            report["transactions"] = evidence["transactions"]
            report["savepoints"] = evidence["transactions"]
            report["listen_notify"] = evidence["transactions"]
            report["pool"] = evidence["pool"]
            report["async_non_blocking"] = public_task_bridge
            report["cancellation"] = {
                "driver_operation": evidence["cancellation"],
                "public_task_bridge": public_task_bridge,
            }
            report["copy_in"] = evidence["copy"]
            report["copy_out"] = evidence["copy"]

            collector_port = free_port()
            collector_output = Path(args.report).with_suffix(".collector.json")
            collector_output.parent.mkdir(parents=True, exist_ok=True)
            collector_output.unlink(missing_ok=True)
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
                try:
                    request = Request(
                        f"http://127.0.0.1:{collector_port}/shutdown",
                        data=b"",
                        method="POST",
                    )
                    with urlopen(request, timeout=5):
                        pass
                    collector.wait(timeout=10)
                except (OSError, subprocess.TimeoutExpired):
                    collector.terminate()
                    try:
                        collector.wait(timeout=5)
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
            if any(
                span["start"] <= 0 or span["end"] < span["start"]
                for span in spans
            ):
                report["failures"].append("OTLP spans contain invalid timestamps")

            fixture = Path(args.fixture)
            with tempfile.TemporaryDirectory(prefix="spectra-r2505-") as temporary:
                generated = Path(temporary) / fixture.name
                generated.write_text(fixture.read_text(encoding="utf-8").replace("__SPECTRA_POSTGRES_URL__", url), encoding="utf-8")
                cli = run([str(ROOT / args.binary), "run", str(generated)], env, secret, 180)
            report["connection"]["cli_fixture"] = cli
            if cli["exit_code"] != 0:
                report["failures"].append("PostgreSQL Spectra fixture failed")
            encoded_spans = json.dumps(spans)
            attribute_keys = {
                key.lower()
                for span in spans
                for key in span.get("attributes", {})
            }
            command_leaks = [
                section
                for section in (
                    version,
                    *evidence.values(),
                    public_task_bridge,
                    http_parent,
                    cli,
                )
                if section.get("secret_leaked")
            ]
            credential_attribute = any(
                key in {"db.user", "server.user", "db.connection_string"}
                or any(marker in key for marker in ("password", "credential", "dsn"))
                for key in attribute_keys
            )
            secret_not_in_spans = (
                not contains_sensitive_value(encoded_spans, secret)
                and "postgres://" not in encoded_spans
                and "postgresql://" not in encoded_spans
                and not credential_attribute
            )
            query_attribute = any(
                key in {"db.statement", "db.query.text", "db.query.summary"}
                or key.endswith(".sql")
                for key in attribute_keys
            )
            report["security"] = {
                "credentials_not_exported": secret_not_in_spans,
                "command_output_leaks": len(command_leaks),
                "query_text_not_exported": not query_attribute and not any(
                    "SELECT pg_sleep" in json.dumps(span) or "spectra_r2505" in json.dumps(span)
                    for span in spans
                ),
            }
            if command_leaks or not secret_not_in_spans or not report["security"]["query_text_not_exported"]:
                report["failures"].append("credentials, DSN, or query text leaked into evidence")
            report["diagnostics"] = {
                "named_tests_proved": all(item.get("proved") for item in evidence.values())
                and public_task_bridge.get("proved"),
                "skips_are_non_certifying": True,
            }
            report["status"] = "passed" if not report["failures"] else "failed"

    output = Path(args.report)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(scrub(report, secret), indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(scrub(report, secret), indent=2, sort_keys=True))
    return 0 if report["status"] == "passed" or (
        report["status"] == "skipped_environment" and not args.require_database
    ) else 1


if __name__ == "__main__":
    raise SystemExit(main())
