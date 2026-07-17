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

The default is disabled. HTTP server and client instrumentation is enabled
only when a configuration is active. Incoming `traceparent` headers are
validated before becoming a server-span parent, and outgoing client requests
receive the current context.

The current implementation is tracked by R-2701 and remains `in_progress`
until bounded export, retry/timeout/shutdown, concurrency isolation, and
independent OTLP payload assertions pass the production gate.
