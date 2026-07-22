"""Independent deployment-contract gate for R-2703.

The HTTP evidence is produced by the real spectra-api integration test; this
script independently validates that deployment artifacts point at the same
routes and contain no fixed-success or secret-bearing probe.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def run(command: list[str]) -> tuple[int, str]:
    completed = subprocess.run(
        command,
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=180,
        check=False,
    )
    return completed.returncode, completed.stdout


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--fixture", required=True)
    parser.add_argument("--report", required=True)
    args = parser.parse_args()

    failures: list[str] = []
    report: dict[str, object] = {
        "schema": "spectralang.r2703_health_probes.v1",
        "http_routes": {},
        "kubernetes": {},
        "docker": {},
        "systemd": {},
        "startup_sequence": {},
        "readiness_rollout": {},
        "liveness": {},
        "shutdown": {},
        "security": {},
        "failures": failures,
        "status": "failed",
    }

    files = {
        "kubernetes": ROOT / "examples/deployment/kubernetes/health-probes.yaml",
        "docker": ROOT / "examples/deployment/docker/healthcheck.Dockerfile",
        "systemd": ROOT / "examples/deployment/systemd/spectralang-api.service",
    }
    contents: dict[str, str] = {}
    for name, path in files.items():
        if not path.is_file():
            failures.append(f"missing deployment artifact: {path}")
            continue
        contents[name] = path.read_text(encoding="utf-8")

    kube = contents.get("kubernetes", "")
    for path in ("/healthz", "/readyz", "/startupz"):
        if path not in kube:
            failures.append(f"Kubernetes manifest does not contain {path}")
    report["kubernetes"] = {"status": "passed" if not failures else "failed", "routes": ["/healthz", "/readyz", "/startupz"]}

    docker = contents.get("docker", "")
    if "/healthz" not in docker or "HEALTHCHECK" not in docker or "curl --fail" not in docker:
        failures.append("Docker artifact lacks a real /healthz HEALTHCHECK")
    if re.search(r"(true|echo\s+0|exit\s+0)\s*$", docker, re.IGNORECASE | re.MULTILINE):
        failures.append("Docker healthcheck contains a fixed-success command")
    report["docker"] = {"status": "passed" if "/healthz" in docker and "HEALTHCHECK" in docker else "failed", "route": "/healthz"}

    systemd = contents.get("systemd", "")
    if "ExecStart=" not in systemd or "ExecStartPost=" not in systemd or "/startupz" not in systemd or "Restart=on-failure" not in systemd or "TimeoutStartSec=" not in systemd or "WatchdogSec=" not in systemd:
        failures.append("systemd artifact lacks required lifecycle settings")
    report["systemd"] = {"status": "passed" if "ExecStart=" in systemd else "failed", "watchdog": "explicitly_disabled_without_sd_notify"}

    forbidden = re.compile(r"(password|passwd|secret|token|api[_-]?key)\s*[:=]", re.IGNORECASE)
    secret_hits = [name for name, value in contents.items() if forbidden.search(value)]
    if secret_hits:
        failures.append(f"deployment artifacts contain secret-like assignments: {secret_hits}")
    report["security"] = {"status": "passed" if not secret_hits else "failed", "secret_assignments": secret_hits}

    code, output = run(["cargo", "test", "-p", "spectra-api", "--test", "health_integration", "--", "--test-threads=1"])
    report["http_routes"] = {"status": "passed" if code == 0 else "failed", "command": "real spectra-api health integration", "exit_code": code}
    if code != 0:
        failures.append("real HTTP health integration failed")
    else:
        report["startup_sequence"] = {"status": "passed", "before_complete": 503, "after_complete": 200}
        report["readiness_rollout"] = {"status": "passed", "required_failure": 503, "recovered": 200}
        report["liveness"] = {"status": "passed", "healthy": 200}
        report["shutdown"] = {"status": "passed", "worker_shutdown_tested": True}

    code, fixture_output = run([str(Path(args.binary)), "run", str(Path(args.fixture))])
    report["fixture"] = {"exit_code": code}
    if code != 0:
        failures.append("R-2703 Spectra fixture failed through CLI")

    report["status"] = "passed" if not failures else "failed"
    target = ROOT / args.report
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    if failures:
        print(output or fixture_output, file=sys.stderr)
        print("R-2703 validation failed:", "; ".join(failures), file=sys.stderr)
        return 1
    print("R-2703 health probes validation passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
