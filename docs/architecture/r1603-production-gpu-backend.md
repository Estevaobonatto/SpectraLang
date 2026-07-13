# R-1603 Production GPU Backend

Status: in progress (reopened 2026-06-24, see `.kilo/plans/1782330688549-gpu-production-implementation-plan.md`).

## Contract

The default runtime build remains portable and CPU-only. Accelerator execution is enabled through the optional Cargo feature:

```powershell
cargo test -p spectra-runtime --features gpu tensor_runtime_r1603
```

Device codes are stable:

| Code | Device | Status |
|---|---|---|
| `0` | CPU | Always available |
| `1` | CUDA | Reserved |
| `2` | ROCm | Reserved |
| `3` | Metal | Reserved |
| `4` | DirectML | Reserved |
| `5` | Vulkan | Reserved |
| `6` | WGPU | Available only with `--features gpu` and a detected adapter |

`std.tensor.device_status(device)` returns:

| Code | Meaning |
|---|---|
| `0` | Device is available |
| `1` | Device backend exists but is unavailable in this build or host environment |
| `2` | Device code is reserved but not implemented |

Invalid device codes return `HOST_STATUS_INVALID_ARGUMENT`.

## Kernel Coverage

The WGPU backend executes float tensor kernels for:

- tensor transfer through `to_device(..., 6)` and `cpu`
- elementwise `add`, `sub`, `mul`, `div`
- unary `neg` and `relu`
- reductions through `sum` and `sum_f`
- matrix multiplication through `matmul`
- convolution through `std.ml.conv2d`
- autodiff-required forward kernels used by `backward` tests

Backward graph traversal remains runtime autograd over tensor handles. GPU forward tensors preserve device placement where supported; scalar loss and gradient tensors can be materialized through the existing handle API.

## Fallback and Diagnostics

GPU kernel failures no longer abort the operation when a CPU equivalent exists. The runtime records:

- `std.tensor.stats_gpu_kernel_ops()`: successful accelerator kernel dispatches
- `std.tensor.stats_cpu_fallbacks()`: accelerator kernel failures that used CPU fallback
- `std.tensor.stats_device_transfers()`: explicit device transfers
- `std.tensor.kernel_strategy()`: active dispatch family (`5` when WGPU is available)
- `std.tensor.stats_gpu_errors(kind)`: per-kind GPU error counter (R-3023). `kind` is one of:
  - `0` ShapeMismatch
  - `1` ShaderCompile
  - `2` BufferAlloc
  - `3` Dispatch
  - `4` Readback
  - `5` FeatureUnsupported
  - `6` Other

CPU fallback uses the same scalar kernels as the default runtime and is validated against the existing numerical tolerance policy from R-1503.

## Device Code Semantics (R-3024)

`device_status(0)` returns `0` (CPU available). `device_status(6)` returns `0` if a real WGPU adapter is detected, or `1` (backend exists but unavailable in this build/host) when the optional `gpu` feature is off or no adapter is found. `device_status(1..=5)` returns `HOST_STATUS_INVALID_ARGUMENT` — the historical "reserved but not implemented" status code `2` was misleading and has been removed. Adding a real CUDA/ROCm/Metal/DirectML/Vulkan backend will require updating the `is_implemented` predicate in `runtime/src/stdlib/mod.rs` and ADR 0004.

## Validation

Required gate:

```powershell
python scripts\validate_r1603_gpu_backend.py
```

The script runs:

- `cargo test -p spectra-runtime tensor_runtime_r1603_default_cpu_fallback_and_diagnostics`
- `cargo test -p spectra-runtime --features gpu tensor_runtime_r1603 -- --nocapture`

Public Spectra validation:

```powershell
cargo run -p spectra-cli -- check tests\validation\91_tensor_phase16_gpu_backend.spectra
cargo run -p spectra-cli -- run tests\validation\91_tensor_phase16_gpu_backend.spectra
```

The Spectra validation skips accelerator execution safely when WGPU is unavailable, while still validating CPU device status and reserved-device diagnostics.

## Device Memory (R-3051, R-3021, R-3052 full)

Land 2026-06-24 with the Block 3 cornerstone step:

- `runtime/src/gpu.rs` adds `DeviceBuffer` (Arc<wgpu::Buffer> wrapper) and
  `DeviceArena` keyed by `(device, dtype, size_bucket)`. Bucket function is
  `next_power_of_two(n).max(16)`. Per-bucket free list capped at
  `MAX_FREE_PER_BUCKET = 16`.
- `TensorRegistry` holds the arena. No new lock surface; the arena shares
  the existing registry mutex.
- `to_device(_, 6)` acquires from the pool, enqueues a `queue.write_buffer`
  of the host f32 mirror, submits, and stores the resident buffer on the
  new tensor's `device_storage` field.
- `tensor.free` and `tensor.free_all` release the buffer through
  `recycle_tensor` (the same hook the host buffer pool already uses).
- New host calls:
  - `std.tensor.stats_device_pool_hits()` — pool reuse counter
  - `std.tensor.stats_device_pool_misses()` — fresh allocation counter
  - `std.tensor.stats_device_pool_bytes_resident()` — bytes held by the
    arena (free list + in flight)
  - `std.tensor.storage_device(handle)` — returns 0 (Cpu) or 6 (Wgpu) for
    the tensor's residency
- `reset_stats` clears the arena (free lists, hits, misses, bytes_resident).
- Reuse safety: every acquire is followed by a full `queue.write_buffer`
  (data overwrite) before the buffer can be released back to the pool.
  See the `DeviceArena::acquire` doc-comment in `runtime/src/gpu.rs`.

Residency-aware dispatch (R-3052 full) is wired: forward ops (`matmul`,
`conv2d`, `relu`/`neg`, binary ops, reductions, `ml.linear`, and
`ml.mse_loss`) consume `device_storage` pool buffers directly and write
resident outputs back into fresh pool buffers without host readback between
chained ops. Backward for `ml.mse_loss`, `ml.linear`, `relu`, and `matmul`
accumulates into `device_grad`, and `ml.sgd_step` consumes device gradients
to update resident parameters without a host round trip. Explicit scalar
loss reads and `tensor.grad()` inspection remain readback boundaries.

Validation:

- `tensor_runtime_r3021_real_upload_after_to_device` — `to_device` actually
  uploads; `storage_device` reports 6.
- `tensor_runtime_r3051_pool_reuse_under_load` — 100 same-shape
  `to_device` + `free` cycles; `pool_hits >= 99`, `pool_misses <= 1`.
- `tensor_runtime_r3051_pool_recycles_after_free` — second `to_device`
  reuses the pool.
- `tensor_runtime_r3052_full_resident_*` — resident matmul, chain,
  `ml.linear`, relu, binary ops, pool release, backward accumulation, and
  SGD update stay on device and match CPU results within tolerance.
- `runtime/examples/tensor_phase7_gpu_bench.rs` extended JSON includes
  `pool_hits`, `pool_misses`, `pool_bytes_resident`, `device_pool_tested`.
  On RTX 2060: `pool_hits=99, pool_misses=1, pool_bytes_resident=1024`.
- `python scripts/validate_r1603_gpu_backend.py` runs the R-3021, R-3051,
  R-3052, and R-3080 GPU validation set.
