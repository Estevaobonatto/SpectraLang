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
