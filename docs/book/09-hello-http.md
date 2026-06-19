# 9. Hello HTTP

This chapter is the first runnable path through `spectra.api`. It starts a
local HTTP server, registers one route, returns a typed response, and shuts the
server down through the public lifecycle API.

The package is delivered through the local registry as `spectra.api`. In user
projects, install it with:

```powershell
.\target\debug\spectralang.exe package add spectra-api --root . --registry .spectra-registry --version 0.1.0
```

The compiler exposes the public API through `std.api.*` modules. The example
below uses four of them:

- `std.api.routing` creates the route table and the `/hello` route.
- `std.api.handler` creates a `Response`, attaches a header, and registers it
  as the route handler.
- `std.api.http` keeps the request and response typed.
- `std.api.server` configures a local listener, starts the server, reports the
  assigned port, and shuts down gracefully.

## Runnable Example

The checked-in example is `examples/api/00_hello_http.spectra`:

```spectra
module hello_http;

import { Server, new, listen, serve, shutdown, local_port, state } from std.api.server;
import { Router, Route, router, get, route_id } from std.api.routing;
import { HandlerHandle, text, with_header, register_sync, dispatch_sync } from std.api.handler;
import {
    Request,
    Response,
    method_get,
    request,
    response_status,
    response_header,
    response_body_len,
} from std.api.http;

pub fn main() -> int {
    let routes: Router = router();
    let route: Route = get(routes, "/hello");
    let response: Response = with_header(text("Hello HTTP from Spectra"), "Content-Type", "text/plain");
    let handler: HandlerHandle = register_sync(route_id(route), response);
    let request_value: Request = request(method_get(), "/hello");
    let dispatched: Response = dispatch_sync(handler, request_value);

    if response_status(dispatched) != 200 {
        return 5;
    }
    if response_header(response, "content-type") != "text/plain" {
        return 3;
    }
    if response_body_len(response) <= 0 {
        return 4;
    }

    let server: Server = new();
    if listen(server, 0) != true {
        return 6;
    }
    if block_on(serve(server, routes)) != 1 {
        return 7;
    }
    if local_port(server) <= 0 {
        return 9;
    }
    if shutdown(server) != true {
        return 10;
    }
    if state(server) != 3 {
        return 11;
    }
    return 0;
}
```

Run it from the repository root:

```powershell
.\target\debug\spectralang.exe run examples\api\00_hello_http.spectra
```

The program exits with status `0` when the route, typed response, handler
dispatch, local listener, assigned port, and graceful shutdown all work.

## What This Proves

This is not a long-running production server yet. That belongs to later API
tooling and operations phases. The purpose here is narrower and testable:

- route definition works through the public router surface;
- a typed `Response` can be returned by a registered handler;
- handler dispatch can be checked locally without external dependencies;
- `serve` starts the server on a local assigned port;
- shutdown leaves the server in the stopped state.

The same example is validated by `scripts/validate_r2218_hello_http_book.py`
and by the full `run_tests.ps1` suite.
