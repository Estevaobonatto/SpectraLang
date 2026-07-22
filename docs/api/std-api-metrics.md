# Prometheus metrics

`HttpServer` exposes `GET /metrics` in the Prometheus text exposition format.
The route is reserved and is not dispatched to application handlers.

```rust
let registry = MetricsRegistry::new();
registry.register_counter("app_jobs_total", "Completed jobs", &["queue"])?;
registry.counter_inc("app_jobs_total", &["default"], 1)?;
```

The server records these metrics automatically:

- `spectra_http_requests_total{method,status}`;
- `spectra_http_request_duration_seconds{method}`;
- `spectra_http_errors_total{class}`;
- `spectra_http_active_connections`;
- `spectra_http_accepted_connections_total`;
- `spectra_http_timeouts_total`.

Metric names and label names follow Prometheus rules. Labels are fixed at
registration time and are bounded in count, cardinality, and value length.
URL, query, body, token, user, credential, and other sensitive values must not
be used as labels. NaN and infinity are rejected. Rendering is deterministic
and includes `HELP`, `TYPE`, and histogram `_bucket`, `_sum`, and `_count`
series.

The registry is a Rust/API infrastructure contract in this version. No
incomplete Spectra host calls are exposed. Validate the complete contract with
`scripts/validate_r2702_metrics.py`; its report is written to
`target/r2702-metrics/report.json`.
