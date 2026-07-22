# `spectra.api.db.sqlite`

The SQLite surface is backed by the bundled SQLite engine and operates on
file-backed databases. It is not an in-memory mock and does not perform SQL
string interpolation.

Parameter and column indexes are zero-based. `step` returns `1` for a row and
`2` when the statement is complete. Invalid handles and state transitions
return `false`/`0`; the stable error code and message are available through
`last_error_code` and `last_error_message`.

Prepared statements support null, integer, floating-point, text and blob
bindings. A statement must be reset before it can be bound again after a
`step`, and `finalize` permanently invalidates the statement.

Transactions are explicit through `begin`, `commit` and `rollback`. Dropping
the native transaction object rolls back an active transaction. The driver
uses the shared `spectra-db` connection pool for asynchronous consumers.

`execute_async(connection, sql)` runs the blocking SQLite operation on a
dedicated worker and returns a handle consumed by the existing
`std.async.task.poll` and `std.async.task.result` protocol. It does not run
SQLite work on the reactor thread.

R-2504 is complete. The independent v2 gate validates file-backed CRUD,
prepared statements, transactions, pool consumption, cancellation, a real
SQLite lock wait off the reactor thread, and SQLite spans parented by a real
HTTP server span and decoded from OTLP protobuf. PostgreSQL, Redis, query
builder, and migrations remain separate roadmap items.
