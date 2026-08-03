# std.api.routing

`std.api.routing` is the Phase 22 router surface for matching typed HTTP
requests before handler dispatch.

Book walkthrough: [Hello HTTP](../book/09-hello-http.md)

## Types

- `Router`: route table handle.
- `Route`: registered route handle.
- `RouteMatch`: successful match handle with path parameters.

All three are runtime handles. The midend lowers them to integer handles while
the semantic layer keeps the public types distinct.

## Registering Routes

```spectra
from std.api.http import method_get
from std.api.routing import router, get, route_add

let routes = router()
let users = get(routes, "/users")
let order = route_add(routes, method_get(), "/orders/{id:\\d+}")
```

Convenience functions:

- `get(router, path)`
- `post(router, path)`
- `put(router, path)`
- `patch(router, path)`
- `delete(router, path)`

Generic registration:

- `route_add(router, method, path)`

## Pattern Syntax

- Literal segment: `/users`
- Parameter segment: `/users/{id}`
- Wildcard tail: `/files/*path`
- Regex-constrained parameter: `/orders/{id:\d+}`

The native matcher currently supports documented regex-style constraints used
by the API platform: `\d+`, `[0-9]+`, `\w+`, `[a-zA-Z]+`, `.+`, and exact
literal constraints. Invalid patterns are rejected.

## Matching and Parameters

```spectra
let hit = route_match(routes, method_get(), "/orders/123")
let route = match_route_id(hit)
let id = match_param_int(hit, "id")
```

- `route_match(router, method, path)` returns a `RouteMatch` handle or `0`.
- `match_route_id(match)` returns the matched route id or `0`.
- `match_param(match, name)` returns a path parameter as string.
- `match_param_int(match, name)` parses a path parameter as int or returns `-1`.
- `route_id(route)` returns the numeric route handle.

## Conflict Reporting

Route registration is intentionally conservative. Literal, parameter, and
wildcard patterns that can match the same method/path are rejected with a
conflict message containing both patterns.

```spectra
let first = get(routes, "/users/{id}")
let duplicate = get(routes, "/users/me")
let message = last_conflict()
```

## Validation

R-2211 is validated by:

- `cargo test -p spectra-api routing --offline`
- `cargo test -p spectra-compiler --offline`
- `cargo test -p spectra-midend --offline`
- `spectralang compile tests/validation/135_api_router_matching.spectra`
- `spectralang run tests/validation/135_api_router_matching.spectra`
- `scripts/validate_r2211_router_matching.py`
