# Spectra Runtime Standard Library (Alpha)

The Spectra runtime ships a minimal host-driven standard library implemented as registered host
functions. The functions are grouped by namespace and can be installed by calling
`spectra_runtime::register_standard_library()` (or invoking `spectra_rt_std_register` once it is
gated through the CLI).

All host calls use the shared [`SpectraHostCallContext`](host-call-conventions.md) contract and the
status codes defined in `runtime::ffi` (`HOST_STATUS_*`). Arguments and results are encoded as
64-bit values (`SpectraHostValue`).

## Versioning

The standard library follows semantic versioning aligned with the Spectra runtime release train:

- **MAJOR** versions bump when breaking API or behavioral changes are introduced (e.g., incompatible
  host-call signatures, altered error semantics). Breaking updates only ship alongside a runtime
  MAJOR release and require explicit opt-in flags when available.
- **MINOR** versions add backwards-compatible functionality such as new host calls, optional
  arguments, or improved documentation. Minor updates are the default cadence between runtime
  feature releases.
- **PATCH** versions are reserved for bug fixes or clarifications that do not alter public
  contracts.

Host-call symbols remain stable within a MAJOR line. Deprecations progress through documentation
  warnings before removal; removal occurs only with a subsequent MAJOR bump. Tooling discovering the
  stdlib should rely on the runtime/CLI version string to determine compatibility.

## math namespace

| Host call | Description | Arguments | Results |
|-----------|-------------|-----------|---------|
| `spectra.std.math.abs` | Absolute value for signed integers. | `x` | `abs(x)` |
| `spectra.std.math.min` | Returns the smaller of two integers. | `lhs`, `rhs` | `min(lhs, rhs)` |
| `spectra.std.math.max` | Returns the larger of two integers. | `lhs`, `rhs` | `max(lhs, rhs)` |
| `spectra.std.math.clamp` | Clamps an integer to the provided inclusive range. | `value`, `min`, `max` | `min(max(value, min), max)` |
| `spectra.std.math.add` | Integer addition with overflow checking. | `lhs`, `rhs` | `lhs + rhs` |
| `spectra.std.math.sub` | Integer subtraction with overflow checking. | `lhs`, `rhs` | `lhs - rhs` |
| `spectra.std.math.mul` | Integer multiplication with overflow checking. | `lhs`, `rhs` | `lhs * rhs` |
| `spectra.std.math.div` | Integer division rejecting division by zero. | `numerator`, `denominator` | `numerator / denominator` |
| `spectra.std.math.mod` | Remainder operation rejecting division by zero. | `numerator`, `denominator` | `numerator % denominator` |
| `spectra.std.math.pow` | Integer exponentiation for non-negative exponents. | `base`, `exponent` | `base^exponent` |
| `spectra.std.math.float_to_int` | Converts a 64-bit float to a 64-bit integer with saturation. | `value` | saturated integer |
| `spectra.std.math.int_to_float` | Converts a 64-bit integer to a 64-bit float. | `value` | float64 |
| `spectra.std.math.float_add` | Adds two 64-bit floats. | `lhs`, `rhs` | float64 |
| `spectra.std.math.float_sub` | Subtracts two 64-bit floats. | `lhs`, `rhs` | float64 |
| `spectra.std.math.float_mul` | Multiplies two 64-bit floats. | `lhs`, `rhs` | float64 |
| `spectra.std.math.float_div` | Divides two 64-bit floats (propagates NaN, Inf). | `lhs`, `rhs` | float64 |
| `spectra.std.math.trig_sin` | Computes the sine of a 64-bit float angle in radians. | `radians` | float64 |
| `spectra.std.math.trig_cos` | Computes the cosine of a 64-bit float angle in radians. | `radians` | float64 |
| `spectra.std.math.trig_tan` | Computes the tangent of a 64-bit float angle in radians. | `radians` | float64 |
| `spectra.std.math.trig_atan2` | Computes the arctangent of `y/x` using signs to determine the quadrant. | `y`, `x` | float64 |
| `spectra.std.math.float_abs` | Absolute value for 64-bit floats. | `value` | float64 |
| `spectra.std.math.float_sqrt` | Square root of a 64-bit float (domain errors yield NaN). | `value` | float64 |
| `spectra.std.math.float_exp` | Natural exponential of a 64-bit float. | `value` | float64 |
| `spectra.std.math.float_ln` | Natural logarithm of a 64-bit float (non-positive inputs yield NaN). | `value` | float64 |
| `spectra.std.math.float_pow` | Raises a float base to a float exponent using `powf`. | `base`, `exponent` | float64 |
| `spectra.std.math.mean` | Arithmetic mean of one or more integers (integer division, truncates toward zero). | variadic | floor(mean(values)) |
| `spectra.std.math.median` | Median of the provided integers (even counts yield the truncated average of the middle pair). | variadic | median value |
| `spectra.std.math.variance` | Population variance of the provided integers (integer division, truncates toward zero). | variadic | variance value |
| `spectra.std.math.std_dev` | Population standard deviation of the provided integers (integer arithmetic with floor square root). | variadic | floor(std_dev(values)) |
| `spectra.std.math.mode` | Most frequent integer (ties pick the smallest value). | variadic | mode value |
| `spectra.std.math.rng_seed` | Creates a deterministic RNG handle seeded with the provided value. | `seed` | RNG handle |
| `spectra.std.math.rng_next` | Advances the RNG and yields the next pseudo-random integer. | `handle` | pseudo-random `int` |
| `spectra.std.math.rng_next_range` | Advances the RNG and yields a value in the inclusive range. | `handle`, `min`, `max` | pseudo-random `int` within `[min, max]` |
| `spectra.std.math.rng_free` | Releases an RNG handle and its associated state. | `handle` | `0` when `results` is provided |
| `spectra.std.math.rng_free_all` | Releases all RNG handles tracked by the runtime. | *(none)* | number of freed handles |

