# std.api.middleware

`std.api.middleware` defines the Phase 23 middleware chain used by the API
platform before feature-specific middleware such as CORS, rate limiting,
authentication, logging, compression, and security headers.

## Ordering Contract

Middleware is appended to a `MiddlewareChain` in registration order.

Request hooks run in append order:

```text
first.on_request -> second.on_request -> terminal handler
```

Response hooks run in reverse order:

```text
terminal response -> second.on_response -> first.on_response
```

If a middleware short-circuits the request, later request hooks are not called.
The response still unwinds through response hooks for middleware that already
ran. For example, if `second` short-circuits, the trace is:

```text
first:request
second:request
second:response
first:response
```

This deterministic contract is the dependency for Phase 23 middleware items
`R-2302` through `R-2316`.

## Traits

Synchronous middleware implements:

```spectra
public trait Middleware {
    func on_request(&self, request: Request) returns Request
    func on_response(&self, response: Response) returns Response
}
```

Async middleware implements:

```spectra
public trait AsyncMiddleware {
    async func on_request(&self, request: Request) returns Request
    async func on_response(&self, response: Response) returns Response
}
```

The native runtime stores middleware as chain entries and supports both sync
and async execution. `execute_sync` rejects async middleware; `execute_async`
can run sync and async middleware in the same chain.

## Public Functions

- `chain()` / `chain_new()` create an empty `MiddlewareChain`.
- `chain_len(chain)` returns the number of entries.
- `register_sync(before, after)` registers testable sync middleware hooks.
- `register_sync_short_circuit(before, after, response)` registers sync
  middleware that returns `response` instead of calling later middleware.
- `register_async(before, after)` registers testable async middleware hooks.
- `register_async_short_circuit(before, after, response)` registers async
  middleware that short-circuits.
- `use_sync(chain, middleware)` appends a sync middleware and returns the new
  chain handle.
- `use_async(chain, middleware)` appends an async middleware and returns the new
  chain handle.
- `execute_sync(chain, request, terminal_response)` runs a sync-only chain.
- `execute_async(chain, request, terminal_response)` runs a mixed sync/async
  chain.
- `last_trace()` returns the trace from the most recent chain execution.
- `trace_len(trace)`, `trace_event(trace, index)`, and
  `trace_short_circuited(trace)` expose deterministic validation data.

## Validation

The executable regression is `tests/validation/148_api_middleware_chain.spectra`:

```powershell
.\target\debug\spectralang.exe run tests\validation\148_api_middleware_chain.spectra
```

The roadmap validator is:

```powershell
python scripts\validate_r2301_middleware_chain.py
```
