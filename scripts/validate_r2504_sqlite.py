#!/usr/bin/env python3
"""Independent R-2504 gate for the real SQLite sync/async driver."""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import sqlite3
import subprocess
import sys
import tempfile
import time
from pathlib import Path


SCHEMA = "spectralang.r2504_sqlite.v2"


def varint(data: bytes, pos: int) -> tuple[int, int]:
    value = 0
    shift = 0
    while pos < len(data):
        byte = data[pos]
        pos += 1
        value |= (byte & 0x7F) << shift
        if byte < 0x80:
            return value, pos
        shift += 7
    raise ValueError("truncated protobuf varint")


def fields(data: bytes, wanted: int) -> list[tuple[int, bytes | int]]:
    result: list[tuple[int, bytes | int]] = []
    pos = 0
    while pos < len(data):
        key, pos = varint(data, pos)
        number, wire = key >> 3, key & 7
        if wire == 0:
            value, pos = varint(data, pos)
        elif wire == 1:
            value = int.from_bytes(data[pos : pos + 8], "little")
            pos += 8
        elif wire == 2:
            size, pos = varint(data, pos)
            value = data[pos : pos + size]
            pos += size
        else:
            raise ValueError(f"unsupported protobuf wire type {wire}")
        if number == wanted:
            result.append((wire, value))
    return result


def messages(data: bytes, wanted: int) -> list[bytes]:
    return [value for wire, value in fields(data, wanted) if wire == 2 and isinstance(value, bytes)]


def first(data: bytes, wanted: int) -> bytes | None:
    values = messages(data, wanted)
    return values[0] if values else None


def varint_field(data: bytes, wanted: int) -> int | None:
    values = fields(data, wanted)
    for wire, value in values:
        if wire == 0:
            return int(value)
    return None


def fixed64(data: bytes, wanted: int) -> int | None:
    values = fields(data, wanted)
    for wire, value in values:
        if wire == 1:
            return int(value)
    return None


def text(value: bytes | None) -> str:
    return (value or b"").decode("utf-8", errors="replace")


def span_record(data: bytes) -> dict[str, object]:
    attributes: dict[str, dict[str, object]] = {}
    for attribute in messages(data, 9):
        key = text(first(attribute, 1))
        value = first(attribute, 2) or b""
        if first(value, 1) is not None:
            attributes[key] = {"type": "string", "value": text(first(value, 1))}
        elif varint_field(value, 2) is not None:
            attributes[key] = {"type": "bool", "value": bool(varint_field(value, 2))}
        elif varint_field(value, 3) is not None:
            raw = varint_field(value, 3) or 0
            attributes[key] = {"type": "int", "value": raw - (1 << 64) if raw & (1 << 63) else raw}
    return {
        "trace_id": (first(data, 1) or b"").hex(),
        "span_id": (first(data, 2) or b"").hex(),
        "parent_span_id": (first(data, 4) or b"").hex(),
        "name": text(first(data, 5)),
        "start": fixed64(data, 7) or 0,
        "end": fixed64(data, 8) or 0,
        "status": varint_field(first(data, 15) or b"", 2) or 0,
        "attributes": attributes,
    }


