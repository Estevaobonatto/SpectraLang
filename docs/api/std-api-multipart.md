# std.api.multipart

`std.api.multipart` parses `multipart/form-data` request bodies and exposes
text fields and uploaded files through stable Spectra handles.

## Types

- `Multipart`: parsed multipart handle.
- `MultipartPart`: one parsed part. Text parts expose their UTF-8 value
  through `text`; file parts expose metadata and chunked file readers.

## Parsing

`parse(body, boundary, max_total_bytes, max_parts, max_part_bytes)` accepts
the raw body bytes as a Spectra string plus the boundary value from the
`Content-Type` header:

```spectra
let uploads = multipart.parse(body, "BOUNDARY", 10485760, 32, 8388608)
```

The parser rejects empty, malformed, whitespace-containing, quoted, and
semicolon-containing boundaries. It requires the opening boundary, validates
part headers, requires `Content-Disposition: form-data`, and extracts the
`name`, `filename`, and `Content-Type` attributes for each part.

## Limits

The parser enforces all limits before exposing handles:

- `max_total_bytes`: maximum request body size.
- `max_parts`: maximum number of multipart parts.
- `max_part_bytes`: maximum size of any single part.

Violations are reported through `error_code()` and `error_message()` with
typed categories for total-size, part-size, and part-count failures.

## Text Fields

Text fields are UTF-8 decoded and addressable by field name and index:

```spectra
let title = multipart.text(uploads, "title", 0)
```

`part_count`, `field_count`, and `file_count` report the parsed shape without
materializing file contents.

## File Uploads

File parts are spooled to a temporary file managed by the runtime after the
multipart frame is parsed. The `MultipartPart` handle exposes metadata and
chunked readers:

```spectra
let part = multipart.part(uploads, 1)
let name = multipart.part_name(part)
let filename = multipart.part_filename(part)
let content_type = multipart.part_content_type(part)
let size = multipart.part_size(part)
let first_chunk = multipart.file_read(part, 0, 4096)
```

`file_spool_to(part, path)` copies the spooled upload to an application-owned
path without requiring user code to keep the full file content in memory.

## Validation

R-2214 is covered by:

- `packages/spectra-api/src/multipart.rs` unit tests.
- `tests/validation/138_api_multipart_uploads.spectra`.
- `scripts/validate_r2214_multipart_uploads.py`.
- the full `run_tests.ps1` suite.
