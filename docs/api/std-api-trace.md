# `std.api.trace`

`std.api.trace` exposes opt-in tracing handles for API and external-operation
instrumentation. The runtime emits W3C Trace Context and exports OTLP/HTTP
protobuf when a configuration is started.

## Lifecycle

```spectra
let config = config_new("http://127.0.0.1:4318/v1/traces", "my-service")
config_set_sample_rate(config, 1.0)
config_set_batch_size(config, 256)
config_start(config)

let span = span_start("operation", 1)
span_set_attribute(span, "component", "example")
span_set_attribute_int(span, "http.response.status_code", 200)
span_set_attribute_bool(span, "fixture.sampled", true)
span_set_status(span, 1)
span_end(span)
flush()
config_shutdown(config)
```

Span kinds are `1` internal, `2` server, `3` client, `4` producer, and `5`
consumer. Status values are `0` unset, `1` ok, and `2` error. Invalid
configuration, traceparent, handles, and closed spans are reported through the
boolean result and the runtime error code; callers must not treat a failed
operation as an exported span.

Attributes preserve their OTLP type. Use `span_set_attribute` for strings,
`span_set_attribute_int` for signed integers, and
`span_set_attribute_bool` for booleans. Reusing a key replaces its previous
value deterministically.

The default is disabled. HTTP server and client instrumentation is enabled
only when a configuration is active. Incoming `traceparent` headers are
validated before becoming a server-span parent, and outgoing client requests
receive the current context.

R-2701 is complete for the currently supported boundaries. The production
gate independently validates bounded export, retry/timeout/shutdown,
concurrency isolation, HTTP client propagation, filesystem spans, SQLite
query spans, and OTLP payload structure. PostgreSQL and Redis are not claimed
until their drivers exist.
