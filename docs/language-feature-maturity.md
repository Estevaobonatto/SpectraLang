# Language Feature Maturity Policy

Updated: 2026-06-04
Roadmap item: `R-106`

This file is the source of truth for language maturity labels. Documentation, examples, and CLI behavior must match this policy exactly.

## Maturity Levels

- `stable`: enabled by default, documented as part of the normal language contract, and covered by the positive test suite
- `beta`: enabled by default and usable, but still expected to evolve in ergonomics or performance
- `experimental`: available only behind `--enable-experimental <feature>`
- `deferred`: documented only as roadmap/future work, not as usable language syntax

## Current Feature Matrix

### Stable

- modules and multi-file project discovery
- imports:
  - `import module.path;`
  - `import module.path as alias;`
  - `import { name } from module.path;`
  - `pub import { name } from module.path;`
- visibility: `pub`, `internal`
- functions and methods
- structs, enums, traits, impl blocks
- generics in the currently validated surface
- `dyn Trait` in the currently validated surface
- primitives, tuples, function types
- numeric aliases over the current canonical ABI:
  - `i8`, `i16`, `i32`, `i64`, `isize`
  - `u8`, `u16`, `u32`, `u64`, `usize`
  - `f16`, `bf16`, `f32`, `f64`
- top-level `const` evaluation for primitive literal/arithmetic/logical expressions
- control flow:
  - `if`, `elif`, `else`
  - `if let`
  - `while`
  - `while let`
  - `for ... in ...`
  - `for ... of ...`
  - `match`
  - `return`, `break`, `continue`
- tuple, struct, enum, and OR-patterns in the validated pattern surface
- closures/lambdas with by-value captures in the currently validated surface
- qualified stdlib calls such as `std.io.println(...)`
- `std.tensor` production baseline runtime API for tensor handles, safe views, shape metadata, elementwise ops, reductions, transforms, 2D matmul, and batched matmul
- `std.tensor` production baseline reverse-mode autodiff for float tensor handles, scalar tensor losses, gradient accumulation, and inference/no-grad mode
- Phase 14 partial tensor language core:
  - `Tensor<dtype, rankN>` annotations for compiler-visible dtype/rank metadata
  - explicitly typed rank1/rank2 float tensor literals
  - `diff { ... }` differentiable block syntax lowering to `std.tensor.backward`
- `std.tensor` production baseline device placement contract for CPU handles (`device`, `device_available`, `to_device`, `cpu`, `sync`, `stats_device_transfers`)
- optional `std.tensor` `wgpu` accelerator backend behind Cargo feature `gpu`
- `std.tensor` mixed-precision quantization metadata/API for f32, f16, and bf16 float tensor handles
- `std.ml` production baseline runtime API for modules, layers, losses, optimizers, LR scheduling, tensor-backed datasets, and dataloaders
- Phase 8 interop baseline:
  - Python bridge through `python/spectra_bridge.py`
  - Rust helper crate `spectra-interop`
  - stable C ABI header surface
  - NumPy `.npy` v1.0 little-endian f64 one-dimensional tensor exchange
- Phase 9 package baseline:
  - `spectralang package lock/build/check/run/test/bench/doc/add/update`
  - deterministic `spectra.lock`
  - multi-package workspace builds
  - exact semver version validation
  - local path dependencies
  - local filesystem registry publish/install with checksum validation
- Phase 10 tooling baseline:
  - LSP hover, go-to-definition, references, rename, completion, diagnostics, formatting, inlay hints, quick fixes, and semantic tokens
  - `spectralang bench` with JSON timing reports
  - source-aware `error[runtime]` diagnostics for non-zero program exits
- Phase 11 concurrency and serving baseline:
  - `std.concurrent` task handles, deterministic join, non-blocking FIFO channels, counters, stats/reset, and parallel pipeline sum
  - `std.serve` local in-process server handles, warmup, request batching, cancellation, timeout state, resident model lookup, result lookup, and deterministic toy benchmark
- Phase 12 security and operations baseline:
  - release manifests, SHA-256 checksums, signed release evidence, provenance, and CycloneDX-compatible SBOM
  - CI dependency scanning with `cargo audit` and high-severity `npm audit`
  - defined stress/soak runner with timeout, optional RSS limit, and JSON report
  - runtime debug invariant checks for host registry/manual allocation state
- Phase 13 documentation and adoption baseline:
  - `docs/book/` production adoption book for language basics, numerics, tensors, autodiff, model authoring, deployment/export, stdlib/runtime/packages, and benchmark/comparison workflow
  - six end-to-end AI reference examples under `examples/ai/`
  - automated book/example discoverability validation through `scripts/validate_ai_book.py`
  - AI example execution integrated into `run_tests.ps1`

### Beta

- class syntax footprint
- `static` item surface
- mutable/reference closure captures beyond the current by-value capture contract
- first-class tensor language design beyond the current stdlib handle/autodiff API
- native DWARF/PDB source stepping beyond the current AOT debug-map workflow
- HTTP/gRPC serving, async I/O integration, and distributed model residency policy

These are usable where covered, but still not treated as fully production-hardened language design.

### Experimental

These features must remain hidden behind the CLI feature gate and are the exact values returned by `spectralang --list-experimental`.

- `switch`
- `unless`
- `do-while`
- `loop`

CLI contract:

- enable with `--enable-experimental <feature>`
- repeat the flag to enable more than one feature
- parser diagnostics for disabled use must emit a feature-gate error with code `P004`

### Deferred

- Unicode identifiers
- advanced numeric literal syntax beyond current decimal forms
- exact-width numeric storage and overflow semantics beyond current canonical ABI
- closure captures with environment objects
- `repeat/until`
- `foreach`
- `goto`
- `yield`
- raw strings and advanced literal modes
- production tensor syntax and static shape types
- native CUDA/ROCm/Metal/DirectML/Vulkan backends beyond the current optional `wgpu` baseline
- `.npz`, safetensors, checkpoints, and ONNX import/export beyond the current `.npy` baseline
- network package registry protocol, authentication, provenance signatures, and semver range solving beyond exact local versions

## Synchronization Rules

When a feature changes maturity:

1. update this file
2. update the user-facing reference docs
3. update examples if their required invocation changes
4. update CLI help or `--list-experimental` if the change affects experimental gating
5. add or adjust tests in `tests/validation`, `tests/errors`, `tests/cli`, or `examples`
