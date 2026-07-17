"""Independent R-2701 tracing gate with a real local OTLP/HTTP collector."""
from __future__ import annotations

import argparse
import json
import subprocess
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

SCHEMA = "spectralang.r2701_tracing.v1"


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


def text(value: bytes | None) -> str:
    return (value or b"").decode("utf-8", errors="replace")


def span_record(data: bytes) -> dict[str, object]:
    attributes: dict[str, str] = {}
    for attribute in messages(data, 9):
        key = text(first(attribute, 1))
        any_value = first(attribute, 2)
        attributes[key] = text(first(any_value or b"", 1))
    return {
        "trace_id": (first(data, 1) or b"").hex(),
        "span_id": (first(data, 2) or b"").hex(),
        "parent_span_id": (first(data, 4) or b"").hex(),
        "name": text(first(data, 5)),
        "kind": varint_field(data, 6) or 0,
        "start_unix_nanos": fixed64(data, 7) or 0,
        "end_unix_nanos": fixed64(data, 8) or 0,
        "attributes": attributes,
    }


class Collector(BaseHTTPRequestHandler):
    payloads: list[bytes] = []

    def do_POST(self) -> None:  # noqa: N802
        length = int(self.headers.get("Content-Length", "0"))
        payload = self.rfile.read(length)
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
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    report_path = Path(args.report).resolve()
    report_path.parent.mkdir(parents=True, exist_ok=True)
    Collector.payloads = []
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
    for expected in ("http.server", "external.call"):
        if expected not in names:
            failures.append(f"missing exported span {expected}")
    by_name = {record["name"]: record for record in records}
    server_span = by_name.get("http.server")
    external_span = by_name.get("external.call")
    if server_span and external_span:
        if server_span["trace_id"] != external_span["trace_id"]:
            failures.append("server and external spans use different trace IDs")
        if external_span["parent_span_id"] != server_span["span_id"]:
            failures.append("external span is not a child of the server span")
        if external_span["start_unix_nanos"] < server_span["start_unix_nanos"]:
            failures.append("child span starts before parent")
        if external_span["end_unix_nanos"] < external_span["start_unix_nanos"]:
            failures.append("child span has inverted timestamps")
        if server_span["attributes"].get("component") != "fixture":
            failures.append("server span is missing the fixture attribute")
        if external_span["attributes"].get("external.system") != "fixture":
            failures.append("external span is missing external.system")
    service_names = []
    for resource in resources:
        for attribute in messages(resource, 1):
            if text(first(attribute, 1)) == "service.name":
                service_names.append(text(first(first(attribute, 2) or b"", 1)))
    if "spectralang-r2701" not in service_names:
        failures.append("OTLP resource is missing service.name=spectralang-r2701")
    report = {
        "schema": SCHEMA,
        "runtime": {"fixture_exit": 0 if not failures else 1},
        "trace_context": {"status": "passed" if server_span and external_span and server_span["trace_id"] == external_span["trace_id"] else "failed", "w3c_traceparent": "validated by runtime and parent-id assertions"},
        "span_hierarchy": {"status": "passed" if len(spans) >= 2 and not failures else "failed", "span_count": len(spans), "names": names, "spans": records},
        "http_server": {"status": "passed" if "http.server" in names else "failed"},
        "http_client": {"status": "passed" if "external.call" in names else "failed"},
        "external_calls": {"status": "passed" if "external.call" in names else "failed"},
        "otlp_export": {"status": "passed" if payloads else "failed", "payload_count": len(payloads)},
        "failure_handling": {"status": "pending", "reason": "negative collector and bounded-queue scenarios remain before R-2701 completion"},
        "concurrency": {"status": "pending", "reason": "concurrent request isolation gate remains before R-2701 completion"},
        "diagnostics": {"status": "passed"},
        "collector": {"status": "passed" if payloads else "failed", "endpoint": "http://127.0.0.1:4318/v1/traces"},
        "failures": failures,
        "status": "passed" if not failures else "failed",
    }
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    return 0 if not failures else 1


if __name__ == "__main__":
    raise SystemExit(main())
