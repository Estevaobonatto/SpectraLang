# R-1902 AI Safety and Guardrail Runtime

## Status

Complete for the current production local-serving safety baseline.

## Runtime Contract

`std.serve` servers can attach deterministic guardrails:

- `server_set_input_policy(server, min, max)` blocks out-of-range inputs before queueing.
- `server_set_output_policy(server, min, max)` blocks out-of-range outputs before result publication.
- `server_set_rate_limit(server, limit)` blocks requests after the accepted-request limit.
- `server_set_fallback(server, value)` defines the degraded safe result for blocked requests.
- `server_last_diagnostic(server)` returns `spectra.serve.guardrail_diagnostic.v1` JSON for the latest guardrail failure.
- `server_audit_log(server)` returns `spectra.serve.audit.v1` JSON with policy attachment, accepted, completed, and blocked events.

Blocked requests complete with the fallback result rather than surfacing an internal runtime failure. This keeps serving callers on the normal request/result path while still exposing machine-readable diagnostics and audit evidence.

## Validation

- Runtime unit test: `serve_host_calls_cover_guardrails_rate_limit_fallback_and_audit`
- Public language validation: `tests/validation/99_phase19_ai_safety_guardrails.spectra`
- AI reference example: `examples/ai/safe_serving_guardrails.spectra`
- Gate script: `scripts/validate_r1902_ai_safety_guardrails.py`
- Full suite integration: `run_tests.ps1` group `phase19-safety`
