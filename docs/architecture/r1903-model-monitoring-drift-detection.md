# R-1903 Model Monitoring and Drift Detection

## Status

Complete for the current production local-serving monitoring baseline.

## Runtime Contract

`std.serve` servers collect deterministic monitoring data during local inference:

- `server_set_model_version(server, version)` attaches model-version metadata.
- `server_monitoring_snapshot(server)` returns `spectra.serve.monitoring_snapshot.v1` JSON with request, completed, blocked, cancelled, error, batch, pending, latency, throughput, and model-version metrics.
- `server_distribution_summary(server)` returns `spectra.serve.distribution_summary.v1` JSON for observed inputs and outputs.
- `drift_check(reference, live, threshold_per_mille)` compares distribution summaries and returns `spectra.serve.drift_check.v1` JSON.
- `export_monitoring(server, path, distribution, drift, audit)` writes `spectra.serve.monitoring_export.v1` JSON for external observability systems.

The implementation builds on the local serving and guardrail runtime. Network transport and external observability backends remain separate future work; the production baseline here is a versioned artifact contract with deterministic runtime evidence.

## Validation

- Runtime unit test: `serve_host_calls_cover_monitoring_drift_and_export`
- Public language validation: `tests/validation/100_phase19_model_monitoring.spectra`
- AI reference example: `examples/ai/model_monitoring_drift_detection.spectra`
- Gate script: `scripts/validate_r1903_model_monitoring.py`
- Full suite integration: `run_tests.ps1` group `phase19-monitoring`
