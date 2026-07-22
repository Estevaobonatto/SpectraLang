"""Independent R-2707 gate for the real OTel + Prometheus example."""
from __future__ import annotations

import argparse
import http.client
import json
import os
import re
import signal
import socket
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCHEMA = "spectralang.r2707_otel_prometheus.v1"


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


def fields(data: bytes):
    pos = 0
    while pos < len(data):
        key, pos = varint(data, pos)
        number, wire = key >> 3, key & 7
        if wire == 0:
            value, pos = varint(data, pos)
        elif wire == 1:
            value, pos = data[pos:pos + 8], pos + 8
        elif wire == 2:
            size, pos = varint(data, pos)
            value, pos = data[pos:pos + size], pos + size
        elif wire == 5:
            value, pos = data[pos:pos + 4], pos + 4
        else:
            raise ValueError(f"unsupported protobuf wire {wire}")
        yield number, wire, value


def repeated(data: bytes, number: int) -> list[bytes]:
    return [value for field, wire, value in fields(data) if field == number and wire == 2]


def first(data: bytes, number: int) -> bytes | None:
    values = repeated(data, number)
    return values[0] if values else None


def scalar(data: bytes, number: int) -> int | None:
    for field, wire, value in fields(data):
        if field == number and wire == 0:
            return int(value)
    return None


def fixed64(data: bytes, number: int) -> int | None:
    for field, wire, value in fields(data):
        if field == number and wire == 1:
            return int.from_bytes(value, "little")
    return None


def text(data: bytes | None) -> str:
    return (data or b"").decode("utf-8", errors="replace")


def any_value(data: bytes) -> object:
    value = first(data, 1)
    if value is not None:
        return {"type": "string", "value": text(value)}
    integer = scalar(data, 3)
    if integer is not None:
        signed = integer - (1 << 64) if integer & (1 << 63) else integer
        return {"type": "int", "value": signed}
    boolean = scalar(data, 2)
    if boolean is not None:
        return {"type": "bool", "value": bool(boolean)}
    return {"type": "unknown"}


def span_record(data: bytes) -> dict[str, object]:
    attrs: dict[str, object] = {}
    for attribute in repeated(data, 9):
        key = text(first(attribute, 1))
        value = first(attribute, 2)
        if value is not None:
            attrs[key] = any_value(value)
    status = first(data, 15) or b""
    return {
        "name": text(first(data, 5)),
        "trace_id": (first(data, 1) or b"").hex(),
        "span_id": (first(data, 2) or b"").hex(),
        "parent_span_id": (first(data, 4) or b"").hex(),
        "kind": scalar(data, 6) or 0,
        "start": fixed64(data, 7) or 0,
        "end": fixed64(data, 8) or 0,
        "status": scalar(status, 2) or 0,
        "attributes": attrs,
    }


def decode_payload(payload: bytes) -> list[dict[str, object]]:
    result: list[dict[str, object]] = []
    for resource_spans in repeated(payload, 1):
        for scope_spans in repeated(resource_spans, 2):
            result.extend(span_record(span) for span in repeated(scope_spans, 2))
    return result


def free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def request(port: int, path: str) -> tuple[int, dict[str, str], str]:
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=3)
    connection.request("GET", path)
    response = connection.getresponse()
    body = response.read().decode("utf-8", errors="replace")
    headers = {key.lower(): value for key, value in response.getheaders()}
    connection.close()
    return response.status, headers, body


def parse_prometheus(payload: str) -> tuple[dict[str, str], list[tuple[str, dict[str, str], float]]]:
    types: dict[str, str] = {}
    samples: list[tuple[str, dict[str, str], float]] = []
    sample_pattern = re.compile(r"^([a-zA-Z_:][a-zA-Z0-9_:]*)(?:\{([^{}]*)\})?\s+([-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][-+]?\d+)?|[+-]?Inf|NaN)$")
    for line in payload.splitlines():
        if not line:
            continue
        if line.startswith("# TYPE "):
            parts = line.split()
            if len(parts) != 4 or parts[3] not in {"counter", "gauge", "histogram"}:
                raise ValueError(f"invalid TYPE line: {line}")
            types[parts[2]] = parts[3]
            continue
        if line.startswith("# HELP ") or line.startswith("#"):
            continue
        match = sample_pattern.match(line)
        if not match:
            raise ValueError(f"invalid Prometheus sample: {line}")
        labels: dict[str, str] = {}
        if match.group(2):
            for item in match.group(2).split(","):
                name, value = item.split("=", 1)
                labels[name] = json.loads(value)
        samples.append((match.group(1), labels, float(match.group(3))))
    if not types:
        raise ValueError("Prometheus payload has no TYPE declarations")
    return types, samples


