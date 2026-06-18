# std.api.query

`std.api.query` is the Phase 22 query-string parser and typed binding
surface for Spectra API programs.

## Types

- `Query`: parsed query handle. It stores decoded keys and values while
  preserving repeated keys as ordered arrays.
- `QuerySchema`: typed binding schema. Fields declare a name, value type,
  whether the field is required, and whether repeated values are accepted.
- `QueryBinding`: result of binding a `Query` to a `QuerySchema`.

## Parsing

`parse(input)` accepts either a raw query string such as `page=2&tag=api`,
a leading-question-mark string such as `?page=2`, or a request target such
as `/search?page=2#section`.

The parser applies RFC 3986 percent decoding:

- `%HH` byte escapes are decoded and then validated as UTF-8.
- malformed percent escapes are rejected and reported through
  `error_code()` and `error_message()`.
- raw control characters are rejected.
- `+` remains a literal plus sign. The `+` to space rule belongs to
  `application/x-www-form-urlencoded` and is handled by the form-binding
  phase, not by query parsing.

## Repeated Keys

Repeated keys are arrays:

```spectra
let query = parse("/search?tag=rust&tag=api");
let total = count(query, "tag");      // 2
let first_tag = value(query, "tag", 0);
let second_tag = value(query, "tag", 1);
```

`first(query, key)` is a convenience for index `0`. Missing keys return an
empty string for string accessors and `0` for numeric/boolean host-call
accessors while recording a typed error where applicable.

## Typed Binding

Bindings are schema-driven until package-level native extern declarations
and reflection over user structs are available:

```spectra
let raw = parse("/search?page=2&published=true&tag=rust&tag=api");
let s0 = schema();
let s1 = schema_field(s0, "page", type_int(), true, false);
let s2 = schema_field(s1, "published", type_bool(), true, false);
let s3 = schema_field(s2, "tag", type_string(), false, true);
let binding = bind(raw, s3);
```

`binding_ok(binding)` reports whether all fields passed the schema. Typed
errors include missing required fields, repeated scalar fields, invalid
integers, invalid booleans, and invalid schema declarations.

Accepted binding types:

- `type_string()`
- `type_int()`
- `type_bool()`

Boolean values accept `true`, `false`, `1`, `0`, `on`, `off`, `yes`, and
`no`. Integer values must parse as signed 64-bit decimal integers.

## Validation

R-2212 is covered by:

- `packages/spectra-api/src/query.rs` unit tests.
- `tests/validation/136_api_query_binding.spectra`.
- `scripts/validate_r2212_query_binding.py`.
- the full `run_tests.ps1` suite.
