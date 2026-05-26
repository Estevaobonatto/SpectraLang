# SpectraLang Roadmap Backlog

## Purpose

This backlog converts the production AI implementation plan into executable work packages.

It is designed for:

- sprint planning
- issue creation
- milestone tracking
- architecture review
- acceptance-based delivery

This file is human-oriented.
The machine-oriented counterpart is [roadmap/roadmap.toml](/D:/Lang/SpectraLang/roadmap/roadmap.toml).

---

## Status Legend

| Status | Meaning |
|---|---|
| `not_started` | Work has not begun |
| `in_progress` | Work is active |
| `blocked` | Work cannot continue due to unmet dependency or design blocker |
| `complete` | Work finished and accepted |

## Priority Legend

| Priority | Meaning |
|---|---|
| `P0` | Foundational blocker for the roadmap |
| `P1` | High-value next step |
| `P2` | Important but can follow core delivery |
| `P3` | Nice-to-have or late-stage maturity item |

## Owner Groups

| Owner | Scope |
|---|---|
| `frontend` | lexer, parser, AST, diagnostics |
| `semantic` | type system, imports, traits, validation |
| `midend` | IR lowering, optimization, validation |
| `backend` | Cranelift, object emission, targets |
| `runtime` | runtime services, allocators, stdlib host calls |
| `numerics` | tensor core, kernels, BLAS/GPU integration |
| `ml` | autodiff, modules, optimizers, datasets |
| `tooling` | CLI, formatter, lint, LSP, debugger |
| `ecosystem` | package manager, registry, interop, docs |

---

# Phase 0: Governance and Execution

## R-001 ADR Foundation

- Status: `complete`
- Priority: `P0`
- Owner: `ecosystem`
- Dependencies: none

### Scope

- Create `docs/adr/`
- Add ADR templates
- Write initial ADRs for:
  - memory model
  - tensor design direction
  - autodiff execution model
  - GPU backend strategy
  - package manager scope

### Acceptance

- `docs/adr/` exists
- at least 5 ADRs are committed
- every major subsystem references an ADR or states pending ADR explicitly

## R-002 Ownership Map

- Status: `complete`
- Priority: `P0`
- Owner: `ecosystem`
- Dependencies: `R-001`

### Scope

- Define code ownership by subsystem
- Document review requirements for cross-cutting changes
- Add escalation path for architecture conflicts

### Acceptance

- ownership document exists
- every top-level workspace crate has a primary owner group

## R-003 Roadmap Reporting Script

- Status: `complete`
- Priority: `P1`
- Owner: `tooling`
- Dependencies: none

### Scope

- Add a script that reads `roadmap/roadmap.toml`
- Emit:
  - Markdown summary
  - status counts
  - dependency readiness report

### Acceptance

- script exists under `tools/` or `scripts/`
- script validates roadmap structure
- script outputs grouped report by phase

---

# Phase 1: Compiler Productionization

## R-101 Frontend Coverage Audit

- Status: `complete`
- Priority: `P0`
- Owner: `frontend`
- Dependencies: `R-001`

### Scope

- Audit lexer coverage vs docs
- Audit parser coverage vs docs
- Audit syntax recovery paths
- Identify all unsupported but documented forms

### Acceptance

- audit document exists
- every syntax form is labeled as supported, gated, partial, or deferred

## R-102 Semantic Coverage Audit

- Status: `complete`
- Priority: `P0`
- Owner: `semantic`
- Dependencies: `R-101`

### Scope

- Map every AST expression and statement kind to semantic handling
- Identify partial validation zones
- Identify missing invariants and weak diagnostics

### Acceptance

- semantic coverage matrix exists
- no AST kind remains unclassified

## R-103 Lowering and Backend Coverage Audit

- Status: `complete`
- Priority: `P0`
- Owner: `midend`
- Dependencies: `R-102`

### Scope

- Map every AST construct to lowering path
- Map every IR instruction to backend coverage
- Identify mismatch between type inference and codegen assumptions

### Acceptance

- lowering/backend coverage matrix exists
- all unsupported constructs are tracked as backlog items