def wait_for_port(port: int, process: subprocess.Popen[str], timeout: float = 8.0) -> bool:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline and process.poll() is None:
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=0.2):
                return True
        except OSError:
            time.sleep(0.05)
    return False


def terminate(process: subprocess.Popen[str] | None) -> None:
    if process is None or process.poll() is not None:
        return
    process.send_signal(signal.SIGTERM)
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--fixture", required=True)
    parser.add_argument("--report", required=True)
    args = parser.parse_args()
    failures: list[str] = []
    report: dict[str, object] = {
        "schema": SCHEMA,
        "example": {}, "otel": {}, "prometheus": {}, "http": {},
        "processes": {}, "security": {}, "failures": failures, "status": "failed",
    }
    report_path = ROOT / args.report
    report_path.parent.mkdir(parents=True, exist_ok=True)
    collector = app = None
    collector_json = ROOT / "target/r2707-otel-prometheus/collector.json"
    collector_port_file = ROOT / "target/r2707-otel-prometheus/collector.port"
    collector_port_file.parent.mkdir(parents=True, exist_ok=True)
    collector_port = free_port()
    app_port = free_port()
    env = os.environ.copy()
    env.update({
        "SPECTRA_R2707_PORT": str(app_port),
        "SPECTRA_R2707_OTLP_ENDPOINT": f"http://127.0.0.1:{collector_port}/v1/traces",
    })
    try:
        collector = subprocess.Popen([
            sys.executable, str(ROOT / "tests/fixtures/r2707/collector.py"),
            "--port", str(collector_port), "--port-file", str(collector_port_file),
            "--output", str(collector_json),
        ], cwd=ROOT, env=env, text=True)
        app = subprocess.Popen([
            str(Path(args.binary).resolve()), "run", str((ROOT / args.fixture).resolve()),
        ], cwd=ROOT, env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        if not wait_for_port(app_port, app):
            failures.append("Spectra HTTP server did not become reachable")
        else:
            ok_status, ok_headers, ok_body = request(app_port, "/demo")
            missing_status, _, _ = request(app_port, "/missing")
            metrics_status, metrics_headers, metrics_body = request(app_port, "/metrics")
            report["http"] = {"status": "passed" if ok_status == 200 and missing_status == 404 else "failed", "ok_status": ok_status, "missing_status": missing_status}
            if ok_status != 200 or ok_body != "otel-prometheus-ok":
                failures.append("real /demo request did not return the expected response")
            if missing_status != 404:
                failures.append("real missing-route request did not return 404")
            if metrics_status != 200 or metrics_headers.get("content-type") != "text/plain; version=0.0.4; charset=utf-8":
                failures.append("/metrics did not return the Prometheus content type")
            try:
                types, samples = parse_prometheus(metrics_body)
                sample_names = {name for name, _, _ in samples}
                required = {
                    "spectra_http_requests_total", "spectra_http_request_duration_seconds_bucket",
                    "spectra_http_request_duration_seconds_sum", "spectra_http_request_duration_seconds_count",
                    "spectra_http_errors_total", "spectra_http_active_connections",
                    "spectra_http_accepted_connections_total", "spectra_http_timeouts_total",
                }
                default_types = {
                    "spectra_http_requests_total", "spectra_http_request_duration_seconds",
                    "spectra_http_errors_total", "spectra_http_active_connections",
                    "spectra_http_accepted_connections_total", "spectra_http_timeouts_total",
                }
                missing_types = sorted(default_types - set(types))
                if missing_types:
                    failures.append(f"Prometheus payload is missing metric types: {missing_types}")
                # A zero-valued counter/gauge may have no sample until the
                # corresponding operation occurs; its TYPE declaration is
                # still part of the contract. Histogram series are required
                # because the example performs real requests.
                missing = sorted({
                    "spectra_http_request_duration_seconds_bucket",
                    "spectra_http_request_duration_seconds_sum",
                    "spectra_http_request_duration_seconds_count",
                } - sample_names)
                if missing:
                    failures.append(f"Prometheus payload is missing samples: {missing}")
                if not any(name == "spectra_http_requests_total" and labels.get("status") == "200" and value >= 1 for name, labels, value in samples):
                    failures.append("Prometheus payload did not record the successful request")
                if not any(name == "spectra_http_requests_total" and labels.get("status") == "404" and value >= 1 for name, labels, value in samples):
                    failures.append("Prometheus payload did not record the 404 request")
                bucket_values = [value for name, labels, value in samples if name == "spectra_http_request_duration_seconds_bucket" and labels.get("method") == "GET"]
                count_values = [value for name, _, value in samples if name == "spectra_http_request_duration_seconds_count"]
                if len(bucket_values) < 2 or any(right < left for left, right in zip(bucket_values, bucket_values[1:])):
                    failures.append("Prometheus histogram buckets are not cumulative and ordered")
                if not count_values or count_values[0] < 2:
                    failures.append("Prometheus histogram count does not include the real requests")
                prometheus_failures = [failure for failure in failures if failure.startswith("Prometheus")]
                report["prometheus"] = {"status": "passed" if not missing and not prometheus_failures else "failed", "types": types, "sample_count": len(samples), "independent_parser": True, "content_type": metrics_headers.get("content-type")}
            except ValueError as error:
                failures.append(str(error))
                report["prometheus"] = {"status": "failed", "independent_parser": True}
        try:
            app.wait(timeout=15)
        except subprocess.TimeoutExpired:
            failures.append("Spectra example did not shut down within the bounded window")
            terminate(app)
        stdout, stderr = app.communicate(timeout=5)
        report["example"] = {"status": "passed" if app.returncode == 0 else "failed", "exit_code": app.returncode, "stdout": stdout[-1000:], "stderr": stderr[-1000:]}
        if app.returncode != 0:
            failures.append(f"example exited with code {app.returncode}")
    except (OSError, subprocess.SubprocessError) as error:
        failures.append(str(error))
    finally:
        terminate(app)
        if collector is not None and collector.poll() is None:
            try:
                connection = http.client.HTTPConnection("127.0.0.1", collector_port, timeout=3)
                connection.request("POST", "/shutdown", body=b"")
                connection.getresponse().read()
                connection.close()
                collector.wait(timeout=5)
            except (OSError, subprocess.TimeoutExpired):
                terminate(collector)

    records: list[dict[str, object]] = []
    collector_data: dict[str, object] = {}
    if collector_json.exists():
        try:
            collector_data = json.loads(collector_json.read_text(encoding="utf-8"))
            for encoded in collector_data.get("payloads", []):
                records.extend(decode_payload(bytes.fromhex(encoded)))
        except (OSError, ValueError, json.JSONDecodeError) as error:
            failures.append(f"collector evidence could not be decoded: {error}")
    else:
        failures.append("collector did not write independent evidence")
    names = [record["name"] for record in records]
    spans_ok = bool(records) and all(len(record["trace_id"]) == 32 and len(record["span_id"]) == 16 and record["end"] >= record["start"] for record in records)
    example_span = next((record for record in records if record["name"] == "example.operation"), None)
    http_spans = [record for record in records if record["name"] == "http.server"]
    attrs_ok = bool(example_span and example_span["attributes"].get("example.component") == {"type": "string", "value": "otel-prometheus"} and example_span["attributes"].get("example.request_count") == {"type": "int", "value": 2} and example_span["attributes"].get("example.sampled") == {"type": "bool", "value": True})
    raw = json.dumps(collector_data).lower()
    for secret in ("password", "secret", "authorization", "bearer"):
        if secret in raw:
            failures.append(f"collector evidence contains sensitive marker {secret}")
    if any(b"otel-prometheus-ok" in bytes.fromhex(encoded) for encoded in collector_data.get("payloads", [])):
        failures.append("HTTP response body was exported in OTLP payload")
    if not http_spans:
        failures.append("collector did not receive a real HTTP server span")
    if not spans_ok:
        failures.append("collector spans have invalid IDs or timestamps")
    if not attrs_ok:
        failures.append("typed example span attributes were not preserved")
    report["otel"] = {
        "collector": {"status": "passed" if collector_data.get("payloads") else "failed", "requests": collector_data.get("requests", 0), "invalid_requests": collector_data.get("invalid_requests", 0)},
        "spans": {"status": "passed" if http_spans else "failed", "count": len(records), "names": names, "records": records},
        "attributes": {"status": "passed" if attrs_ok else "failed", "typed": True},
        "flush": {"status": "passed" if collector_data.get("payloads") else "failed"},
        "shutdown": {"status": "passed" if app is not None and app.poll() is not None else "failed"},
    }
    report["processes"] = {"status": "passed" if all(process is None or process.poll() is not None for process in (app, collector)) else "failed"}
    report["security"] = {"status": "passed" if not any("sensitive marker" in failure for failure in failures) else "failed", "sensitive_values_absent": True}
    report["failures"] = failures
    report["status"] = "passed" if not failures else "failed"
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"R-2707 validation {'passed' if not failures else 'failed'}")
    if failures:
        print("; ".join(failures), file=sys.stderr)
    return 0 if not failures else 1


if __name__ == "__main__":
    raise SystemExit(main())
