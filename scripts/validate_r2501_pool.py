#!/usr/bin/env python3
"""Independent R-2501 gate using a real local TCP server and cargo tests."""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    report = {
        "schema": "spectralang.r2501_connection_pool.v1",
        "configuration": {"real_tcp_server": True},
        "capacity": {}, "acquisition": {}, "timeouts": {},
        "idle_reaping": {}, "recovery": {}, "fairness": {},
        "cancellation": {}, "shutdown": {}, "metrics": {},
        "tracing": {}, "failures": [], "status": "failed",
    }
    server = subprocess.Popen(
        [sys.executable, str(root / "tests/fixtures/r2501_tcp_server.py")],
        cwd=root, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        text=True,
    )
    try:
        port_line = server.stdout.readline().strip() if server.stdout else ""
        if not port_line.isdigit():
            report["failures"].append("real TCP server did not publish a port")
        else:
            env = os.environ.copy()
            env["SPECTRA_POOL_TEST_SERVER"] = f"127.0.0.1:{port_line}"
            started = time.monotonic()
            result = subprocess.run(
                ["cargo", "test", "-p", "spectra-db", "--test", "pool_integration", "--", "--nocapture"],
                cwd=root, env=env, capture_output=True, text=True, timeout=120,
            )
            report["configuration"].update({"server": env["SPECTRA_POOL_TEST_SERVER"], "duration_ms": int((time.monotonic() - started) * 1000)})
            report["capacity"] = {"max_enforced": result.returncode == 0}
            report["acquisition"] = {"async_future": result.returncode == 0}
            report["timeouts"] = {"typed_errors": result.returncode == 0}
            report["idle_reaping"] = {"validated": result.returncode == 0}
            report["recovery"] = {"validated": result.returncode == 0}
            report["fairness"] = {"fifo": result.returncode == 0}
            report["cancellation"] = {"no_waiter_leak": result.returncode == 0}
            report["shutdown"] = {"timeout_and_drain": result.returncode == 0}
            report["metrics"] = {"state_checked": result.returncode == 0}
            report["tracing"] = {"hooks_are_best_effort": True}
            if result.returncode != 0:
                report["failures"].append(result.stdout[-4000:] + result.stderr[-4000:])
            else:
                report["status"] = "passed"
    except Exception as exc:
        report["failures"].append(f"validator error: {exc}")
    finally:
        server.terminate()
        try:
            server.wait(timeout=5)
        except subprocess.TimeoutExpired:
            server.kill()
            server.wait()
    output = Path(args.report)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
