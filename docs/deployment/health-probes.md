# Health probes for deployment

Spectra API servers expose three reserved HTTP routes backed by the same
`HealthRegistry`:

- `GET /healthz`: process liveness only; returns `200` while the server loop is
  alive.
- `GET /readyz`: traffic readiness; returns `503` when a required check fails
  or times out and `200` with `degraded` when only optional checks fail.
- `GET /startupz`: startup gate; returns `503` until the application calls
  `std.api.health.startup_complete()` or the Rust equivalent.

The routes read the most recent atomic snapshot. Database, Redis, PostgreSQL
and TCP checks run in the health evaluator, never in the HTTP request thread.
Configure external dependencies as required only when the deployment owns and
monitors that dependency. An unavailable optional service must remain
`degraded`, not be reported as healthy.

## Kubernetes

Use the manifest in `examples/deployment/kubernetes/health-probes.yaml`.
Startup gates application initialization, readiness controls traffic during
rollout, and liveness does not depend on external services. Keep the initial
delays appropriate to the application image rather than using readiness as a
replacement for startup.

## Docker

`examples/deployment/docker/healthcheck.Dockerfile` uses the real `/healthz`
endpoint. The image must contain `curl` (or the command must be replaced by an
equivalent real HTTP client); no shell sleep or fixed-success command is a
valid health check.

## systemd

`examples/deployment/systemd/spectralang-api.service` uses `/startupz` for a
post-start verification and `/readyz` for operational inspection. The unit
does not enable `WatchdogSec` until the application is built with a real
`sd_notify` integration; setting a watchdog without notifications would cause
false restarts.
