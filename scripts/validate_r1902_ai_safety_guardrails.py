#!/usr/bin/env python3
"""Validate R-1902 AI safety guardrails and serving audit gates."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def run_step(name: str, args: list[str]) -> None:
    print(f"[R-1902] {name}: {' '.join(args)}")
    completed = subprocess.run(args, cwd=ROOT, text=True)
    if completed.returncode != 0:
        raise SystemExit(completed.returncode)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def validate_audit(path: Path) -> None:
    data = json.loads(path.read_text(encoding="utf-8"))
    require(data["schema"] == "spectra.serve.audit.v1", "bad guardrail audit schema")
    events = data["events"]
    require(any(event["event"] == "policy_attached" for event in events), "policy attachment audit missing")
    require(any(event["event"] == "blocked" and event["stage"] == "input" for event in events), "blocked input audit missing")
    require(any(event["event"] == "blocked" and event["stage"] == "output" for event in events), "blocked output audit missing")
    require(any(event["result"] == -404 for event in events), "fallback result audit missing")


def main() -> int:
    (ROOT / "target/ai-examples/safety").mkdir(parents=True, exist_ok=True)
    run_step(
        "runtime guardrails",
        [
            "cargo",
            "test",
            "-p",
            "spectra-runtime",
            "serve_host_calls_cover_guardrails_rate_limit_fallback_and_audit",
        ],
    )
    run_step(
        "public Spectra validation",
        [
            "cargo",
            "run",
            "-p",
            "spectra-cli",
            "--",
            "run",
            "tests/validation/99_phase19_ai_safety_guardrails.spectra",
        ],
    )
    run_step(
        "AI safety example",
        [
            "cargo",
            "run",
            "-p",
            "spectra-cli",
            "--",
            "run",
            "examples/ai/safe_serving_guardrails.spectra",
        ],
    )
    validate_audit(ROOT / "target/ai-examples/safety/guardrail-audit.json")
    summary = ROOT / "target/ai-examples/safety/guardrail-summary.txt"
    require(summary.exists(), "guardrail summary missing")
    require("safe_serving_guardrails=pass" in summary.read_text(encoding="utf-8"), "guardrail summary failed")
    print("[R-1902] validation passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
