# ADR 0004: Tensor Device Runtime Contract

Status: accepted for `R-701`

Date: 2026-05-26

## Context

Phase 7 requires explicit tensor placement semantics before any accelerator backend can be implemented honestly. The current runtime has a production CPU tensor stack, but no checked-in CUDA, ROCm, Metal, DirectML, or Vulkan backend and no CI hardware proving accelerator behavior.

## Decision

`std.tensor` exposes device placement as an explicit runtime contract over tensor handles:

- device code `0`: CPU, available in the default production build
- device code `1`: CUDA, reserved and currently unavailable
- device code `2`: ROCm, reserved and currently unavailable
- device code `3`: Metal, reserved and currently unavailable
- device code `4`: DirectML, reserved and currently unavailable
- device code `5`: Vulkan, reserved and currently unavailable
- device code `6`: `wgpu`, available when the optional `gpu` feature is enabled and a real adapter is detected

The public API is:

- `device(handle) -> int`
- `device_available(device: int) -> bool`
- `to_device(handle, device: int) -> int`
- `cpu(handle) -> int`
- `sync(handle) -> unit`
- `stats_device_transfers() -> int`

`to_device(handle, 0)` creates a materialized CPU tensor handle preserving dtype, shape, value data, existing gradient data, and `requires_grad`. `to_device(handle, 6)` creates an accelerator-tagged float tensor in `f32` precision for `wgpu` kernels when the feature and adapter are available. Unsupported or unknown device codes fail fast with `HOST_STATUS_INVALID_ARGUMENT`; invalid tensor handles fail with `HOST_STATUS_NOT_FOUND`. `sync` validates the handle and acts as the explicit synchronization point.

## Consequences

`R-701` is complete: tensors are device-aware, CPU placement is implemented and tested, and transfer/sync failure modes are deterministic.

`R-702` is complete for the current baseline: `wgpu` kernels cover float elementwise arithmetic, `relu`, `sum_f`, `matmul`, and `ml.conv2d`, with hardware validation and benchmark timing evidence. `R-703` is complete for the current baseline through f32/f16/bf16 quantization, precision metadata, loss scaling support, and a converging mixed-precision training example.
