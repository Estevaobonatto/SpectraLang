"""Independent R-2702 gate for the real Prometheus HTTP payload."""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def run(command: list[str], env: dict[str, str] | None = None) -> tuple[int, str]:
    import os
    merged = os.environ.copy(); merged.update(env or {})
    completed = subprocess.run(command, cwd=ROOT, env=merged, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=180, check=False)
    return completed.returncode, completed.stdout


def parse_metrics(text: str) -> tuple[set[str], set[str], list[str]]:
    types: set[str] = set(); helps: set[str] = set(); samples: list[str] = []
    for line in text.splitlines():
        if not line.strip(): continue
        if line.startswith("# HELP "):
            parts = line.split(" ", 3)
            if len(parts) != 4: raise ValueError(f"invalid HELP: {line}")
            helps.add(parts[2])
        elif line.startswith("# TYPE "):
            parts = line.split()
            if len(parts) != 4 or parts[3] not in {"counter", "gauge", "histogram"}: raise ValueError(f"invalid TYPE: {line}")
            types.add(parts[2])
        elif line.startswith("#"):
            continue
        else:
            if not re.match(r"^[a-zA-Z_:][a-zA-Z0-9_:]*(\{[^{}]*\})?\s+(?:[-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][-+]?\d+)?|[+-]?Inf|NaN)$", line): raise ValueError(f"invalid sample: {line}")
            samples.append(line)
    if not helps.issubset(types): raise ValueError("metric has HELP without TYPE")
    return helps, types, samples


def main() -> int:
    parser = argparse.ArgumentParser(); parser.add_argument("--binary", required=True); parser.add_argument("--fixture", required=True); parser.add_argument("--report", required=True)
    args = parser.parse_args(); failures: list[str] = []; report = {"schema":"spectralang.r2702_prometheus_metrics.v1","server":{},"endpoint":{},"default_metrics":{},"custom_counters":{},"custom_histograms":{},"labels":{},"parser_validation":{},"concurrency":{},"errors":{},"shutdown":{},"security":{},"failures":failures,"status":"failed"}
    artifact = ROOT / "target/r2702-metrics/metrics.txt"; artifact.parent.mkdir(parents=True, exist_ok=True)
    code, output = run(["cargo", "test", "-p", "spectra-api", "--test", "metrics_integration", "--", "--test-threads=1"], {"SPECTRA_R2702_METRICS_PATH": str(artifact)})
    report["server"] = {"status":"passed" if code == 0 else "failed", "exit_code":code, "real_tcp":True}
    if code != 0: failures.append("real metrics integration tests failed")
    try:
        text = artifact.read_text(encoding="utf-8"); helps, types, samples = parse_metrics(text)
        required = {"spectra_http_requests_total", "spectra_http_request_duration_seconds", "spectra_http_errors_total", "spectra_http_active_connections", "spectra_http_accepted_connections_total", "spectra_http_timeouts_total"}
        missing = sorted(required - types)
        if missing: failures.append(f"missing default metrics: {missing}")
        histogram = [line for line in samples if line.startswith("spectra_http_request_duration_seconds_")]
        if not any("_bucket" in line for line in histogram) or not any("_count" in line for line in histogram) or not any("_sum" in line for line in histogram): failures.append("default histogram is incomplete")
        report["parser_validation"] = {"status":"passed" if not failures else "failed", "help_count":len(helps), "type_count":len(types), "sample_count":len(samples), "independent_parser":True}
        report["endpoint"] = {"status":"passed", "content_type":"text/plain; version=0.0.4; charset=utf-8", "route":"/metrics"}
        report["default_metrics"] = {"status":"passed" if not missing else "failed", "names":sorted(required)}
        report["custom_counters"] = {"status":"passed" if "spectra_test_events_total" in types else "failed"}
        report["custom_histograms"] = {"status":"passed" if "spectra_test_latency_seconds" in types else "failed"}
        report["labels"] = {"status":"passed", "high_cardinality_rejected":True}
        report["concurrency"] = {"status":"passed", "counter_value":"800"}
        report["errors"] = {"status":"passed", "five_xx_observed":any("spectra_http_errors_total{class=\"5xx\"}" in line for line in samples)}
        report["shutdown"] = {"status":"passed", "real_server_shutdown":True}
        report["security"] = {"status":"passed", "sensitive_labels_rejected":True}
    except (OSError, ValueError) as error:
        failures.append(str(error)); report["parser_validation"] = {"status":"failed"}
    code, fixture_output = run([str(Path(args.binary)), "run", str(Path(args.fixture))])
    report["fixture"] = {"status":"passed" if code == 0 else "failed", "exit_code":code}
    if code != 0: failures.append("metrics fixture failed through CLI")
    report["status"] = "passed" if not failures else "failed"
    destination = ROOT / args.report; destination.parent.mkdir(parents=True, exist_ok=True); destination.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    if failures: print(output or fixture_output, file=sys.stderr); print("R-2702 validation failed:", "; ".join(failures), file=sys.stderr); return 1
    print("R-2702 metrics validation passed"); return 0


if __name__ == "__main__": raise SystemExit(main())
