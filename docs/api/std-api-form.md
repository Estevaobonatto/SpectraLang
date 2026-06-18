# std.api.form

`std.api.form` parses and binds `application/x-www-form-urlencoded`
request bodies for Spectra API programs.

## Types

- `Form`: parsed form handle. It stores decoded keys and values.
- `FormSchema`: typed binding schema. Fields declare name, value type,
  required status, and whether repeated values are accepted.
- `FormBinding`: result of binding a `Form` to a `FormSchema`.

## Parsing

`parse(input)` accepts the raw request body without the media type header:

```spectra
let form = parse("name=Ada+Lovelace&tags[]=math&tags[]=api");
```

The parser follows `application/x-www-form-urlencoded` rules:

- `%HH` byte escapes are decoded and validated as UTF-8.
- `+` decodes to a space.
- raw control characters are rejected.
- empty field names are rejected.
- malformed bracket notation is rejected.

Bracket notation is normalized for binding:

- `tags[]=math&tags[]=api` becomes repeated field `tags`.
- `profile[city]=London` becomes nested field path `profile.city`.
- `profile[address][city]=London` becomes `profile.address.city`.

## Accessors

Repeated values are addressable by index:

```spectra
let tag_count = count(form, "tags");
let first_tag = value(form, "tags", 0);
let city = first(form, "profile.city");
```

Scalar coercion helpers return typed values and record typed errors through
`error_code()` and `error_message()` when coercion fails:

- `int(form, field, index)`
- `bool(form, field, index)`

Boolean values accept `true`, `false`, `1`, `0`, `on`, `off`, `yes`, and
`no`. Integer values must parse as signed 64-bit decimal integers.

## Typed Binding

Bindings are schema-driven until package-level native extern declarations
and reflection over user structs are available:

```spectra
let s0 = schema();
let s1 = schema_field(s0, "name", type_string(), true, false);
let s2 = schema_field(s1, "age", type_int(), true, false);
let s3 = schema_field(s2, "tags", type_string(), false, true);
let binding = bind(form, s3);
```

`binding_ok(binding)` reports whether all fields passed validation.
Failures are typed and field-specific:

- missing required field
- duplicate scalar field
- invalid integer
- invalid boolean
- invalid schema declaration

Duplicate keys are accepted for repeated fields only. If a schema marks a
field as scalar and the body contains it more than once, binding fails with
the offending field name in `binding_error(binding)`.

## Validation

R-2213 is covered by:

- `packages/spectra-api/src/form.rs` unit tests.
- `tests/validation/137_api_form_binding.spectra`.
- `scripts/validate_r2213_form_binding.py`.
- the full `run_tests.ps1` suite.