Overflow yields `HOST_STATUS_ARITHMETIC_ERROR`; invalid input (division by zero, negative exponents, inverted ranges, empty argument lists) returns `HOST_STATUS_INVALID_ARGUMENT`.
`spectra.std.math.median` requires at least one argument and, for even-sized inputs, returns the truncated mean of the two middle values.
`spectra.std.math.variance` computes the population variance and truncates toward zero when dividing the accumulated sum of squared differences by the input count.
`spectra.std.math.std_dev` derives its result from the population variance and applies an integer square root, truncating fractional parts.
`spectra.std.math.mode` returns the smallest integer among the most frequent values when multiple modes exist.
Floating-point host calls treat `SpectraHostValue` payloads as IEEE-754 `f64` bit patterns. Use `std.math.int_to_float` and `std.math.float_to_int` to bridge between integer and floating-point domains.
`spectra.std.math.float_to_int` truncates toward zero and saturates to `i64::MIN`/`i64::MAX`; it yields `0` for NaN inputs. Other floating-point operations bubble standard IEEE-754 NaN/±Inf values without returning host errors.

```spectra
let angle = std.math.int_to_float(1);
let doubled = std.math.float_add(angle, angle);
let sine = std.math.trig_sin(angle);
let magnitude = std.math.float_sqrt(std.math.float_mul(sine, sine));
let back_to_int = std.math.float_to_int(doubled);
```

RNG handles are opaque identifiers; free them explicitly with `rng_free` (or `rng_free_all`) to avoid leaking manual allocations.

## io namespace

| Host call | Description | Arguments | Results |
|-----------|-------------|-----------|---------|
| `spectra.std.io.print` | Prints all arguments as integers separated by spaces and terminates with a newline. | variadic | argument count written to `results[0]` when available |
| `spectra.std.io.print_err` | Same contract as `print`, but writes to the process stderr stream. | variadic | argument count written to `results[0]` when available |
| `spectra.std.io.print_to_buffer` | Formats arguments like `print` and appends the resulting UTF-8 line (with trailing `\n`) to a list handle representing a byte buffer. | `buffer_handle`, variadic | new buffer length in bytes written to `results[0]` when available |
| `spectra.std.io.write_file` | Writes the UTF-8 bytes stored in a list handle to the target path (also provided as a list handle). Optional third argument appends instead of truncating. | `path_handle`, `data_handle`, `[append_flag]` | bytes written in `results[0]` when available |
| `spectra.std.io.read_file` | Reads an entire file into a list handle. When a target buffer handle is provided the contents replace it; otherwise a fresh buffer handle is allocated. | `path_handle`, `[target_buffer_handle]` | `results[0]` = buffer handle, `results[1]` = byte length |
| `spectra.std.io.flush` | Flushes the process stdout stream. | *(none)* | `0` when `results` is provided |