## R-104 Compiler Test Pyramid

- Status: `not_started`
- Priority: `P0`
- Owner: `tooling`
- Dependencies: `R-101`, `R-102`, `R-103`

### Scope

- Add unit tests per stage
- Add AST/IR/diagnostic snapshots
- Add regression suite policy
- Add parser and semantic fuzz targets

### Acceptance

- each compiler crate has stage-local tests
- fuzz targets exist
- regression policy documented

## R-105 Diagnostics Standardization

- Status: `not_started`
- Priority: `P1`
- Owner: `frontend`
- Dependencies: `R-102`

### Scope

- stable diagnostic codes
- JSON and SARIF output
- better hints for common failures

### Acceptance

- stable error code table committed
- JSON diagnostics usable by tooling
- at least 20 top diagnostics include actionable hints

## R-106 Experimental Feature Policy

- Status: `not_started`
- Priority: `P1`
- Owner: `ecosystem`
- Dependencies: `R-101`, `R-105`

### Scope

- classify current features into stable, beta, experimental, deferred
- align docs and CLI behavior

### Acceptance

- language docs and CLI help match
- no feature remains undocumented in maturity level

---

# Phase 2: Scientific Type System

## R-201 Numeric Type Expansion

- Status: `complete`
- Priority: `P0`
- Owner: `semantic`
- Dependencies: `R-103`

### Scope

- add signed integer families
- add unsigned integer families
- add `f32`, `f64`, `f16`, `bf16`
- define promotions and casts
- backend support for all implemented primitives

### Acceptance

- alpha numeric aliases are implemented end-to-end over the current canonical `int`/`float` ABI
- invalid conversions are rejected deterministically
- tests cover arithmetic, casts, and current ABI representation

### Implementation Notes

- `i8`, `i16`, `i32`, `i64`, `isize`, `u8`, `u16`, `u32`, `u64`, and `usize` currently canonicalize to `int`.
- `f16`, `bf16`, `f32`, and `f64` currently canonicalize to `float`.
- Exact-width storage and overflow semantics remain future runtime/backend work before production AI/ML numerics.

## R-202 Const Evaluation Engine

- Status: `complete`
- Priority: `P1`
- Owner: `semantic`
- Dependencies: `R-201`

### Scope

- compile-time numeric expression evaluation
- shape- and size-related const contexts

### Acceptance

- const expressions are usable in declared top-level `const` contexts
- failures produce targeted diagnostics for non-const initializers

### Implementation Notes

- Supported const expressions: primitive literals, references to previous constants, grouping, unary operators, binary arithmetic/comparison/logical operators, string concatenation, and valid casts.
- Shape/size const contexts remain future tensor/type-system work.

## R-203 Destructuring and Pattern Ergonomics

- Status: `in_progress`
- Priority: `P2`
- Owner: `frontend`
- Dependencies: `R-102`

### Scope

- tuple destructuring
- struct destructuring
- enum destructuring in `let`
- OR-patterns

### Acceptance

- syntax, semantics, lowering, and tests all implemented

## R-204 Closure Completion

- Status: `partial_alpha`
- Priority: `P1`
- Owner: `midend`
- Dependencies: `R-102`, `R-103`

### Scope

- closure capture model
- function values and invocation completion
- returning/storing closures

### Acceptance

- non-capturing closures work outside parser/check-only scenarios
- storing, passing, indirect invocation, and returning non-capturing closures are covered

### Implementation Notes

- Closure captures are not production-ready yet because the IR still represents closures as function values without an environment object.
- Capturing closures must remain documented as deferred until an explicit environment/capture ABI is implemented.

---

# Phase 3: Tensor Core

## R-301 Tensor Type Design

- Status: `in_progress`
- Priority: `P0`
- Owner: `numerics`
- Dependencies: `R-201`, `R-202`

### Scope

- define tensor API and metadata
- define ownership and view model
- define dtype/device/layout model

### Acceptance

- tensor ADR is approved
- prototype tensor API compiles in examples/tests

### Implementation Notes

