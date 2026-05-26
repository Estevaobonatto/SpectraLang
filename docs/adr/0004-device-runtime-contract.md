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

The public API is:

- `device(handle) -> int`
- `device_available(device: int) -> bool`
- `to_device(handle, device: int) -> int`
- `cpu(handle) -> int`
- `sync(handle) -> unit`
- `stats_device_transfers() -> int`

`to_device(handle, 0)` creates a materialized CPU tensor handle preserving dtype, shape, value data, existing gradient data, and `requires_grad`. Unsupported or unknown device codes fail fast with `HOST_STATUS_INVALID_ARGUMENT`; invalid tensor handles fail with `HOST_STATUS_NOT_FOUND`. `sync` is a no-op for CPU tensors and validates the handle.

## Consequences

`R-701` can be complete without pretending a GPU backend exists: tensors are device-aware, CPU placement is implemented and tested, and transfer/sync failure modes are deterministic.

`R-702` remains blocked until the project adds one real production accelerator backend with kernel implementations and benchmark evidence. `R-703` remains blocked until that backend supports `f16`/`bf16` mixed-precision execution and training stability tests.
