# std.api.handler

`std.api.handler` defines the Phase 22 handler contract used by API routes to
produce `std.api.http.Response` values.

## Traits

- `IntoResponse`: converts a value into `Response`.
- `Handler`: synchronous handler contract with `call(request) -> Response`.
- `AsyncHandler`: asynchronous handler contract with
  `async call(request) -> Response`.

The native `spectra-api` crate also provides Rust implementations of
`IntoResponse` for `Response`, `String`, `&str`, `Vec<u8>`, `()`,
`HandlerError`, and `Result<T, HandlerError>` when `T: IntoResponse`.

## Types

- `HandlerHandle`: runtime handle for a registered synchronous handler.
- `AsyncHandlerHandle`: runtime handle for a registered async handler.
- `HandlerError`: typed handler failure with HTTP status and message.

## Response Helpers

The module exposes stable helpers that normalize common handler return
values into `Response`:

```spectra
let ok = handler.text("created");
let body = handler.json("{\"ok\":true}");
let empty = handler.status(204);
let with_id = handler.with_header(ok, "X-Request-Id", "abc");
```

`into_text_response` and `into_status_response` are the public Spectra bridge
for custom `IntoResponse` implementations until package-level native extern
declarations and blanket impls are first-class in Spectra source.

## Dispatch

`register_sync(route_id, response)` and `register_async(route_id, response)`
produce handler handles. `dispatch_sync` and `dispatch_async` return the
normalized `Response` for a request handle. Phase 22 keeps dispatch
deterministic and handle-based; server lifecycle integration is owned by
R-2216.

## Errors

`error(status, message)` creates a `HandlerError`. `error_response(error)`
converts it into a response, and `last_error_message()` exposes the latest
handler failure for integration with the unified error middleware workstream.

## Validation

R-2215 is covered by:

- `packages/spectra-api/src/handler.rs` unit tests.
- `tests/validation/139_api_handler_response_return.spectra`.
- `scripts/validate_r2215_handler_response.py`.
- the full `run_tests.ps1` suite.