- Completed: ADR [0001](adr/0001-tensor-runtime-contract.md) accepts the current production tensor contract for the compiler architecture: `std.tensor` exports public `Tensor` metadata and uses opaque runtime handles with dtype, shape, strides, layout, CPU host device, and safe view semantics.
- Future `Tensor<T, Shape>` syntax remains a later type-system workstream and is not part of the Phase 3 completion gate.

## R-302 Tensor Runtime Representation

- Status: `complete`
- Priority: `P0`
- Owner: `runtime`
- Dependencies: `R-301`

### Scope

- tensor header
- storage abstraction
- shape/stride validation
- view semantics

### Acceptance

- runtime allocation and destruction tests pass
- view semantics are validated for correctness and safety

### Implementation Notes

- Completed: tensors store dtype, shape, strides, layout, shared storage, and base offset. `reshape`, contiguous `flatten`, `transpose`, `permute`, and `slice` create safe shared-storage views where possible.
- Completed: `set` and `set2` use copy-on-write when storage is shared, so views cannot corrupt aliased tensors. Runtime tests validate view lifetime after freeing a base handle and mutation isolation.
- Completed: explicit `free`/`free_all`, allocation metrics, buffer pool reuse, and active byte accounting remain integrated with the Phase 4 allocator work.

## R-303 Tensor Operations MVP

- Status: `complete`
- Priority: `P0`
- Owner: `numerics`
- Dependencies: `R-302`

### Scope

- creation ops
- reshape/transpose/flatten/slice/concat/stack
- elementwise arithmetic
- reductions
- matmul

### Acceptance

- core ops have shape and numeric correctness tests
- CPU benchmark harness exists

### Implementation Notes

- Completed: creation, metadata, reshape, flatten, permute, transpose, slice, concat, stack, elementwise arithmetic, unary kernels, reductions, argmax, dot, 2D matmul, batched matmul, RNG fills, and metrics are available through `std.tensor`.
- Completed: Rust runtime tests cover numeric correctness and shape behavior; `tests/validation/70_tensor_phase3_production.spectra` validates the public `.spectra` API; the Phase 4 benchmark harness provides CPU kernel coverage.

## R-304 Shape System

- Status: `complete`
- Priority: `P1`
- Owner: `semantic`
- Dependencies: `R-303`

### Scope

- rank/axis validation
- broadcast validation
- invalid reshape diagnostics

### Acceptance

- invalid shape operations fail with specific diagnostics
- rank and axis validation are enforced consistently

### Implementation Notes

- Completed: rank, axis, slice bounds, reshape size, concat/stack compatibility, matmul compatibility, and batched matmul compatibility are enforced consistently at runtime with deterministic host status codes.
- Broadcast-specific diagnostics and static shape typing remain future work tied to the later typed tensor syntax, not Phase 3 completion.

---

# Phase 4: Numerical Runtime and Kernels

## R-401 CPU Kernel Library

- Status: `implemented_alpha`
- Priority: `P0`
- Owner: `numerics`
- Dependencies: `R-303`

### Scope

- scalar kernels
- vectorized kernels
- BLAS integration strategy

### Acceptance

- core tensor ops match or outperform naive scalar reference implementations in release benchmarks
- reproducible perf benchmarks exist

### Implementation Notes

- Completed: portable production kernels for unary numeric ops, float activations, transpose, dot, elementwise, reductions, and matmul.
- Release benchmark evidence is checked in at `docs/performance/tensor-phase4-benchmark.md` and generated by `runtime/examples/tensor_phase4_bench.rs`.
- SIMD/BLAS policy: default Windows build uses portable kernels; native BLAS/LAPACK is not required by default; `blas` is an opt-in Cargo feature hook; AVX-512 is rejected for the current production baseline due target portability, with release benchmark evidence covering the accepted portable path.

## R-402 Tensor Allocator and Buffer Pool

- Status: `complete`
- Priority: `P1`
- Owner: `runtime`
- Dependencies: `R-302`, `R-401`

### Scope

- alignment guarantees
- scratch buffer reuse
- allocation metrics