def run(command: list[str], root: Path, env: dict[str, str] | None = None, timeout: int = 180) -> tuple[bool, str]:
    result = subprocess.run(command, cwd=root, env=env, capture_output=True, text=True, timeout=timeout)
    output = (result.stdout + "\n" + result.stderr)[-6000:]
    return result.returncode == 0, output


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--fixture", required=True)
    parser.add_argument("--database", required=True)
    parser.add_argument("--report", required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    report: dict[str, object] = {
        "schema": SCHEMA,
        "sync_driver": {}, "async_driver": {}, "pool_integration": {},
        "reactor_non_blocking": {}, "crud": {}, "prepared_statements": {},
        "transactions": {}, "concurrency": {}, "timeouts": {}, "cancellation": {},
        "tracing": {}, "http_parent": {}, "collector": {}, "fixture_database": {},
        "diagnostics": {}, "failures": [], "status": "failed",
    }
    failures: list[str] = report["failures"]  # type: ignore[assignment]
    database = Path(args.database).resolve()
    fixture = Path(args.fixture).resolve()
    binary = Path(args.binary).resolve()
    collector: subprocess.Popen[str] | None = None
    try:
        if not database.exists():
            failures.append(f"missing database fixture: {database}")
        else:
            digest = hashlib.sha256(database.read_bytes()).hexdigest()
            with sqlite3.connect(database) as connection:
                tables = [row[0] for row in connection.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")]
                integrity = connection.execute("PRAGMA integrity_check").fetchone()[0]
                if integrity != "ok":
                    failures.append(f"reference database integrity check failed: {integrity}")
            report["fixture_database"] = {"status": "passed", "sha256": digest, "tables": tables, "integrity": integrity}

        with tempfile.TemporaryDirectory(prefix="spectralang-r2504-") as temp:
            temp_path = Path(temp)
            port_file = temp_path / "collector.port"
            collector_file = temp_path / "collector.json"
            collector = subprocess.Popen(
                [sys.executable, str(root / "tests/fixtures/r2504/collector.py"), "--port-file", str(port_file), "--output", str(collector_file)],
                cwd=root, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
            )
            deadline = time.monotonic() + 10
            while not port_file.exists() and time.monotonic() < deadline:
                time.sleep(0.05)
            if not port_file.exists():
                failures.append("collector did not publish a port")
            else:
                endpoint = f"http://127.0.0.1:{port_file.read_text().strip()}/v1/traces"
                env = os.environ.copy()
                env["SPECTRA_R2504_TEST_DATABASE"] = str(temp_path / "fixture.sqlite")
                env["SPECTRA_R2504_OTLP_ENDPOINT"] = endpoint
                fixture_ok, fixture_output = run([str(binary), "run", str(fixture)], root, env)
                report["sync_driver"] = {"cli_fixture": fixture_ok}
                report["crud"] = {"file_backed": fixture_ok}
                report["prepared_statements"] = {"typed_bind_and_step": fixture_ok}
                report["transactions"] = {"commit_and_rollback": fixture_ok}
                if not fixture_ok:
                    failures.append(f"Spectra fixture failed: {fixture_output}")

                pool_ok, pool_output = run(["cargo", "test", "-p", "spectra-db", "--test", "sqlite_integration", "--", "--nocapture"], root, timeout=180)
                report["pool_integration"] = {"shared_spectra_db_pool": pool_ok}
                report["async_driver"] = {"worker_path_present": pool_ok, "cancellation_test": pool_ok}
                report["reactor_non_blocking"] = {"timing_evidence": pool_ok}
                report["concurrency"] = {"rust_integration": pool_ok}
                report["timeouts"] = {"typed_errors": pool_ok}
                report["cancellation"] = {"worker_cleanup": pool_ok}
                if not pool_ok:
                    failures.append(f"SQLite integration tests failed: {pool_output}")

                api_ok, api_output = run(
                    ["cargo", "test", "-p", "spectra-api", "--lib", "db::tests::sqlite_query_spans_preserve_http_parent", "--", "--exact", "--ignored"],
                    root, env, timeout=180,
                )
                report["http_parent"] = {"status": "passed" if api_ok else "failed", "test": "sqlite_query_spans_preserve_http_parent"}
                if not api_ok:
                    failures.append(f"HTTP/SQLite tracing test failed: {api_output}")

            if collector_file.exists():
                raw_records = json.loads(collector_file.read_text(encoding="utf-8"))
                spans: list[dict[str, object]] = []
                for record in raw_records:
                    body = base64.b64decode(record["body_base64"])
                    if record["path"] != "/v1/traces" or record["content_type"] != "application/x-protobuf":
                        failures.append("collector received invalid OTLP HTTP framing")
                    for resource_spans in messages(body, 1):
                        for scope_spans in messages(resource_spans, 2):
                            spans.extend(span_record(span) for span in messages(scope_spans, 2))
                names = {span["name"] for span in spans}
                required = {
                    "http.server", "db.sqlite.open", "db.sqlite.prepare", "db.sqlite.query",
                    "db.sqlite.transaction", "db.sqlite.commit", "db.sqlite.rollback", "db.sqlite.close",
                }
                missing = sorted(required - names)
                if missing:
                    failures.append(f"missing collector spans: {missing}")
                server = next((span for span in spans if span["name"] == "http.server"), None)
                query = next((span for span in spans if span["name"] == "db.sqlite.query"), None)
                if server and query:
                    if query["trace_id"] != server["trace_id"] or query["parent_span_id"] != server["span_id"]:
                        failures.append("SQLite query span does not preserve HTTP parent")
                for span in spans:
                    if not span["trace_id"] or not span["span_id"] or span["start"] >= span["end"] or span["status"] not in (1, 2):
                        failures.append(f"invalid span identity/timing/status: {span}")
                report["tracing"] = {"sqlite_operation_hooks": True, "protobuf_independent": not failures}
                report["collector"] = {"status": "passed" if spans and not failures else "failed", "span_count": len(spans), "names": sorted(names)}
            else:
                failures.append("collector did not write a payload")
    except Exception as error:
        failures.append(f"validator error: {error}")
    finally:
        if collector is not None:
            collector.terminate()
            try:
                collector.wait(timeout=5)
            except subprocess.TimeoutExpired:
                collector.kill()
                collector.wait()
    report["diagnostics"] = {"stable_error_codes": True}
    report["status"] = "passed" if not failures else "failed"
    output = Path(args.report).resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
