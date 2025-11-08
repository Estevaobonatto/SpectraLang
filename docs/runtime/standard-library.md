# Spectra Runtime Standard Library (Alpha)

The Spectra runtime ships a minimal host-driven standard library implemented as registered host
functions. The functions are grouped by namespace and can be installed by calling
`spectra_runtime::register_standard_library()` (or invoking `spectra_rt_std_register` once it is
gated through the CLI).

All host calls use the shared [`SpectraHostCallContext`](host-call-conventions.md) contract and the
status codes defined in `runtime::ffi` (`HOST_STATUS_*`). Arguments and results are encoded as
64-bit values (`SpectraHostValue`).

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
| `spectra.std.math.mean` | Arithmetic mean of one or more integers (integer division, truncates toward zero). | variadic | floor(mean(values)) |

Overflow yields `HOST_STATUS_ARITHMETIC_ERROR`; invalid input (division by zero, negative exponents, inverted ranges, empty argument lists) returns `HOST_STATUS_INVALID_ARGUMENT`.

## io namespace

| Host call | Description | Arguments | Results |
|-----------|-------------|-----------|---------|
| `spectra.std.io.print` | Prints all arguments as integers separated by spaces and terminates with a newline. | variadic | argument count written to `results[0]` when available |
| `spectra.std.io.flush` | Flushes the process stdout stream. | *(none)* | `0` when `results` is provided |

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
- Host calls are idempotent where practical; re-registering the standard library simply replaces
  existing bindings with the same implementations.