### Acceptance

- allocation churn drops on repeated workloads
- memory metrics are exposed in tests/benchmarks

### Implementation Notes

- Completed: `std.tensor` keeps a runtime buffer pool for released tensor data and exposes allocation, active tensor, active byte, peak byte, pool hit/miss, reused buffer, scratch reuse, kernel op, and kernel element metrics.
- Release benchmark gate observes pool hits, pool misses, and scratch reuse.

## R-403 RNG and Statistical Primitives

- Status: `complete`
- Priority: `P2`
- Owner: `numerics`
- Dependencies: `R-401`

### Scope

- deterministic RNG
- uniform/normal/Bernoulli/categorical
- tensor random fills

### Acceptance

- seeding is reproducible
- distribution tests pass sanity checks

### Implementation Notes

- Completed: tensor RNG APIs `seed`, `uniform`, `uniform_f`, `normal_f`, `bernoulli`, and `categorical`.
- Runtime tests validate deterministic seeding and basic sanity bounds for uniform, Bernoulli, normal, and categorical paths.

---

# Phase 5: Autodiff

## R-501 Reverse-Mode Autodiff Core

- Status: `complete`
- Priority: `P0`
- Owner: `ml`
- Dependencies: `R-303`

### Scope

- computation graph
- `requires_grad`
- backward pass
- gradient storage

### Acceptance

- analytical gradient tests pass
- scalar loss backward works end-to-end

### Implementation Notes

- Completed: ADR [0002](adr/0002-autodiff-runtime-contract.md) accepts eager reverse-mode autodiff through the current `std.tensor` handle runtime.
- Completed: float tensors support `requires_grad`, scalar tensor `backward`, accumulated `grad`, and `zero_grad`.
- Completed: Rust tests and `tests/validation/71_tensor_phase5_autodiff.spectra` cover end-to-end scalar loss backward.

## R-502 Gradient Rules

- Status: `complete`
- Priority: `P0`
- Owner: `ml`
- Dependencies: `R-501`

### Scope

- gradient rules for elementwise, reduction, matmul, transpose, activations
- broadcast-aware gradient handling

### Acceptance

- finite-difference checks pass on all supported ops

### Implementation Notes

- Completed: gradient rules exist for elementwise add/sub/mul/div, unary neg/relu/exp/log/sqrt/sigmoid/tanh, tensor reductions `sum_t`/`mean_t`, `matmul`, `transpose`, `dot_t`, and reshape/flatten view edges.
- Completed: analytical and finite-difference tests cover the supported operation set.
- Broadcast-aware gradient reduction remains future work because production broadcasted tensor operations are not yet part of `std.tensor`.

## R-503 Graph Lifetime and Inference Mode

- Status: `complete`
- Priority: `P1`
- Owner: `runtime`
- Dependencies: `R-501`

### Scope

- graph release policy
- `no_grad` / inference mode
- checkpointing strategy

### Acceptance

- repeated training iterations do not show graph retention leaks

### Implementation Notes

- Completed: graph creator nodes are released after `backward` by default and exposed through `stats_graph_nodes`.
- Completed: `set_grad_enabled(false)` / `grad_enabled()` provide inference/no-grad mode and prevent graph construction overhead.
- Completed: tests verify graph node count returns to zero and no gradient is created while grad mode is disabled.

---

# Phase 6: ML Framework Layer

## R-601 Module and Layer System

- Status: `complete`
- Priority: `P0`
- Owner: `ml`
- Dependencies: `R-502`

### Scope

- module abstraction
- parameter registration
- base layers

### Acceptance

- MLP and CNN examples train end-to-end

### Implementation Notes

- Completed: ADR [0003](adr/0003-ml-framework-runtime-contract.md) accepts `std.ml` as the Phase 6 runtime-backed ML framework layer.
- Completed: module handles support parameter registration and training/eval mode.
- Completed: differentiable `linear` and `conv2d` layers integrate with `std.tensor` autograd; dropout and max pooling are available for model code.
- Completed: Rust tests verify MLP and CNN convergence; Spectra examples `72_ml_phase6_mlp_training.spectra` and `73_ml_phase6_cnn_training.spectra` compile and run.

