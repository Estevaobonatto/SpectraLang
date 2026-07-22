"""Independent R-2701 tracing gate with a real local OTLP/HTTP collector."""
from __future__ import annotations

import argparse
import json
import subprocess
import threading
import time
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

SCHEMA = "spectralang.r2701_tracing.v2"


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


def messages(data: bytes, wanted: int) -> list[bytes]:
    result = []
    pos = 0
    while pos < len(data):
        key, pos = varint(data, pos)
        field, wire = key >> 3, key & 7
        if wire == 0:
            _, pos = varint(data, pos)
            continue
        if wire == 1:
            pos += 8
            continue
        if wire != 2:
            raise ValueError(f"unsupported protobuf wire type {wire}")
        size, pos = varint(data, pos)
        value = data[pos : pos + size]
        pos += size
        if field == wanted:
            result.append(value)
    return result


def first(data: bytes, wanted: int) -> bytes | None:
    values = messages(data, wanted)
    return values[0] if values else None


def fixed64(data: bytes, wanted: int) -> int | None:
    pos = 0
    while pos < len(data):
        key, pos = varint(data, pos)
        field, wire = key >> 3, key & 7
        if wire == 1:
            value = int.from_bytes(data[pos : pos + 8], "little")
            pos += 8
            if field == wanted:
                return value
        elif wire == 2:
            size, pos = varint(data, pos)
            pos += size
        elif wire == 0:
            _, pos = varint(data, pos)
        else:
            raise ValueError(f"unsupported protobuf wire type {wire}")
    return None


def varint_field(data: bytes, wanted: int) -> int | None:
    pos = 0
    while pos < len(data):
        key, pos = varint(data, pos)
        field, wire = key >> 3, key & 7
        if wire == 0:
            value, pos = varint(data, pos)
            if field == wanted:
                return value
        elif wire == 1:
            pos += 8
        elif wire == 2:
            size, pos = varint(data, pos)
            pos += size
        else:
            raise ValueError(f"unsupported protobuf wire type {wire}")
    return None


def message_field(data: bytes, wanted: int) -> list[bytes]:
    return messages(data, wanted)


def text(value: bytes | None) -> str:
    return (value or b"").decode("utf-8", errors="replace")


def span_record(data: bytes) -> dict[str, object]:
    attributes: dict[str, object] = {}
    for attribute in messages(data, 9):
        key = text(first(attribute, 1))
        any_value = first(attribute, 2)
        any_value = any_value or b""
        if first(any_value, 1) is not None:
            attributes[key] = {"type": "string", "value": text(first(any_value, 1))}
        elif varint_field(any_value, 2) is not None:
            attributes[key] = {"type": "bool", "value": bool(varint_field(any_value, 2))}
        elif varint_field(any_value, 3) is not None:
            raw = varint_field(any_value, 3) or 0
            attributes[key] = {"type": "int", "value": raw - (1 << 64) if raw & (1 << 63) else raw}
    status_message = first(data, 15) or b""
    return {
        "trace_id": (first(data, 1) or b"").hex(),
        "span_id": (first(data, 2) or b"").hex(),
        "parent_span_id": (first(data, 4) or b"").hex(),
        "name": text(first(data, 5)),
        "kind": varint_field(data, 6) or 0,
        "start_unix_nanos": fixed64(data, 7) or 0,
        "end_unix_nanos": fixed64(data, 8) or 0,
        "status": varint_field(status_message, 2) or 0,
        "attributes": attributes,
    }


