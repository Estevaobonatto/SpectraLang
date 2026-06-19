# 10. Middleware Chain

This chapter documents the deterministic middleware contract introduced by
`R-2301`. The chain is intentionally small at this stage: it defines the
composition rules that later Phase 23 middleware builds on.

## Deterministic Order

Request hooks run in append order. Response hooks run in reverse order.

For a chain created as:

```spectra
let base = chain_new();
let one = use_sync(base, first);
let chain = use_sync(one, second);
```

the request path is:

```text
first:request
second:request
```

and the response path is:

```text
second:response
first:response
```

The complete trace is therefore:

```text
first:request
second:request
second:response
first:response
```

## Short-Circuit

A middleware may return a response before later middleware runs. When that
happens, later request hooks are skipped, but already-run middleware still gets
its response hook in reverse order.

If `limit` short-circuits after `first`, the trace is:

```text
first:request
limit:request
limit:response
first:response
```

This is the rule used by upcoming CORS, rate limiting, authentication, error,
and timeout middleware.

## Sync And Async Middleware

The public traits are:

```spectra
pub trait Middleware {
    fn on_request(&self, request: Request) -> Request;
    fn on_response(&self, response: Response) -> Response;
}

pub trait AsyncMiddleware {
    async fn on_request(&self, request: Request) -> Request;
    async fn on_response(&self, response: Response) -> Response;
}
```

`execute_sync` runs sync-only chains. `execute_async` accepts mixed sync and
async middleware and preserves the same ordering contract.

## Runnable Regression

The checked-in regression is `tests/validation/148_api_middleware_chain.spectra`:

```powershell
.\target\debug\spectralang.exe run tests\validation\148_api_middleware_chain.spectra
```

It verifies:

- synchronous trait use;
- async trait use;
- request ordering;
- reverse response ordering;
- short-circuit behavior;
- post-response hook execution;
- trace inspection through `last_trace`, `trace_len`, `trace_event`, and
  `trace_short_circuited`.

The same regression is enforced by:

```powershell
python scripts\validate_r2301_middleware_chain.py
```
