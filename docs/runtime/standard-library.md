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
| `spectra.std.tensor.device` / `device_available` / `device_status` | Query tensor placement, availability, and stable capability status. `device_status` returns `0` available, `1` unavailable backend/build/host, or `2` reserved device. | `handle` or device code | device code, bool, or status code |
| `spectra.std.tensor.to_device` / `cpu` / `sync` | Transfer to supported devices and synchronize. CPU (`0`) is supported in the default build; `wgpu` (`6`) is supported with `--features gpu` and a detected adapter. | `handle`, optional device code | handle or `0` |
| `spectra.std.tensor.precision` / `to_precision` | Query or quantize float tensor precision. Codes: `0` f64, `1` f32, `2` f16, `3` bf16. | `handle`, optional precision code | precision code or handle |
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
| `spectra.std.tensor.sum_t` / `mean_t` / `dot_t` | Differentiable scalar tensor loss primitives. | handles | handle |
| `spectra.std.tensor.matmul` / `matmul_batched` / `dot` | Matrix/vector kernels with shape validation. | handles | handle or scalar |
| `spectra.std.tensor.seed` | Set deterministic tensor RNG seed. | `seed` | `0` |
| `spectra.std.tensor.requires_grad` / `backward` / `grad` / `zero_grad` | Reverse-mode autodiff controls. | handles, bool flag | handle or `0` |
| `spectra.std.tensor.set_grad_enabled` / `grad_enabled` | Inference/no-grad mode. | bool flag or none | `0` or bool |
| `spectra.std.tensor.stats_graph_nodes` | Live autograd creator node count. | none | integer metric |
| `spectra.std.tensor.stats_*` / `kernel_strategy` / `reset_stats` | Allocation, buffer-pool, scratch, device-transfer, GPU-kernel, CPU-fallback, and kernel work metrics. | none | integer metric or `0` |
| `spectra.std.tensor.free` / `free_all` | Release tensor handles. | `handle` or none | `0` or freed count |

## Usage Notes

- All collection handles are process-local and must be treated as opaque identifiers by Spectra
  programs.
- Allocation failures (for example, when the manual heap exceeds its soft limit) produce
  `HOST_STATUS_INTERNAL_ERROR`.
- GPU acceleration is optional. The default build keeps CPU semantics available; with
  `--features gpu`, WGPU float kernels dispatch for supported tensor ops and fall back
  to CPU when a dispatchable kernel reports failure.
- Passing invalid handles or mismatched argument counts yields `HOST_STATUS_INVALID_ARGUMENT` or
  `HOST_STATUS_NOT_FOUND`.
- Tensor shape mismatches return `HOST_STATUS_INVALID_ARGUMENT`; invalid handles return
  `HOST_STATUS_NOT_FOUND`.
- Autodiff is supported for float tensors. Scalar host-returning calls are not graph nodes; use
  `sum_t`, `mean_t`, and `dot_t` when a differentiable scalar tensor loss is required.
- `backward` releases graph creator nodes by default. Disable graph construction in inference
  sections with `set_grad_enabled(false)` and restore it with `set_grad_enabled(true)`.

## `std.ml`

`std.ml` is the Phase 6 high-level ML layer over `std.tensor` handles.

| Host call | Description | Arguments | Results |
|-----------|-------------|-----------|---------|
| `spectra.std.ml.module_new` | Create a module handle. | none | module handle |
| `spectra.std.ml.module_add_parameter` / `module_parameter_count` / `module_parameter` | Register and inspect tensor parameters. | module, tensor/index | `0`, count, or tensor handle |
| `spectra.std.ml.module_set_training` / `module_is_training` | Toggle training/eval mode. | module, bool | `0` or bool |
| `spectra.std.ml.linear` | Differentiable dense layer. | input, weight, bias | tensor handle |
| `spectra.std.ml.conv2d` | Differentiable valid 2D convolution over flattened NCHW tensors. | input, kernel, bias, dimensions | tensor handle |
| `spectra.std.ml.dropout` / `max_pool2d` | Inference/training utilities. | tensor and layer params | tensor handle |
| `spectra.std.ml.mse_loss` / `bce_loss` / `cross_entropy_loss` / `nll_loss` | Scalar tensor losses for autodiff. | predictions/logits, targets | loss tensor handle |
| `spectra.std.ml.sgd_step` / `sgd_momentum_step` / `adam_step` / `adamw_step` | In-place optimizer updates from accumulated gradients. | parameter, state tensors, hyperparameters | `0` |
| `spectra.std.ml.exp_lr` | Exponential learning-rate schedule. | base, gamma, step | float bits |
| `spectra.std.ml.unscale_grad` | Divide accumulated parameter gradient by a finite loss scale. | parameter, scale | `0` |
| `spectra.std.ml.dataset_from_tensors` / `dataset_len` | Tensor-backed datasets. | features, labels, length | dataset handle or length |
| `spectra.std.ml.dataloader_new` / `dataloader_batch_*` | Deterministic minibatch access. | dataset/loader, batch index | loader handle, count, or tensor handle |
- Host calls are idempotent where practical; re-registering the standard library simply replaces
  existing bindings with the same implementations.
