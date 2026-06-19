# spectra.api Conformance v0

The v0 conformance suite is the Phase 22 HTTP/1.1 release gate for the
implemented `spectra.api` baseline. It covers the protocol, JSON, and router
behavior that every local implementation must preserve before later middleware,
database, tooling, and observability phases build on top of it.

The canonical runner is:

```powershell
python scripts\validate_r2220_api_conformance_v0.py
```

The validator runs the Rust conformance suite through
`packages/spectra-api/examples/conformance_v0.rs` and writes a machine-readable
report to `target/api-conformance-v0.json`.

## HTTP/1.1 Must-Pass Cases

- `http1.request.get_minimal`: parse a minimal HTTP/1.1 GET request with Host.
- `http1.request.content_length_body`: parse POST Content-Length bodies exactly.
- `http1.request.pipelined_streaming`: stream and retain pipelined request bytes.
- `http1.request.chunked_round_trip`: round-trip chunked requests with extensions
  and trailers.
- `http1.response.rfc7230_sample`: parse a representative RFC 7230 response.
- `http1.response.chunked_round_trip`: round-trip chunked HTTP/1.1 responses.
- `http1.connection.http10_keep_alive`: honor HTTP/1.0 keep-alive only when
  requested.
- `http1.error.malformed_header_position`: report malformed headers with typed
  byte positions.
- `http1.error.invalid_chunk_size`: report invalid chunk sizes as typed parser
  errors.
- `http1.error.unsupported_transfer_encoding`: reject unsupported transfer
  encodings.
- `http1.error.conflicting_content_length`: reject conflicting Content-Length
  headers.
- `http1.types.method_status_matrix`: validate documented Method and Status
  semantics.
- `http1.types.header_validation`: validate header name and value rules.

## JSON Must-Pass Cases

- `json.kind.matrix`: classify null, bool, number, string, array, and object.
- `json.round_trip.nested_object`: round-trip nested objects, arrays, strings,
  numbers, bools, and null.
- `json.escape.unicode`: decode common escapes and unicode sequences.
- `json.error.invalid_syntax_offset`: report invalid syntax with line, column,
  and byte offset.
- `json.encode.non_finite_rejected`: reject NaN and infinity before JSON
  encoding.
- `json.number.exponent_round_trip`: preserve supported exponent number
  representations.

## Router Must-Pass Cases

- `routing.literal.match`: match literal routes.
- `routing.param.extract`: extract named path parameters.
- `routing.wildcard.extract`: extract wildcard path tails.
- `routing.regex.constraint`: apply numeric route constraints.
- `routing.method.separation`: keep routes with the same path but different
  methods separate.
- `routing.conflict.overlap`: reject overlapping literal and parameter routes.
- `routing.invalid.path`: reject invalid route match paths.

The report is versioned with suite id `spectra.api.conformance.v0` and includes
one entry per case with `id`, `category`, `description`, `passed`, and `detail`.