## R-602 Losses and Optimizers

- Status: `complete`
- Priority: `P0`
- Owner: `ml`
- Dependencies: `R-601`

### Scope

- MSE, BCE, cross entropy
- SGD, Adam, AdamW
- LR scheduling

### Acceptance

- toy models converge on standard examples

### Implementation Notes

- Completed: losses `mse_loss`, `bce_loss`, `cross_entropy_loss`, and `nll_loss` produce scalar tensor losses for autodiff.
- Completed: optimizers `sgd_step`, `sgd_momentum_step`, `adam_step`, and `adamw_step` update parameters in place from accumulated gradients.
- Completed: `exp_lr` provides baseline exponential learning-rate scheduling.
- Completed: runtime convergence tests validate the MLP and convolutional toy models.

## R-603 Dataset and Dataloader APIs

- Status: `complete`
- Priority: `P1`
- Owner: `ml`
- Dependencies: `R-601`

### Scope

- dataset abstraction
- batching
- shuffling
- prefetching
- simple data readers

### Acceptance

- minibatch training loop works on real sample datasets

### Implementation Notes

- Completed: tensor-backed datasets and dataloaders support length checks, batch counts, reproducible shuffling, feature batches, and label batches.
- Completed: Phase 6 runtime tests exercise minibatch access through `dataset_from_tensors` and `dataloader_*`.
- Future work: CSV/image-folder/JSONL readers and parallel prefetch remain planned for richer data ingestion beyond the production baseline.

---

# Phase 7: Acceleration

## R-701 Device Abstraction

- Status: `complete`
- Priority: `P0`
- Owner: `runtime`
- Dependencies: `R-302`

### Scope

- CPU/GPU device model
- placement and transfer semantics

### Acceptance

- tensors can be created and moved across supported devices

### Completed

- ADR [0004](adr/0004-device-runtime-contract.md) defines the production device contract.
- CPU is the supported production device in the default build (`0`).
- `std.tensor` exposes `device`, `device_available`, `to_device`, `cpu`, `sync`, and `stats_device_transfers`.
- Unsupported accelerator codes fail fast instead of silently falling back.
- Runtime and Spectra validation cover CPU placement, CPU transfer, synchronization, metrics, invalid device codes, and unavailable accelerators.

## R-702 GPU Backend MVP

- Status: `complete`
- Priority: `P0`
- Owner: `numerics`
- Dependencies: `R-701`, `R-401`

### Scope

- one production-grade accelerator backend
- elementwise/reduction/matmul support

### Acceptance

- same program semantics on CPU and GPU
- GPU benchmark records CPU/GPU timings on supported hardware
- no speedup requirement for this baseline; correctness is the completion gate

### Completed

- Optional Cargo feature `gpu` enables a real `wgpu` compute backend.
- Device code `6` is the `wgpu` accelerator backend; it is available only when the feature is enabled and an adapter is detected.
- Float tensor kernels are implemented for elementwise arithmetic, `relu`, `sum_f`, `matmul`, and `ml.conv2d`.
- CLI feature forwarding is available through `spectra-cli --features gpu`.
- `tests/validation/75_tensor_phase7_gpu.spectra` validates semantic parity when GPU is available and skips safely in default builds.
- `runtime/examples/tensor_phase7_gpu_bench.rs` records CPU/GPU timings and semantic parity on supported hardware.

## R-703 Mixed Precision

- Status: `complete`
- Priority: `P1`
- Owner: `ml`
- Dependencies: `R-702`

### Scope

- `f16`/`bf16`
- autocast or explicit mixed precision
- loss scaling

### Acceptance

- mixed precision training example converges on supported hardware

### Completed

- `std.tensor.precision(handle)` exposes precision metadata.
- `std.tensor.to_precision(handle, code)` supports `0` f64, `1` f32, `2` f16, and `3` bf16 quantization for float tensors.
- `std.ml.unscale_grad(parameter, scale)` supports loss-scaling workflows.
- `tests/validation/76_mixed_precision_training.spectra` validates a converging mixed-precision training loop with loss scaling and gradient unscale.

