# ADR 0011: API Library Architecture

Status: Accepted

Date: 2026-06-17

Roadmap item: R-2201

## Context

SpectraLang now has the Phase 21 async foundation required for native API
development: `Task<T>`, `Stream<T>`, structured concurrency, cancellation,
async diagnostics, formal `Send`/`Sync` evidence, and a runtime reactor
boundary.

Phase 22 starts the API platform workstream. The project needs a stable
architecture before implementing HTTP parsing, servers, clients, TLS, JSON,
routing, package publishing, documentation, and conformance tests. Without an
ADR, those pieces can drift into unrelated places such as ad-hoc `std`
modules, runtime-only host calls, or examples that cannot be published as a
versioned package.

The API platform must serve two goals at the same time:

- a native Spectra package named `spectra.api` that application authors import
  and depend on through the Phase 9 package manager;
- a Rust implementation boundary that owns protocol parsing, server/client
  state, TLS, JSON plumbing, routing support, and host-call registration.

## Decision

Spectra accepts `spectra.api` as a separately versioned API platform package
backed by a Rust workspace crate named `spectra-api`.

The accepted model is:

- public Spectra package name: `spectra.api`;
- public import path: `std.api.*`;
- Rust crate name: `spectra-api`;
- crate path: `packages/spectra-api`;
- Spectra package manifest: `packages/spectra-api/spectra.toml`;
- Spectra package bindings: `packages/spectra-api/src/*.spectra`;
- Rust implementation modules: `packages/spectra-api/src/`;
- runtime integration point: `runtime/src/api/` for thin host-call plumbing
  that delegates to `spectra-api`;
- documentation root: `docs/api/`;
- runnable examples root: `examples/api/`;
- host-call prefix: `spectra.api.*`.

`spectra.api` is not part of the core language and is not implemented as a
monolithic extension of `std`. The compiler exposes `std.api.*` as the stable
import surface because existing stdlib resolution already treats `std.*`
modules as virtual public modules, but the ownership, release cadence, and
runtime implementation belong to the package.

## Repository Layout

Phase 22 uses this layout:

```text
packages/
  spectra-api/
    Cargo.toml
    spectra.toml
    src/
      lib.rs
      http/
      server/
      client/
      json/
      tls/
      routing/
      bindings/
        mod.spectra
        http.spectra
        server.spectra
        client.spectra
        json.spectra
        tls.spectra
        routing.spectra
runtime/
  src/
    api/
      mod.rs
      host_calls.rs
docs/
  api/
examples/
  api/
```

The Rust crate is a workspace member and links against `spectra-runtime`. It
owns protocol state machines and typed API resources. `runtime/src/api/` is a
small integration layer that registers host calls and keeps the existing FFI
registry boundary stable.

The Spectra package under the same root is the artifact published to the local
registry. Its `.spectra` binding files expose the package API while delegating
implementation to `spectra.api.*` host calls.

## Public API Surface

The canonical public surface is `std.api.*`. The Phase 22 surface is fixed as
these module families:

- `std.api.http`
- `std.api.server`
- `std.api.client`
- `std.api.json`
- `std.api.tls`
- `std.api.routing`
- `std.api.errors`

The canonical public types are:

- `Request`
- `Response`
- `Method`
- `Status`
- `Header`
- `Headers`
- `Cookie`
- `Body`
- `Route`
- `Router`
- `Server`
- `Client`
- `TlsConfig`
- `ApiError`

The first stable function families are:

- `std.api.http.request(...)`
- `std.api.http.response(...)`
- `std.api.http.header(...)`
- `std.api.http.status(...)`
- `std.api.server.serve(...)`
- `std.api.server.shutdown(...)`
- `std.api.client.request(...)`
- `std.api.json.encode(...)`
- `std.api.json.decode(...)`
- `std.api.routing.router(...)`
- `std.api.routing.get(...)`
- `std.api.routing.post(...)`
- `std.api.routing.put(...)`
- `std.api.routing.patch(...)`
- `std.api.routing.delete(...)`
- `std.api.tls.server_config(...)`
- `std.api.tls.client_config(...)`

Every public async operation returns `Task<T>`. Streaming request or response
bodies use `Stream<bytes>` once byte buffers are available in the language
surface; until then, Phase 22 represents bodies as runtime-managed `Body`
handles.

## Host-Call Boundary

Host calls use the prefix `spectra.api.*`.

The first host-call families are:

- `spectra.api.http.*`
- `spectra.api.server.*`
- `spectra.api.client.*`
- `spectra.api.json.*`
- `spectra.api.tls.*`
- `spectra.api.routing.*`
- `spectra.api.errors.*`

The host-call boundary follows existing runtime conventions:

- all handles are opaque integer runtime handles at the ABI layer;
- public Spectra types hide handle representation;
- functions return structured status or typed `ApiError` data rather than
  panicking;
- async host calls integrate with the Phase 21 reactor and cancellation model;
- host-call names are stable and covered by a focused validation script before
  a roadmap item can be marked complete.

