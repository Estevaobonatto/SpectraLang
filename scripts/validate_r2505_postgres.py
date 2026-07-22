"""Independent R-2505 gate.

The gate never substitutes SQLite or an in-process protocol implementation for
PostgreSQL. Without SPECTRA_POSTGRES_URL it records skipped_environment; the
required CI lane supplies a real PostgreSQL 16 service.
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def run(command: list[str], env: dict[str, str] | None = None) -> dict:
    completed = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True, timeout=180)
    return {
        "command": command,
        "exit_code": completed.returncode,
        "stdout_tail": completed.stdout[-2000:],
        "stderr_tail": completed.stderr[-2000:],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", default="target/debug/spectralang.exe")
    parser.add_argument("--database-url", default=None)
    parser.add_argument("--fixture", default="tests/validation/195_postgres_driver.spectra")
    parser.add_argument("--report", required=True)
    args = parser.parse_args()

    url = args.database_url or os.environ.get("SPECTRA_POSTGRES_URL")
    report = {
        "schema": "spectralang.r2505_postgres.v1",
        "environment": {"postgresql_required": True, "server_version": "16", "configured": bool(url)},
        "connection": {}, "pool": {}, "prepared_statements": {}, "crud": {},
        "transactions": {}, "savepoints": {}, "copy_in": {}, "copy_out": {},
        "listen_notify": {}, "async_non_blocking": {}, "cancellation": {},
        "query_builder": {}, "tracing": {}, "http_parent": {}, "security": {},
        "diagnostics": {}, "failures": [], "status": "skipped_environment" if not url else "failed",
    }

    if not url:
        report["environment"]["reason"] = "SPECTRA_POSTGRES_URL is not configured; required CI PostgreSQL lane must run this gate."
    else:
        env = os.environ.copy()
        env["SPECTRA_POSTGRES_URL"] = url
        test = run(["cargo", "test", "-p", "spectra-db", "--test", "postgres_integration", "--", "--test-threads=1"], env)
        report["connection"]["rust_integration"] = test
        if test["exit_code"] != 0:
            report["failures"].append("postgres integration tests failed")
        fixture = Path(args.fixture)
        with tempfile.TemporaryDirectory(prefix="spectra-r2505-") as temporary:
            generated = Path(temporary) / fixture.name
            generated.write_text(fixture.read_text(encoding="utf-8").replace("__SPECTRA_POSTGRES_URL__", url), encoding="utf-8")
            cli = run([str(ROOT / args.binary), "run", str(generated)], env)
            report["connection"]["cli_fixture"] = cli
            if cli["exit_code"] != 0:
                report["failures"].append("PostgreSQL Spectra fixture failed")
        report["security"]["secret_not_in_report"] = url not in json.dumps(report)
        report["status"] = "passed" if not report["failures"] else "failed"

    output = Path(args.report)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if report["status"] in {"passed", "skipped_environment"} else 1


if __name__ == "__main__":
    raise SystemExit(main())