---

# Phase 8: Interoperability

## R-801 Python Interop

- Status: `complete`
- Priority: `P0`
- Owner: `ecosystem`
- Dependencies: `R-303`, `R-602`

### Scope

- call Spectra from Python
- tensor exchange with NumPy
- optional PyTorch interop

### Acceptance

- `python/demo_phase8.py` calls Spectra through the CLI/JIT boundary.
- NumPy `.npy` tensor exchange round-trips f64 data.

### Completed so far

- `python/spectra_bridge.py` provides `run_spectra_main`, NumPy `.npy` read/write helpers, and a ctypes wrapper for the native interop ABI.
- `python/demo_phase8.py` validates calling Spectra and exchanging tensor data with NumPy.
- `docs/interop.md` documents the Python bridge contract and validation commands.

## R-802 C and Rust FFI

- Status: `complete`
- Priority: `P1`
- Owner: `ecosystem`
- Dependencies: `R-701`

### Scope

- stable C ABI
- Rust helper crate
- headers/bindings generation

### Acceptance

- Rust sample compiles and runs against Spectra interop exports.
- C ABI header and sample exist.
- C sample compiles and runs against Spectra interop exports with LLVM `clang`.

### Completed so far

- `tools/spectra-interop` defines a `cdylib`/`rlib` interop crate.
- `tools/spectra-interop/include/spectra_interop.h` defines the stable C ABI surface.
- `tools/spectra-interop/examples/rust_ffi_sample.rs` compiles and runs locally.
- `tools/spectra-interop/examples/c_ffi_sample.c` is checked in and uses the same ABI surface.
- Rust unit tests validate the safe helper API and C ABI `.npy` round-trip in-process.
- LLVM `clang` was installed through `winget` and validated against `target/release/spectra_interop.dll.lib`.
- `run_tests.ps1` now compiles and executes `c_ffi_sample.exe` when a supported C compiler is available.

## R-803 Model and Data Formats

- Status: `complete`
- Priority: `P1`
- Owner: `ecosystem`
- Dependencies: `R-801`

### Scope

- ONNX
- `.npy` / `.npz`
- safetensors
- checkpoints

### Acceptance

- NumPy `.npy` v1.0 little-endian f64 arrays round-trip correctly.

### Completed so far

- `spectra-interop` implements `.npy` v1.0 read/write for one-dimensional little-endian f64 arrays.
- Rust helper tests, C ABI tests, Rust sample, and Python demo cover round-trip behavior.
- Broader formats such as `.npz`, safetensors, checkpoints, and ONNX remain future work and are not claimed as complete in this item.

---

# Phase 9: Package Manager and Registry

## R-901 Package Manager MVP

- Status: `complete`
- Priority: `P0`
- Owner: `tooling`
- Dependencies: `R-003`

### Scope

- dependency resolver
- lockfile
- workspace support
- package commands

### Acceptance

- multi-package workspace builds reproducibly
- lockfile guarantees deterministic resolution
- exact semver package versions are validated
- package commands are available for `lock`, `build`, `check`, `run`, `test`, `bench`, `doc`, `add`, and `update`

### Completed so far

- `tools/spectra-cli/src/package.rs` implements manifest loading, workspace resolution, local path dependency resolution, deterministic `spectra.lock` generation, local registry publishing/install, and package documentation generation.
- Package manifests and dependency versions validate exact semver `MAJOR.MINOR.PATCH` with optional prerelease suffixes.
- `spectralang package lock/build/check/run/test/bench/doc/add/update` are wired into the CLI.
- Normal `spectralang compile <project-dir>` includes dependency sources for multi-package manifests.
- `tests/projects/valid/package_workspace` validates a reproducible multi-package workspace with a path dependency.
- `run_tests.ps1` validates lock/build/check/doc package commands.

## R-902 Registry MVP

- Status: `complete`
- Priority: `P1`
- Owner: `ecosystem`
- Dependencies: `R-901`

