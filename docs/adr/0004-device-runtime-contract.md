# ADR 0004: Tensor Device Runtime Contract

Status: accepted for `R-701`; device-status semantics updated 2026-06-24 for `R-3024`

Date: 2026-05-26 (updated 2026-06-24)

## Context

Phase 7 requires explicit tensor placement semantics before any accelerator backend can be implemented honestly. The current runtime has a production CPU tensor stack and an optional `wgpu` backend. There is no checked-in CUDA, ROCm, Metal, DirectML, or Vulkan backend and no CI hardware proving accelerator behavior for those paths.

## Decision

`std.tensor` exposes device placement as an explicit runtime contract over tensor handles:

- device code `0`: CPU, available in the default production build
- device code `1`: CUDA — **no implementation in this build** (R-3024)
- device code `2`: ROCm — **no implementation in this build** (R-3024)
- device code `3`: Metal — **no implementation in this build** (R-3024)
- device code `4`: DirectML — **no implementation in this build** (R-3024)
- device code `5`: Vulkan — **no implementation in this build** (R-3024)
- device code `6`: `wgpu`, available when the optional `gpu` feature is enabled and a real adapter is detected

The public API is:

- `device(handle) -> int`
- `device_available(device: int) -> bool`
- `to_device(handle, device: int) -> int`
- `cpu(handle) -> int`
- `sync(handle) -> unit`
- `stats_device_transfers() -> int`

`to_device(handle, 0)` creates a materialized CPU tensor handle preserving dtype, shape, value data, existing gradient data, and `requires_grad`. `to_device(handle, 6)` creates an accelerator-tagged float tensor in `f32` precision for `wgpu` kernels when the feature and adapter are available. Device codes `1..=5` are not implemented in this build and fail fast with `HOST_STATUS_INVALID_ARGUMENT` from `device_status`, `device_available`, and `to_device` (R-3024). This replaces the earlier "reserved but not implemented" status code 2, which misled callers into expecting a future CUDA/ROCm/Metal backend that does not exist. Unknown device codes (`< 0` or `> 6`) also fail with `HOST_STATUS_INVALID_ARGUMENT`. Invalid tensor handles fail with `HOST_STATUS_NOT_FOUND`. `sync` validates the handle and acts as the explicit synchronization point (real device-queue wait is R-3022).

## Consequences

`R-701` is complete: tensors are device-aware, CPU placement is implemented and tested, and transfer/sync failure modes are deterministic.

`R-3024` (reopened 2026-06-24): device codes 1..=5 are now honest about having no implementation. Adding a real CUDA/ROCm/Metal/DirectML/Vulkan backend in the future will require updating this ADR and the `is_implemented` predicate in `runtime/src/stdlib/mod.rs`.

`R-702` and `R-1603` are in progress: the WGPU backend is the only accelerator path; production speedup and the missing backward kernels are tracked in `.kilo/plans/1782330688549-gpu-production-implementation-plan.md`. `R-703` is in progress: f16/bf16 GPU execution is tracked as R-3071..R-3073.
