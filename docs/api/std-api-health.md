# Health checks

The API server reserves `GET /healthz`, `GET /readyz` and `GET /startupz`.
Responses are JSON and use the latest snapshot produced by the runtime health
evaluator; a check is never executed on the HTTP request thread.

Rust applications can install a `spectra_runtime::health::HealthRegistry` with
`HttpServer::with_health_registry`. Checks have a category, timeout and required
flag. Required failures make readiness return `503`; optional failures produce
`degraded` readiness. Liveness is independent of external services and startup
is `503` until `set_startup_complete()` is called. `startup_failed` keeps it
unavailable.

The Spectra compatibility surface currently exposes only:

- `std.api.health.startup_complete()`;
- `std.api.health.startup_failed(reason)`.

The reserved routes are the production HTTP contract. Redis and PostgreSQL
health adapters are only enabled when their real services are configured and
validated; an unavailable optional environment is reported as skipped, never
as a successful dependency check.
