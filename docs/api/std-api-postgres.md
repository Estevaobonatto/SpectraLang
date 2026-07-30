# `spectra.api.db.postgres`

The PostgreSQL surface is backed by the real `spectra-db` driver and accepts a
sanitized `postgres://` connection URL. It supports prepared statements,
typed bindings, row iteration, transactions/savepoints, pool-backed
connections, cancellable async tasks, COPY, and typed LISTEN/NOTIFY handles.

The implementation is currently `in_progress` under R-2505. SQLite is not a
fallback. PostgreSQL production status requires the PostgreSQL 16 integration
lane and the independent v2 report at `target/r2505-postgres/report.json`.
The report must be executed with `--require-database` and include an
independent `psql` version probe, named capability tests, 100,000-row streaming
COPY evidence, real LISTEN/NOTIFY effects, operation-scoped cancellation
through the public Task bridge, and an external OTLP collector proving that a
driver-owned PostgreSQL span is a child of an HTTP server span.

The synchronous calls remain available for compatibility and migrations.
Application code should use `execute_async`, `step_async`,
`copy_in_text_async`, `copy_out_text_async`, `notify_async`, and
`notification_next_async` with normal `await`/`block_on`. COPY is incremental
inside the Rust driver; the text helpers are the convenient language boundary.
`copy_out_text_async` is capped at 16 MiB (`DB2505_COPY_LIMIT`) to keep that
whole-string boundary memory-safe; larger exports must use the Rust driver's
incremental `copy_out_to` sink.
Prepared handles retain and execute the server-prepared statement. Long-lived
notification waits use an isolated cancellable I/O executor so they cannot
consume the general database worker pool.

Passwords, complete DSNs, SQL parameters, query text, and COPY/notification
payloads are not emitted in diagnostics or tracing payloads.