### Scope

- publish/install flow
- integrity validation
- semver compatibility

### Acceptance

- package can be published and consumed from a local registry instance
- artifact integrity is validated before install

### Completed so far

- `spectralang package publish --registry <path>` publishes the root package into a local filesystem registry.
- Published packages include registry metadata with checksum.
- `spectralang package add <name> --registry <path> --version <version>` validates checksum before installing into `.spectra/packages`.
- `run_tests.ps1` validates publish, registry add, and building a registry consumer.

### Future hardening

- Network registry protocol, authentication, provenance signatures, semver range solving, and private registry policy remain future work beyond the completed local registry MVP.

---

# Phase 10: Tooling Maturity

## R-1001 LSP Completion

- Status: `not_started`
- Priority: `P1`
- Owner: `tooling`
- Dependencies: `R-105`, `R-901`

### Scope

- hover
- definitions
- references
- rename
- completion
- semantic tokens

### Acceptance

- editor workflow supports daily coding in a non-trivial Spectra workspace

## R-1002 Debugger and Stack Traces

- Status: `not_started`
- Priority: `P2`
- Owner: `backend`
- Dependencies: `R-103`

### Scope

- source-aware stack traces
- AOT debug info strategy
- JIT introspection strategy

### Acceptance

- runtime failures produce actionable source-level traces

## R-1003 Profiling and Benchmark Tooling

- Status: `not_started`
- Priority: `P2`
- Owner: `tooling`
- Dependencies: `R-401`

### Scope

- `spectra bench`
- op-level timing
- perf regression tracking

### Acceptance

- benchmark suite exists and perf deltas are reportable

---

# Phase 11: Concurrency and Serving

## R-1101 Concurrency Model

- Status: `not_started`
- Priority: `P1`
- Owner: `runtime`
- Dependencies: `R-402`

### Scope

- threads/tasks/channels
- synchronization primitives

### Acceptance

- parallel data pipeline sample works and is tested

## R-1102 Inference Serving Foundations

- Status: `not_started`
- Priority: `P2`
- Owner: `ml`
- Dependencies: `R-1101`, `R-702`

### Scope

- request batching
- warmup
- timeout/cancellation
- model residency controls

### Acceptance

- toy inference server benchmark exists

---

# Phase 12: Security and Operations

## R-1201 Build and Release Security

- Status: `not_started`
- Priority: `P2`
- Owner: `ecosystem`
- Dependencies: `R-901`

### Scope

- checksums
- signatures
- SBOM
- dependency scanning

### Acceptance

- release artifacts are signed and traceable

## R-1202 Stress and Soak Testing

- Status: `not_started`
- Priority: `P1`
- Owner: `tooling`
- Dependencies: `R-104`, `R-402`, `R-503`

### Scope

- long-run compile stress
- tensor stress
- runtime soak tests
- JIT stress

### Acceptance

- no crashes or unbounded leaks under defined stress runs

---

# Phase 13: Documentation and Adoption

## R-1301 Spectra Book

- Status: `not_started`
- Priority: `P1`
- Owner: `ecosystem`
- Dependencies: `R-106`, `R-303`, `R-602`

### Scope

- language guide
- numerics guide
- tensor guide
- autodiff guide
- ML tutorial path

### Acceptance

- user can train a toy model using docs alone

## R-1302 AI Reference Examples

- Status: `not_started`
- Priority: `P1`
- Owner: `ml`
- Dependencies: `R-602`, `R-603`

### Scope

- linear regression
- logistic regression
- MLP
- CNN
- toy transformer inference

### Acceptance

- at least 3 AI examples run end-to-end in automated environments

---

## Recommended First Execution Slice

If implementation starts immediately, the recommended first sequence is:

1. `R-001`
2. `R-003`
3. `R-101`
4. `R-102`
5. `R-103`
6. `R-104`
7. `R-105`
8. `R-106`
9. `R-201`
10. `R-301`

This sequence establishes:

- governance
- reporting
- coverage visibility
- compiler confidence
- the first real foundation for AI workloads