## HTTP Version Strategy

Phase 22 implements HTTP/1.1 only.

The supported roadmap is:

- Phase 22: HTTP/1.1 parser, server, client, JSON, TLS, basic routing, and v0
  conformance;
- Phase 24: HTTP/2 server/client using ALPN and HPACK;
- Phase 24 or later: HTTP/3 over QUIC only when the Rust QUIC dependency is
  stable enough for the Spectra release channel;
- Phase 28: v1 API conformance and release certification.

The public API may include protocol-version configuration, but unsupported
versions must return a typed `ApiError` until their roadmap item is complete.

## TLS Model

TLS uses `rustls`.

Accepted rules:

- HTTPS is the secure default for production server and client configuration.
- Plain HTTP is allowed only through explicit configuration such as
  `allow_plaintext`.
- Certificates and trust roots are explicit runtime resources.
- Server Name Indication is required for HTTPS clients.
- ALPN is configured through the TLS layer so HTTP/2 can be added without
  redesigning the API.
- OpenSSL is rejected for the default implementation because it adds platform
  variance and native dependency management that conflicts with the current
  portability goals.

## Async Dependencies

`spectra.api` uses the Spectra Phase 21 async model. It does not expose or
require a second async runtime.

Accepted dependencies:

- `spectra-runtime` for host-call registration, task handles, cancellation,
  reactor integration, and runtime resource registries;
- `mio` through the runtime reactor boundary where OS readiness is required;
- `rustls` for TLS;
- parser/JSON helper crates may be used inside `spectra-api` when they do not
  leak into the public Spectra API.

Rejected dependency shape:

- no public Tokio runtime dependency;
- no callback-first public API;
- no runtime-specific types in public `.spectra` bindings.

## Ownership

Phase 22 API work is owned by `web` unless the item is clearly ecosystem
documentation or release work.

Ownership decisions:

- `R-2201`: `ecosystem`, because it is an ADR and planning gate;
- `R-2202`: `web`, because the crate owns public HTTP/API host-call surface;
- HTTP parser/server/client/TLS/JSON/routing items: `web`;
- package publish, book, examples, and conformance release gates:
  `ecosystem`;
- future database drivers: `db`.

`runtime` remains responsible for generic runtime primitives. API-specific
HTTP, routing, TLS, and package surfaces should not be assigned to `runtime`
solely because they register host calls.

## Migration Path

There is no production HTTP surface in the completed Phase 11 serving baseline.
`std.serve` is a local in-process model-serving harness, not a network API
server.

Migration rules:

- `std.serve` remains for local AI serving experiments and does not become the
  HTTP server.
- New network API code must use `std.api.*`.
- Any future ad-hoc `std.http`, `std.web`, or socket examples must migrate to
  `spectra.api`.
- The final migration guide is tracked by `R-2807`.
- Until `spectra.api` v1.0, migration notes live in `docs/api/` and the
  `Hello HTTP` book chapter.

## Consequences

- `R-2202` must create the `packages/spectra-api` workspace crate and package
  root according to this ADR.
- `R-2203` must expose `std.api.*` based on the package binding surface, not as
  unrelated compiler-only fake modules.
- `R-2204` through `R-2216` must use the `spectra.api.*` host-call prefix.
- `R-2217` publishes the package to the local Phase 9 registry.
- `R-2220` must gate HTTP/1.1 v0 conformance before Phase 22 is treated as
  complete.
- `R-2801` remains the final v1 conformance gate for `spectra.api`.

## Rejected Alternatives

### Put HTTP Directly Into `std`

Rejected. It would couple API evolution to the core language/runtime release
cadence and make optional dependencies such as TLS, routing, OpenAPI, and
database drivers part of the base runtime.

### Implement Only Runtime Host Calls Without a Spectra Package

Rejected. Host calls alone do not provide a documented, versioned Spectra API
surface and cannot be consumed through the package manager.

### Use OpenSSL as the Default TLS Backend

Rejected. OpenSSL introduces native dependency and platform variance that is
not appropriate for the default Spectra API package. `rustls` is the accepted
default.

### Expose Tokio Types Publicly

Rejected. Spectra already has a Phase 21 async model. Public API signatures
must use `Task<T>` and `Stream<T>`, not Rust runtime-specific types.

### Start With HTTP/2 or HTTP/3

Rejected for Phase 22. HTTP/1.1 is the foundation for parser correctness,
headers, body limits, keep-alive, TLS, and conformance. HTTP/2 and HTTP/3 are
layered roadmap items.

## Acceptance Evidence

- This ADR fixes the crate layout, package name, import path, public API
  surface, host-call prefix, HTTP version plan, TLS model, async dependencies,
  and migration path.
- `scripts/validate_r2201_api_adr.py` validates this ADR and roadmap/backlog
  synchronization.
- `run_tests.ps1` includes the `validate_r2201_api_adr` gate.
