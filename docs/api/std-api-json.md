# std.api.json

`std.api.json` is the Phase 22 JSON codec surface for Spectra API programs.
The native implementation lives in `packages/spectra-api/src/json.rs`.

## Values

The codec supports the RFC 8259 JSON data model:

- `null`
- booleans
- finite numbers
- strings with standard JSON escapes and Unicode escapes
- arrays
- objects/maps with string keys
- nested combinations of the values above

## Decoder

The decoder parses a complete JSON document and returns a typed value.
Invalid input reports:

- `kind`: syntax, unexpected EOF, data, or I/O classification
- `offset`: byte offset in the original document
- `line` and `column`
- `message`: parser detail from the underlying RFC 8259 parser

The host-call compatibility surface keeps:

- `spectra.api.json.validate`
- `spectra.api.json.kind`

Both host calls use the same full decoder as the Rust API. They do not use
brace balancing or partial classification.

## Encoder

The encoder serializes supported values to RFC 8259 JSON. It rejects non-finite
numbers such as `NaN` and infinity, and it validates stored number
representations before writing output.

Objects are represented with deterministic key ordering in the native API.

## Validation

R-2208 is validated by:

- `cargo test -p spectra-api json --offline`
- `scripts/validate_r2208_json_codec.py`