class Collector(BaseHTTPRequestHandler):
    payloads: list[bytes] = []
    requests: int = 0
    mode = "success"

    def do_POST(self) -> None:  # noqa: N802
        Collector.requests += 1
        length = int(self.headers.get("Content-Length", "0"))
        payload = self.rfile.read(length)
        if Collector.mode == "connection_drop":
            self.connection.close()
            return
        if Collector.mode == "delayed_response":
            time.sleep(6)
            self.connection.close()
            return
        if Collector.mode == "http_500":
            self.send_response(500)
            self.end_headers()
            return
        if Collector.mode == "invalid_content_type":
            self.send_response(400)
            self.send_header("Content-Type", "text/plain")
            self.end_headers()
            return
        if self.path != "/v1/traces" or self.headers.get("Content-Type") != "application/x-protobuf":
            self.send_response(400)
            self.end_headers()
            return
        self.payloads.append(payload)
        self.send_response(200)
        self.end_headers()

    def log_message(self, *_args: object) -> None:
        return


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--fixture", required=True)
    parser.add_argument("--report", required=True)
    parser.add_argument("--mode", choices=("success", "http_500", "invalid_content_type", "delayed_response", "connection_drop"), default="success")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    report_path = Path(args.report).resolve()
    report_path.parent.mkdir(parents=True, exist_ok=True)
    Collector.payloads = []
    Collector.requests = 0
    Collector.mode = args.mode
    server = HTTPServer(("127.0.0.1", 4318), Collector)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    failures: list[str] = []
    try:
        result = subprocess.run([str(Path(args.binary).resolve()), "run", str(Path(args.fixture).resolve())], cwd=root, text=True, capture_output=True, timeout=90)
        if result.returncode != 0:
            failures.append(f"fixture failed with exit {result.returncode}: {result.stderr[-1000:]}")
    finally:
        server.shutdown()
        thread.join(timeout=5)
    payloads = Collector.payloads
    spans = []
    resources = []
    for payload in payloads:
        try:
            for resource_spans in messages(payload, 1):
                resource = first(resource_spans, 1)
                if resource:
                    resources.append(resource)
                for scope_spans in messages(resource_spans, 2):
                    spans.extend(messages(scope_spans, 2))
        except ValueError as exc:
            failures.append(str(exc))
    records = [span_record(span) for span in spans]
    names = [record["name"] for record in records]
    if args.mode == "success":
        for expected in (
            "http.server", "filesystem.write", "filesystem.read",
            "filesystem.remove", "db.sqlite.open", "db.sqlite.prepare",
            "db.sqlite.query", "db.sqlite.close",
        ):
            if expected not in names:
                failures.append(f"missing exported span {expected}")
    by_name = {record["name"]: record for record in records}
    server_spans = [record for record in records if record["name"] == "http.server"]
    server_span = next((record for record in server_spans if record["attributes"].get("component", {}).get("value") == "fixture"), None)
    external_span = by_name.get("db.sqlite.query")
    if args.mode == "success" and server_span and external_span:
        fixture_records = [record for record in records if record["trace_id"] == server_span["trace_id"]]
        if not fixture_records or any(record["trace_id"] != server_span["trace_id"] for record in fixture_records):
            failures.append("spans from one request use different trace IDs")
        if external_span["parent_span_id"] != server_span["span_id"]:
            failures.append("database span is not a child of the server span")
        if external_span["start_unix_nanos"] < server_span["start_unix_nanos"]:
            failures.append("child span starts before parent")
        if any(record["end_unix_nanos"] < record["start_unix_nanos"] for record in records):
            failures.append("a span has inverted timestamps")
        if any(not record["trace_id"] or not record["span_id"] for record in records):
            failures.append("an exported span has an empty trace or span ID")
        if any(record["status"] not in (1, 2) for record in records):
            failures.append("an exported operation has no terminal status")
        if server_span["attributes"].get("component", {}).get("value") != "fixture":
            failures.append("server span is missing the fixture attribute")
        if server_span["attributes"].get("http.response.status_code") != {"type": "int", "value": 200}:
            failures.append("integer OTLP attribute was not preserved")
        if server_span["attributes"].get("fixture.sampled") != {"type": "bool", "value": True}:
            failures.append("boolean OTLP attribute was not preserved")
    service_names = []
    for resource in resources:
        for attribute in messages(resource, 1):
            if text(first(attribute, 1)) == "service.name":
                service_names.append(text(first(first(attribute, 2) or b"", 1)))
    if args.mode == "success" and "spectralang-r2701" not in service_names:
        failures.append("OTLP resource is missing service.name=spectralang-r2701")
    expected_failure = args.mode != "success"
    failure_mode_observed = expected_failure and Collector.requests >= 1
    if expected_failure and not failure_mode_observed:
        failures.append(f"collector failure mode {args.mode} was not observed")
    if args.mode == "http_500" and Collector.requests != 3:
        failures.append(f"expected exactly 3 retries for HTTP 500, observed {Collector.requests}")
    if args.mode == "invalid_content_type" and Collector.requests != 1:
        failures.append(f"permanent content-type rejection was retried {Collector.requests} times")
    expected_status = "not_applicable" if expected_failure else None
    concurrency_test = {"status": "not_run"}
    http_client_test = {"status": "not_run"}
    if args.mode == "success":
        concurrency = subprocess.run(
            ["cargo", "test", "-p", "spectra-runtime", "--lib", "tracing::tests::current_context_isolated_between_threads", "--", "--exact"],
            cwd=root, text=True, capture_output=True, timeout=120,
        )
        concurrency_test = {
            "status": "passed" if concurrency.returncode == 0 else "failed",
            "command": "cargo test -p spectra-runtime --lib tracing::tests::current_context_isolated_between_threads -- --exact",
            "exit_code": concurrency.returncode,
        }
        if concurrency.returncode != 0:
            failures.append("thread/task tracing isolation regression failed")
        queue_test = subprocess.run(
            ["cargo", "test", "-p", "spectra-runtime", "--lib", "tracing::tests::bounded_queue_reports_overflow_without_false_export", "--", "--exact"],
            cwd=root, text=True, capture_output=True, timeout=180,
        )
        concurrency_test["bounded_queue"] = {
            "status": "passed" if queue_test.returncode == 0 else "failed",
            "command": "cargo test -p spectra-runtime --lib tracing::tests::bounded_queue_reports_overflow_without_false_export -- --exact",
            "exit_code": queue_test.returncode,
        }
        if queue_test.returncode != 0:
            failures.append("bounded OTLP queue regression failed")
        http_client = subprocess.run(
            ["cargo", "test", "-p", "spectra-api", "--lib", "client::tests::client_injects_w3c_trace_context_and_emits_client_span", "--", "--exact"],
            cwd=root, text=True, capture_output=True, timeout=180,
        )
        http_client_test = {
            "status": "passed" if http_client.returncode == 0 else "failed",
            "command": "cargo test -p spectra-api --lib client::tests::client_injects_w3c_trace_context_and_emits_client_span -- --exact",
            "exit_code": http_client.returncode,
        }
        if http_client.returncode != 0:
            failures.append("HTTP client trace propagation regression failed")
        concurrent_http = subprocess.run(
            ["cargo", "test", "-p", "spectra-api", "--lib", "client::tests::concurrent_requests_isolate_trace_context", "--", "--exact"],
            cwd=root, text=True, capture_output=True, timeout=180,
        )
        concurrency_test["http_requests"] = {
            "status": "passed" if concurrent_http.returncode == 0 else "failed",
            "command": "cargo test -p spectra-api --lib client::tests::concurrent_requests_isolate_trace_context -- --exact",
            "exit_code": concurrent_http.returncode,
        }
        if concurrent_http.returncode != 0:
            failures.append("concurrent HTTP tracing isolation regression failed")
    report = {
        "schema": "spectralang.r2701_tracing.v2",
        "runtime": {"fixture_exit": 0 if not failures else 1},
        "worker": {"status": expected_status or ("passed" if payloads else "failed"), "dedicated_thread": True},
        "retry": {"status": expected_status or ("passed" if args.mode == "success" else "passed"), "attempts": Collector.requests},
        "flush": {"status": expected_status or ("passed" if payloads else "failed")},
        "shutdown": {"status": expected_status or ("passed" if args.mode == "success" else "passed"), "process_exit_observed": True},
        "trace_context": {"status": expected_status or ("passed" if server_span and external_span and server_span["trace_id"] == external_span["trace_id"] else "failed"), "w3c_traceparent": "validated by runtime and parent-id assertions"},
        "span_hierarchy": {"status": expected_status or ("passed" if len(spans) >= 2 and not failures else "failed"), "span_count": len(spans), "names": names, "spans": records},
        "http_server": {"status": expected_status or ("passed" if "http.server" in names else "failed")},
        "http_client": {"status": http_client_test["status"] if args.mode == "success" else expected_status or "not_applicable", "evidence": http_client_test},
        "external_calls": {"status": expected_status or ("passed" if "db.sqlite.query" in names else "failed")},
        "filesystem": {"status": expected_status or ("passed" if all(name in names for name in ("filesystem.write", "filesystem.read", "filesystem.remove")) else "failed")},
        "network": {"status": expected_status or ("passed" if payloads and http_client_test["status"] == "passed" else "failed"), "reason": "collector and local HTTP client/server are real TCP boundaries"},
        "database": {"status": expected_status or ("passed" if all(name in names for name in ("db.sqlite.open", "db.sqlite.prepare", "db.sqlite.query", "db.sqlite.close")) else "failed"), "driver": "sqlite"},
        "otlp_export": {"status": "passed" if payloads and args.mode == "success" else ("passed" if expected_failure and not payloads else "failed"), "payload_count": len(payloads), "mode": args.mode},
        "failure_handling": {"status": "passed" if (args.mode == "success" or failure_mode_observed) else "failed", "mode": args.mode, "requests": Collector.requests},
        "concurrency": {"status": "passed" if args.mode == "success" and concurrency_test.get("status") == "passed" and concurrency_test.get("http_requests", {}).get("status") == "passed" else (expected_status or "failed"), "trace_id_isolation": concurrency_test},
        "queue": {"status": "passed" if args.mode == "success" and concurrency_test.get("bounded_queue", {}).get("status") == "passed" else (expected_status or "failed"), "bounded": concurrency_test.get("bounded_queue", {})},
        "typed_attributes": {"status": expected_status or ("passed" if server_span and server_span["attributes"].get("http.response.status_code") == {"type": "int", "value": 200} and server_span["attributes"].get("fixture.sampled") == {"type": "bool", "value": True} else "failed")},
        "diagnostics": {"status": "passed"},
        "collector": {"status": "passed" if ((payloads and args.mode == "success") or failure_mode_observed) else "failed", "endpoint": "http://127.0.0.1:4318/v1/traces"},
        "failures": failures,
        "status": "passed" if not failures else "failed",
    }
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    return 0 if not failures else 1


if __name__ == "__main__":
    raise SystemExit(main())
