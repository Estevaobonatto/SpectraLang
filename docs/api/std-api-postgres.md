# `spectra.api.db.postgres`

The PostgreSQL surface is backed by the real `spectra-db` driver and accepts a
sanitized `postgres://` connection URL. It supports prepared statements,
typed bindings, row iteration, transactions, and pool-backed connections.

The implementation is currently `in_progress` under R-2505. SQLite is not a
fallback. PostgreSQL production status requires the PostgreSQL 16 integration
lane and the independent report at `target/r2505-postgres/report.json`.

COPY and LISTEN/NOTIFY are available in the Rust driver contract while the
language stream-handle contract is still being certified. Passwords, complete
DSNs, SQL parameters, and query text are not emitted in diagnostics or tracing
payloads.
