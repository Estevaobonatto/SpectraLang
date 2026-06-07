# R-1601 Tensor Graph IR

Updated: 2026-06-06

Roadmap item: `R-1601 Tensor Graph IR`

## Purpose

The Tensor Graph IR is the midend representation used as the handoff point for graph optimization, fusion, export, and accelerator lowering. It does not replace the existing SSA IR or runtime host-call ABI. Instead, it extracts tensor-producing host calls from lowered SSA and builds a stable graph view with operator, shape, dtype, layout, device, dependency, and source-location metadata.

## Production Contract

- Graph extraction entry point: `spectra_midend::TensorGraph::from_ir_module(&IRModule)`.
- Validation entry point: `TensorGraph::validate()`.
- Stable dump entry point: `TensorGraph::stable_dump()`.
- Graph nodes use deterministic IDs based on lowered instruction order.
- Unknown metadata is represented explicitly as `?`, not guessed.
- Supported validation failures are stable categories: cycle, shape mismatch, device mismatch, unsupported operator, and invalid dependency.

## Covered Operators

- Tensor creation: `zeros`, `ones`, `full`, `full_f`, `arange`, `zeros2`, `ones2`, `full2`, `full2_f`, `uniform`, `uniform_f`, `normal_f`, `bernoulli`.
- Tensor transforms: `reshape`, `transpose`, `to_device`, `cpu`.
- Tensor math: `add`, `sub`, `mul`, `div`, `neg`, `relu`, `sigmoid_f`, `tanh_f`, `sqrt_f`, `log_f`, `matmul`, `matmul_batched`, `sum_t`.
- ML operators: `linear`, `conv2d`, `dropout`, `max_pool2d`, `mse_loss`, `bce_loss`.

## Validation Behavior

- Matmul validates `lhs.dim1 == rhs.dim0` when both dimensions are known.
- Elementwise and loss operators validate compatible ranked shapes when known.
- Binary tensor operators validate same-device inputs when both devices are known.
- Dependency validation rejects references to missing graph nodes.
- Cycle validation rejects cyclic dependency graphs.

## Test Gate

Run:

```powershell
cargo test -p spectra-midend --test tensor_graph_tests
```

The gate includes:

- snapshot extraction from a real lowered `.spectra` tensor program;
- shape mismatch validation;
- device mismatch validation;
- cycle validation.

