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

## tensor namespace

Spectra exposes tensor operations through runtime-managed opaque handles. The alpha tensor runtime
stores CPU tensors with dtype (`int` or `float`), shape, strides, layout, shared storage, and a base
offset for safe views. Float values use the same host-call convention as other float stdlib
functions: f64 bits encoded in `SpectraHostValue`.

| Host call | Description | Arguments | Results |
|-----------|-------------|-----------|---------|
| `spectra.std.tensor.zeros` | Allocate 1D int tensor filled with zero. | `size` | handle |
| `spectra.std.tensor.ones` | Allocate 1D int tensor filled with one. | `size` | handle |
| `spectra.std.tensor.full` | Allocate 1D int tensor filled with value. | `size`, `value` | handle |
| `spectra.std.tensor.full_f` | Allocate 1D float tensor filled with value. | `size`, `value_bits` | handle |
| `spectra.std.tensor.arange` | Allocate 1D int range tensor. | `start`, `end`, `step` | handle |
| `spectra.std.tensor.zeros2` / `ones2` / `full2` / `full2_f` | Allocate 2D tensors. | `rows`, `cols`, optional `value` | handle |
| `spectra.std.tensor.uniform` / `uniform_f` / `normal_f` / `bernoulli` / `categorical` | Seeded random tensor fills. | `size`, distribution parameters | handle |
| `spectra.std.tensor.len` / `rank` / `dim` / `rows` / `cols` | Query tensor metadata. | `handle`, optional `axis` | integer metadata |
| `spectra.std.tensor.get` / `get_f` / `get2` / `get2_f` | Read tensor values. | `handle`, index or row/col | scalar |
| `spectra.std.tensor.set` / `set_f` / `set2` / `set2_f` | Mutate tensor values. | `handle`, index/row/col, value | `0` |
| `spectra.std.tensor.reshape` | Return a validated 2D view handle when possible. | `handle`, `rows`, `cols` | handle |
| `spectra.std.tensor.flatten` | Return a 1D view or materialized tensor handle. | `handle` | handle |
| `spectra.std.tensor.permute` / `transpose` | Return axis-swapped view handles. | `handle`, axes or handle | handle |
| `spectra.std.tensor.slice` | Return a 1D shared-storage slice view. | `handle`, `start`, `end` | handle |
| `spectra.std.tensor.concat` / `stack` | Combine compatible tensors. | handles | handle |
| `spectra.std.tensor.add` / `sub` / `mul` / `div` | Elementwise arithmetic with exact shape and dtype match. | `lhs`, `rhs` | handle |
| `spectra.std.tensor.neg` / `relu` / `exp_f` / `log_f` / `sqrt_f` / `sigmoid_f` / `tanh_f` | Unary CPU kernels. | `handle` | handle |
| `spectra.std.tensor.sum` / `sum_f` / `mean_f` / `min` / `max` / `argmax` | Reductions. | `handle` | scalar |
| `spectra.std.tensor.matmul` / `matmul_batched` / `dot` | Matrix/vector kernels with shape validation. | handles | handle or scalar |
| `spectra.std.tensor.seed` | Set deterministic tensor RNG seed. | `seed` | `0` |
| `spectra.std.tensor.stats_*` / `kernel_strategy` / `reset_stats` | Allocation, buffer-pool, scratch, and kernel work metrics. | none | integer metric or `0` |
| `spectra.std.tensor.free` / `free_all` | Release tensor handles. | `handle` or none | `0` or freed count |

## Usage Notes

- All collection handles are process-local and must be treated as opaque identifiers by Spectra
  programs.
- Allocation failures (for example, when the manual heap exceeds its soft limit) produce
  `HOST_STATUS_INTERNAL_ERROR`.
- Passing invalid handles or mismatched argument counts yields `HOST_STATUS_INVALID_ARGUMENT` or
  `HOST_STATUS_NOT_FOUND`.
- Tensor shape mismatches return `HOST_STATUS_INVALID_ARGUMENT`; invalid handles return
  `HOST_STATUS_NOT_FOUND`.
- Host calls are idempotent where practical; re-registering the standard library simply replaces
  existing bindings with the same implementations.