## log namespace

Structured logging uses numeric levels: `TRACE = 0`, `DEBUG = 1`, `INFO = 2`, `WARN = 3`, `ERROR = 4`.

| Host call | Description | Arguments | Results |
|-----------|-------------|-----------|---------|
| `spectra.std.log.set_level` | Sets the global minimum level; entries below this threshold are discarded. | `level` | new global level |
| `spectra.std.log.add_sink` | Registers a sink with an optional minimum level. Sink kinds: `0 = stdout`, `1 = stderr`, `2 = file (path handle)`, `3 = buffer (list handle)`, `4 = list of entries (list handle storing handles to JSON log records)`. | `kind`, `[config_handle]`, `[min_level]` | total number of sinks |
| `spectra.std.log.clear_sinks` | Removes all configured sinks. | *(none)* | cleared sink count |
| `spectra.std.log.record` | Emits an entry with the provided level, message, and metadata payload encoded either as a JSON object or comma-separated `key=value` pairs. | `level`, `message_handle`, `[metadata_handle]` | number of sinks that accepted the entry |

## text namespace

String handles guarantee valid UTF-8 storage while interoperating with list-based byte buffers for
I/O. Each handle owns an allocation that must be released explicitly when no longer needed.

| Host call | Description | Arguments | Results |
|-----------|-------------|-----------|---------|
| `spectra.std.text.new` | Allocates an empty UTF-8 string and returns its handle. | *(none)* | handle |
| `spectra.std.text.from_list` | Validates the bytes stored in a list handle as UTF-8 and, if valid, copies them into a new string handle. | `list_handle` | string handle |
| `spectra.std.text.to_list` | Copies the UTF-8 bytes stored in a string handle into a freshly allocated list handle. | `string_handle` | new list handle |
| `spectra.std.text.len` | Returns the number of Unicode scalar values stored in the string. | `string_handle` | length |
| `spectra.std.text.from_int` | Formats an integer as decimal ASCII characters and returns a new string handle. | `value` | string handle |
| `spectra.std.text.from_float` | Formats a 64-bit float using Rust's default `Display` implementation. | `value_bits` | string handle |
| `spectra.std.text.parse_int` | Parses a decimal integer, trimming surrounding ASCII whitespace. | `string_handle` | parsed integer |
| `spectra.std.text.parse_float` | Parses an IEEE-754 `f64` (accepts Rust/JSON-compatible syntax). | `string_handle` | float bits |
| `spectra.std.text.substring` | Extracts a slice by Unicode scalar index. The optional length parameter limits the number of scalars copied; absent length consumes the remainder of the string. | `string_handle`, `start_index`, `[length]` | string handle |
| `spectra.std.text.concat` | Produces a new string handle containing the concatenation of the provided handles. | `lhs_handle`, `rhs_handle` | string handle |
| `spectra.std.text.format` | Expands a template using positional placeholders (`{}` or `{index}`) pulled from a list of string handles. `{{` and `}}` escape literal braces. | `template_handle`, `values_list_handle` | string handle |
| `spectra.std.text.interpolate` | Replaces `${key}` markers using name/value string pairs supplied as an alternating list. `$$` yields a literal `$`. | `template_handle`, `pairs_list_handle` | string handle |
| `spectra.std.text.free` | Releases the allocation associated with a string handle. | `handle` | `0` when `results` provided |
| `spectra.std.text.free_all` | Releases every string handle tracked by the runtime. | *(none)* | number of freed strings |

`spectra.std.text.len` counts Unicode scalar values rather than raw bytes, making it safe to use for
user-facing text. `spectra.std.text.substring` operates on the same scalar indices, returning an
empty string when the start index equals the string length and rejecting out-of-range spans. The
formatting helpers (`from_int`/`from_float`/`format`/`interpolate`) allocate fresh handles, while the
parsing helpers return `HOST_STATUS_INVALID_ARGUMENT` when the payload cannot be interpreted as a
number. `std.text.format` consumes a list of string handles; use `{}` for sequential arguments,
`{n}` for explicit indices, and double braces for literals. `std.text.interpolate` expects an
alternating list of key/value handles and leaves `$$` sequences as a literal `$`. Use
`from_list`/`to_list` to bridge between I/O pipelines (which consume raw byte lists) and higher-level
text operations.

## time namespace

| Host call | Description | Arguments | Results |
|-----------|-------------|-----------|---------|
| `spectra.std.time.now` | Returns the current UTC time since the Unix epoch encoded as seconds and sub-second nanoseconds. | *(none)* | `results[0]` = seconds, `results[1]` = nanoseconds |
| `spectra.std.time.now_monotonic` | Reads a monotonic clock relative to the first invocation, suitable for measuring durations. | *(none)* | `results[0]` = seconds, `results[1]` = nanoseconds |
| `spectra.std.time.sleep` | Suspends execution for the provided duration. The first argument encodes whole seconds; an optional second argument supplies additional nanoseconds (0–999,999,999). | `seconds`, `[nanoseconds]` | `0` when `results` is provided |

## collections namespace

Spectra exposes list operations backed by runtime-managed vectors. Lists are represented by opaque
handles (integers) that map to manual allocations tracked by the runtime. Failing to free a list will
keep the allocation alive until `spectra.std.collections.list_free_all` is invoked or the process
terminates.

| Host call | Description | Arguments | Results |
|-----------|-------------|-----------|---------|
| `spectra.std.collections.list_new` | Allocates an empty list and returns its handle. | *(none)* | handle |
| `spectra.std.collections.list_push` | Appends an integer to the list referenced by the handle. | `handle`, `value` | new length |
| `spectra.std.collections.list_len` | Returns the current length of the list. | `handle` | length |
| `spectra.std.collections.list_clear` | Removes all elements from the list without releasing the handle. | `handle` | `0` |
| `spectra.std.collections.list_free` | Drops the list allocation associated with the handle. | `handle` | `0` when `results` provided |
| `spectra.std.collections.list_free_all` | Drops every list managed by the runtime. | *(none)* | number of freed lists |

## Usage Notes

- All collection handles are process-local and must be treated as opaque identifiers by Spectra
  programs.
- Allocation failures (for example, when the manual heap exceeds its soft limit) produce
  `HOST_STATUS_INTERNAL_ERROR`.
- Passing invalid handles or mismatched argument counts yields `HOST_STATUS_INVALID_ARGUMENT` or
  `HOST_STATUS_NOT_FOUND`.
- RNG handles behave like list handles: call `spectra.std.math.rng_free` (or
  `spectra.std.math.rng_free_all`) to release manual allocations when finished.
- Host calls are idempotent where practical; re-registering the standard library simply replaces
  existing bindings with the same implementations.
- `spectra.std.io.print_err` mirrors `print` but targets stderr, making it suitable for diagnostics
  without interleaving regular program output.
- `spectra.std.io.write_file`/`read_file` expect paths encoded as UTF-8 bytes within a `std.collections` list. File contents are produced/consumed as raw bytes (0–255) and stored in the same data structure for reuse across host calls.
- `spectra.std.io.print_to_buffer` is useful for capturing textual output in-memory; it emits UTF-8 bytes matching the console formatting of `print`.
- Logging sinks accept the formatted entry with a trailing newline. Buffer and file sinks reuse list handles to exchange UTF-8 bytes; list sinks (`kind = 4`) append the JSON representation of each log entry to the provided list handle for later inspection.
- List sinks store compact JSON objects containing `timestamp`, `level`, `message` and either `fields` (structured metadata) or `raw_metadata` when unstructured text is supplied.
- Metadata strings must be valid JSON objects or comma-separated `key=value` pairs. Key/value payloads support quoted strings (`"value"`), booleans, and numeric literals; malformed metadata yields `HOST_STATUS_INVALID_ARGUMENT`.
- `spectra.std.time.now` reports wall-clock seconds/nanoseconds since the Unix epoch, while `now_monotonic` is stable across clock adjustments and only advances. Both always return non-negative components encoded as `int` values.
- `spectra.std.time.sleep` relies on the operating system scheduler; it blocks for at least the requested duration but may resume slightly later depending on timer precision.
- String handles provide validated UTF-8 storage with Unicode-aware length reporting. Always pair
  `spectra.std.text.free` (or `free_all`) with any long-lived handle to avoid leaking manual
  allocations, and prefer `spectra.std.text.to_list` when interacting with APIs that expect raw
  byte buffers.
