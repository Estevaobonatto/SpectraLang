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

- Status: `complete`
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

### Completed Implementation

- Added compiler AST and diagnostic snapshot tests in `compiler/tests/`.
- Added midend IR snapshot tests in `midend/tests/`.
- Added cargo-fuzz targets for parser, semantic analysis, full no-op pipeline,
  and lowering under `fuzz/fuzz_targets/`.
- Added the regression placement and snapshot/fuzz policy in
  `docs/testing-regression-policy.md`.
- Added `scripts/validate_test_pyramid.py` and wired it into `run_tests.ps1`.

### Validation

- `cargo test -p spectra-compiler --test snapshot_tests`
- `cargo test -p spectra-midend --test ir_snapshot_tests`
- `python scripts\validate_test_pyramid.py`
- `.\run_tests.ps1`

## R-105 Diagnostics Standardization

- Status: `complete`
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

### Completed Implementation

- `docs/diagnostics/error-code-reference.md` documents stable diagnostic
  families, at least 20 high-frequency diagnostics, JSON diagnostics, and SARIF
  diagnostics.
- `spectralang compile/check/lint --json` emits machine-readable diagnostics.
- `spectralang compile/check/lint --sarif` emits SARIF 2.1.0 diagnostics.
- `--json` and `--sarif` are mutually exclusive and preserve diagnostic exit
  code behavior.
- `scripts/validate_diagnostics_standardization.py` validates the reference and
  generated JSON/SARIF reports.
- `run_tests.ps1` runs R-105 validation as a gated check.

### Validation

- `cargo test -p spectra-cli`
- `python scripts\validate_diagnostics_standardization.py`
- `python scripts\validate_diagnostics_standardization.py --json-report target\r105-diagnostics\diagnostics.json --sarif-report target\r105-diagnostics\diagnostics.sarif`
- `.\run_tests.ps1`

## R-106 Experimental Feature Policy

- Status: `complete`
- Priority: `P1`
- Owner: `ecosystem`
- Dependencies: `R-101`, `R-105`

### Scope

- classify current features into stable, beta, experimental, deferred
- align docs and CLI behavior

### Acceptance

- language docs and CLI help match
- no feature remains undocumented in maturity level

### Completed Implementation

- `docs/language-feature-maturity.md` defines stable, beta, experimental, and
  deferred feature classes.
- `spectralang --list-experimental` returns the exact experimental feature set
  documented by the policy: `switch`, `unless`, `do-while`, and `loop`.
- `scripts/validate_feature_maturity.py` compares policy docs, CLI source, and
  CLI output.
- `run_tests.ps1` runs R-106 validation as a gated check.

### Validation

- `python scripts\validate_feature_maturity.py --binary target\debug\spectralang.exe`
- `.\run_tests.ps1`

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

- Status: `complete`
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

### Completed Implementation

- Tuple, struct, enum, and OR-pattern parsing is implemented.
- Semantic validation handles destructuring bindings and match exhaustiveness.
- Midend lowering handles the supported pattern forms.
- `tests/validation/31_tuple_variant_destructuring.spectra`,
  `tests/validation/60_pattern_control_surface.spectra`, and
  `tests/validation/63_destructuring_and_or_patterns.spectra` cover positive
  parser/semantic/lowering paths.
- `tests/errors/non_exhaustive_enum_match.spectra` covers the negative
  exhaustiveness path.
- `scripts/validate_pattern_ergonomics.py` validates source coverage plus
  positive/negative examples.
- `run_tests.ps1` runs R-203 validation as a gated check.

### Validation

- `python scripts\validate_pattern_ergonomics.py --binary target\debug\spectralang.exe`
- `.\run_tests.ps1`

## R-204 Closure Completion

- Status: `complete`
- Priority: `P1`
- Owner: `midend`
- Dependencies: `R-102`, `R-103`

### Scope

- closure capture model
- function values and invocation completion
- returning/storing closures

### Acceptance

- closures work outside parser/check-only scenarios
- storing, passing, indirect invocation, returning, and by-value captures are covered
- direct mutation of captured variables is rejected with a semantic diagnostic

### Implementation Notes

- Function values lower to runtime closure handles. Slot 0 stores the code pointer; later slots store captured values.
- Captures are by value in deterministic first-use order.
- `tests/validation/79_closure_captures.spectra` covers local capture, captured closure return, captured closure passing, nested capture, and stdlib HOF callbacks.
- `tests/errors/closure_capture_mutation.spectra` covers the by-value mutation restriction.

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

- Status: `complete`
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
- rename is covered by automated tests for definitions, uses, and identifier boundaries

### Completed so far

- `tools/spectra-lsp` advertises hover, go-to-definition, references, rename, completion, diagnostics, document/workspace symbols, formatting, inlay hints, quick fixes, and semantic tokens.
- `prepareRename` and `rename` are implemented.
- Rename uses semantic definition links when available and a bounded lexical block fallback when local symbols do not expose a definition span.
- `cargo test -p spectra-lsp` validates rename behavior.

## R-1002 Debugger and Stack Traces

- Status: `complete`
- Priority: `P2`
- Owner: `backend`
- Dependencies: `R-103`

### Scope

- source-aware stack traces
- AOT debug map strategy for native debugger workflows
- JIT introspection strategy

### Acceptance

- runtime failures produce actionable source-level traces
- AOT artifacts emit a source debug map that can be used with native symbols in gdb/lldb workflows

### Completed so far

- `spectralang run` now emits `error[runtime]` with source location and stack frame `0: main()` when a program exits with a non-zero status.
- `spectralang compile --emit-object` and `--emit-exe` write a sibling `.spectra-debug.json` sidecar with schema version, artifact path, source path, entrypoint span, exported symbol, and supported native debugger workflow.
- `scripts/validate_debugger_stack_traces.py` validates runtime stack diagnostics and AOT object debug map emission.
- `tests/cli/runtime_nonzero.spectra` and `run_tests.ps1` validate the runtime diagnostic path.

### Production Boundary

- Native DWARF/PDB emission is not claimed. The production-supported strategy for this item is native symbol debugging plus the checked-in Spectra source sidecar until backend-native debug sections are added as a future roadmap item.

## R-1003 Profiling and Benchmark Tooling

- Status: `complete`
- Priority: `P2`
- Owner: `tooling`
- Dependencies: `R-401`

### Scope

- `spectra bench`
- op-level timing
- perf regression tracking

### Acceptance

- benchmark suite exists and perf deltas are reportable
- `spectralang bench` emits machine-readable timing reports

### Completed so far

- `spectralang bench <paths>` compiles with pipeline timing metrics enabled.
- `--bench-json <path>` writes module-level and aggregate timing data as JSON.
- `spectralang package bench` uses the benchmark mode for package workspaces.
- `run_tests.ps1` validates `bench --bench-json`.

---

# Phase 11: Concurrency and Serving

## R-1101 Concurrency Model

- Status: `complete`
- Priority: `P1`
- Owner: `runtime`
- Dependencies: `R-402`

### Scope

- threads/tasks/channels
- synchronization primitives
- stdlib-only API through `std.concurrent`
- deterministic handle registry for task, channel, and counter resources
- parallel chunk execution for pipeline sums

### Acceptance

- parallel data pipeline sample works and is tested
- runtime unit tests cover task spawn/join, FIFO channels, counters, stats, reset, and parallel pipeline execution
- `tests/validation/77_concurrency_pipeline.spectra` passes in the integrated test runner

### Completed

- Added virtual module signatures for `std.concurrent`.
- Added runtime host functions for task handles, non-blocking FIFO channels, counters, stats, reset, and deterministic parallel pipeline sum.
- Added midend host-call descriptors so aliased module calls lower to runtime host calls instead of struct method calls.
- Validated through Rust unit tests and `run_tests.ps1`.

## R-1102 Inference Serving Foundations

- Status: `complete`
- Priority: `P2`
- Owner: `ml`
- Dependencies: `R-1101`, `R-702`

### Scope

- request batching
- warmup
- timeout/cancellation
- model residency controls
- local in-process serving queue through `std.serve`
- deterministic toy benchmark through `server_benchmark(server, requests, batch)`

### Acceptance

- toy inference server benchmark exists
- runtime unit tests cover warmup, batching, cancellation, pending queue state, request result lookup, and model residency
- `tests/validation/78_serving_foundations.spectra` passes in the integrated test runner

### Completed

- Added virtual module signatures for `std.serve`.
- Added runtime host functions for server handles, warmup, queueing, batching, cancellation, timeout state, resident model lookup, and deterministic benchmark processing.
- Validated through Rust unit tests and `run_tests.ps1`.

### Remaining Future Hardening

- Network transport, real HTTP/gRPC serving, async I/O, and external model residency policies are not part of this completed baseline and should be tracked as separate future work if required.

---

# Phase 12: Security and Operations

## R-1201 Build and Release Security

- Status: `complete`
- Priority: `P2`
- Owner: `ecosystem`
- Dependencies: `R-901`

### Scope

- checksums
- signatures
- SBOM
- dependency scanning
- release provenance
- automated evidence verification

### Acceptance

- release artifacts are signed and traceable
- dependency scanning is present in CI
- release evidence generation and verification are validated by `run_tests.ps1`

### Completed

- Added `scripts/release_security.py` to generate and verify release manifests,
  SHA-256 checksums, HMAC signatures, provenance, and CycloneDX-compatible SBOM.
- Updated `.github/workflows/release.yml` to require
  `SPECTRA_RELEASE_SIGNING_KEY`, generate evidence, verify it, and publish the
  evidence with release assets.
- Updated `.github/workflows/ci.yml` with `cargo audit` and high-severity
  `npm audit` dependency scanning.
- Added local validation coverage through `run_tests.ps1`.
- Added runtime host interop invariant checks and host invoke status coverage.

## R-1202 Stress and Soak Testing

- Status: `complete`
- Priority: `P1`
- Owner: `tooling`
- Dependencies: `R-104`, `R-402`, `R-503`

### Scope

- long-run compile stress
- tensor stress
- runtime soak tests
- JIT stress
- package workflow stress
- machine-readable stress reports

### Acceptance

- no crashes or unbounded leaks under defined stress runs
- stress report is emitted as JSON
- Phase 12 stress smoke is integrated into `run_tests.ps1`

### Completed

- Added `scripts/stress_soak.py` with compile, runtime/JIT, tensor/autodiff,
  concurrency/serving, and package workflow suites.
- Added timeout enforcement and optional RSS limit checks when process memory is
  observable.
- Added JSON stress report output.
- Added runtime invariant checks for host registry and manual allocation state.
- Validated the smoke profile through `run_tests.ps1`.

### Remaining Future Hardening

- Longer soak windows should run as scheduled/nightly jobs once CI budget and
  retention policy are defined.
- Public-key signing or Sigstore/cosign can replace or augment the current
  HMAC release evidence signature in a later security-hardening item.

---

# Phase 13: Documentation and Adoption

## R-1301 Spectra Book

- Status: `complete`
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

### Completed Implementation

- `docs/book/` now contains the adoption book covering language basics, numerics,
  tensors, autodiff, model authoring, deployment/export, stdlib/runtime/packages,
  and benchmark/comparison workflow.
- `scripts/validate_ai_book.py` verifies that required chapters exist and that
  every AI reference example is discoverable from the book.
- `run_tests.ps1` runs the Phase 13 book validation.

### Validation

- `python scripts\validate_ai_book.py`
- `.\run_tests.ps1`

## R-1302 AI Reference Examples

- Status: `complete`
- Priority: `P1`
- Owner: `ml`
- Dependencies: `R-602`, `R-603`

### Scope

- linear regression
- logistic regression
- MLP
- CNN
- toy transformer inference
- data preprocessing pipeline

### Acceptance

- at least 3 AI examples run end-to-end in automated environments

---

# Next Horizon: Complete AI/ML Development Platform

The baseline roadmap through Phase 13 is complete. The following phases define
the next tracked development cycle toward a broader AI/ML platform.

---

# Phase 14: AI Language Core

## R-1401 First-Class Tensor Language Constructs

- Status: `in_progress`
- Priority: `P0`
- Owner: `semantic`
- Dependencies: `R-204`, `R-303`, `R-403`, `R-503`

### Scope

- tensor literals
- tensor type annotations
- dtype/device/layout annotations
- compiler-visible tensor operation semantics
- compatibility layer for existing `std.tensor` handle API

### Acceptance

- tensor literals and annotations parse, type-check, lower, and run without relying on ad-hoc host-call syntax
- compiler diagnostics report dtype, rank, layout, and device mismatches with stable error codes
- existing `std.tensor` handle API remains compatible through a documented migration layer

### Completed so far

- `Tensor<dtype, rankN>` annotations are represented in the semantic type model and lower to handle-compatible IR.
- Explicitly typed rank1/rank2 float tensor literals compile and run through runtime tensor allocation.
- Rank and dtype mismatches on explicitly typed tensor bindings fail during semantic analysis.
- Existing `std.tensor` handle calls remain accepted through the handle compatibility layer.

### Remaining before completion

- Add device and layout annotations to the public tensor type syntax and diagnostics.
- Add stable diagnostic codes for tensor dtype/rank/layout/device errors.
- Document the migration layer from raw handle-style calls to first-class tensor syntax.

## R-1402 Shape and DType Type System

- Status: `in_progress`
- Priority: `P0`
- Owner: `semantic`
- Dependencies: `R-1401`, `R-202`

### Scope

- rank constraints
- static and dynamic dimensions
- dtype/layout/device constraints
- gradual fallback for runtime-dynamic shapes

### Acceptance

- rank, static dimension, dynamic dimension, dtype, and layout constraints are represented in the semantic type model
- shape errors are caught at check time for static cases and at runtime for dynamic cases
- at least one neural-network example uses static shape validation end-to-end

### Completed so far

- Static rank metadata and dtype metadata are represented for `Tensor<float, rankN>`.
- Rectangular rank2 tensor literal shape mismatches are rejected during semantic analysis.
- Tensor-returning `std.tensor` operations now expose compiler-visible Tensor return types for core autodiff paths.

### Remaining before completion

- Add static dimension values, dynamic dimension variables, layout, and device constraints to the semantic type model.
- Extend compile-time shape checks beyond literal rectangularity into operations such as `matmul`, `reshape`, and neural-network layers.
- Add an end-to-end neural-network example that relies on static shape validation.

## R-1403 Differentiable Language Blocks

- Status: `in_progress`
- Priority: `P1`
- Owner: `midend`
- Dependencies: `R-503`, `R-1402`

### Scope

- differentiable function/block syntax
- unsupported-op diagnostics
- lowering into autodiff/runtime or tensor graph representation

### Acceptance

- users can mark differentiable functions or blocks with documented syntax
- unsupported operations inside differentiable regions produce actionable diagnostics
- gradient tests cover scalar, tensor, control-flow, and nested-function cases

### Completed so far

- `diff { ... }` parses as a language-level differentiable block expression.
- The block result is lowered to `std.tensor.backward(loss)` and the loss value remains usable by the surrounding expression.
- Non-tensor differentiable block results produce an actionable semantic diagnostic.

### Remaining before completion

- Add diagnostics for unsupported operations inside otherwise tensor-returning differentiable regions.
- Add gradient tests for scalar, tensor, control-flow, and nested-function cases.
- Decide and implement differentiable function annotation syntax if block syntax alone is not sufficient for production authoring.

---

# Phase 15: Production Numerical Performance

## R-1501 Numerical Performance Benchmark Suite

- Status: `not_started`
- Priority: `P0`
- Owner: `numerics`
- Dependencies: `R-401`, `R-1003`

### Scope

- release-mode benchmark harness
- tensor creation, unary ops, reductions, matmul, convolution, autodiff, optimizer steps, data loading
- baseline storage and regression thresholds

### Acceptance

- benchmarks cover tensor creation, unary ops, reductions, matmul, convolution, autodiff, optimizer steps, and data loading
- release-mode benchmark output is machine-readable and compared against checked-in baselines
- CI can fail on configured correctness or performance regressions

## R-1502 Memory Planner and Tensor Lifetime Analysis

- Status: `not_started`
- Priority: `P0`
- Owner: `midend`
- Dependencies: `R-402`, `R-1401`

### Scope

- tensor lifetime metadata
- temporary buffer reuse
- memory-pressure diagnostics
- peak/reuse/allocation-site reporting

### Acceptance

- tensor temporaries have visible lifetime metadata in IR or runtime plans
- common training loops reuse buffers without unbounded allocation growth
- memory reports include peak bytes, reuse rate, allocation sites, and tensor lifetimes

## R-1503 Numerical Correctness and Determinism Certification

- Status: `not_started`
- Priority: `P1`
- Owner: `numerics`
- Dependencies: `R-403`, `R-1501`

### Scope

- deterministic RNG mode
- deterministic kernel validation
- float tolerance policy
- cross-platform validation artifacts

### Acceptance

- RNG, reductions, matmul, convolution, and optimizer kernels have deterministic test modes
- float tolerance policy is documented and enforced in tests
- Windows, Linux, and macOS results are compared through portable validation artifacts

---

# Phase 16: Accelerator and Graph Compilation

## R-1601 Tensor Graph IR

- Status: `not_started`
- Priority: `P0`
- Owner: `midend`
- Dependencies: `R-1401`, `R-1502`

### Scope

- graph-level tensor IR
- operator, shape, dtype, device, dependency metadata
- graph validation and stable dumps

### Acceptance

- tensor programs can lower to a graph IR with operators, shapes, dtypes, devices, and dependencies
- graph validation catches unsupported cycles, shape mismatches, and device-placement conflicts
- graph dumps are stable enough for snapshot tests

## R-1602 Graph Optimization and Fusion

- Status: `not_started`
- Priority: `P1`
- Owner: `midend`
- Dependencies: `R-1601`, `R-1501`

### Scope

- elementwise fusion
- constant/layout propagation
- memory-aware scheduling
- optimized vs unoptimized comparison

### Acceptance

- elementwise chains and reduction-adjacent operations fuse in validated cases
- optimization preserves numerical correctness within documented tolerances
- optimized and unoptimized graph execution can be compared in tests

## R-1603 Production GPU Backend

- Status: `not_started`
- Priority: `P0`
- Owner: `numerics`
- Dependencies: `R-702`, `R-1601`, `R-1503`

### Scope

- production accelerator execution for core ops
- CPU fallback
- device capability detection
- accelerator diagnostics

### Acceptance

- GPU execution supports tensor transfer, matmul, reductions, elementwise ops, convolution, and autodiff-required backward kernels
- CPU fallback remains available and produces equivalent results within tolerance
- device capability detection and error reporting are documented and tested

---

# Phase 17: Data and Experiment Platform

## R-1701 Dataset and DataFrame Runtime

- Status: `not_started`
- Priority: `P1`
- Owner: `runtime`
- Dependencies: `R-602`, `R-802`, `R-1101`

### Scope

- dataframe APIs
- CSV, JSONL, NPY, directory-backed datasets
- batching, shuffling, transforms, train/test split, deterministic seeding

### Acceptance

- CSV, JSONL, NPY, and directory-backed datasets can be loaded through stable APIs
- batching, shuffling, map/filter transforms, train/test split, and deterministic seeding are tested
- tabular preprocessing example trains end-to-end without Python glue

## R-1702 Experiment Tracking and Reproducibility

- Status: `not_started`
- Priority: `P1`
- Owner: `ml`
- Dependencies: `R-901`, `R-1701`

### Scope

- run manifests
- configs, metrics, artifacts, seeds, lockfiles, model outputs
- run comparison
- exact reproduction command

### Acceptance

- training runs emit a structured experiment manifest
- metrics and artifacts can be compared across runs
- a documented command reproduces a reference training result from lockfile and manifest

## R-1703 Distributed Training Foundations

- Status: `not_started`
- Priority: `P2`
- Owner: `runtime`
- Dependencies: `R-1101`, `R-1603`, `R-1702`

### Scope

- single-machine multi-worker training simulation
- checkpoint coordination
- resume after interruption
- documented topology and non-goals

### Acceptance

- single-machine multi-worker training simulation is covered by tests
- checkpoint save/resume works across worker interruption
- distributed behavior is documented with explicit non-goals and supported topology

---

# Phase 18: Model Ecosystem and LLM Workloads

## R-1801 ONNX Import and Export

- Status: `not_started`
- Priority: `P0`
- Owner: `ecosystem`
- Dependencies: `R-803`, `R-1601`

### Scope

- ONNX export subset
- ONNX import subset
- shape/dtype/operator validation
- external runtime validation

### Acceptance

- Spectra models can export a supported ONNX subset with shapes and dtypes
- supported ONNX models can import into Spectra graph/runtime representation
- round-trip tests cover linear, convolutional, activation, normalization, and simple transformer blocks

## R-1802 Transformer and LLM Runtime Primitives

- Status: `not_started`
- Priority: `P0`
- Owner: `ml`
- Dependencies: `R-1603`, `R-1801`

### Scope

- attention
- layer norm
- embedding lookup
- positional encoding
- GELU/SwiGLU
- KV cache
- logits sampling

### Acceptance

- attention, layer norm, embedding lookup, positional encoding, GELU/SwiGLU, KV cache, and logits sampling are implemented and tested
- toy transformer example uses real runtime primitives rather than placeholder math
- CPU fallback and accelerator path produce equivalent outputs within tolerance

## R-1803 Tokenization, Embeddings, and RAG Toolkit

- Status: `not_started`
- Priority: `P1`
- Owner: `ml`
- Dependencies: `R-1701`, `R-1802`

### Scope

- BPE or WordPiece-style tokenization
- embedding utilities
- vector index APIs
- chunking, retrieval, prompt assembly, RAG evaluation

### Acceptance

- BPE or WordPiece-style tokenization is implemented with deterministic encoding/decoding
- vector index APIs support insert, query, persist, and load
- RAG example runs retrieval, prompt assembly, model call boundary, and evaluation metrics end-to-end

---

# Phase 19: AI Operations and Evaluation

## R-1901 Model Evaluation and Metrics Suite

- Status: `not_started`
- Priority: `P1`
- Owner: `ml`
- Dependencies: `R-1702`, `R-1802`

### Scope

- classification metrics
- regression metrics
- ranking/generation metrics
- serving latency and throughput metrics

### Acceptance

- metrics include accuracy, precision, recall, F1, ROC-AUC baseline, MSE, MAE, perplexity, latency, and throughput
- evaluation reports are machine-readable and human-readable
- reference examples include evaluation gates before model export or serving

## R-1902 AI Safety and Guardrail Runtime

- Status: `not_started`
- Priority: `P2`
- Owner: `runtime`
- Dependencies: `R-1102`, `R-1803`, `R-1901`

### Scope

- input/output policy hooks
- output validation
- rate limiting
- audit logs
- safe fallback behavior

### Acceptance

- serving APIs can attach input and output policy hooks
- guardrail failures produce structured diagnostics and audit events
- safety examples cover blocked input, blocked output, and degraded fallback behavior

## R-1903 Model Monitoring and Drift Detection

- Status: `not_started`
- Priority: `P2`
- Owner: `runtime`
- Dependencies: `R-1102`, `R-1702`, `R-1901`

### Scope

- inference metrics
- input distribution summaries
- drift checks
- observability JSON export

### Acceptance

- serving runtime emits request, latency, error, and model-version metrics
- drift checks compare live distribution summaries against reference baselines
- monitoring artifacts are exportable as JSON for external observability systems

---

# Phase 20: Production Certification

## R-2001 AI Conformance Suite

- Status: `not_started`
- Priority: `P0`
- Owner: `tooling`
- Dependencies: `R-1402`, `R-1503`, `R-1801`, `R-1901`

### Scope

- compiler conformance
- runtime/tensor/autodiff/graph conformance
- interop/package/serving conformance
- docs-example conformance
- versioned certification reports

### Acceptance

- conformance tests cover compiler, runtime, tensors, autodiff, graph, interop, package, serving, and docs examples
- the suite emits a versioned certification report
- release candidates cannot be certified while conformance tests fail

## R-2002 Production Release Channels

- Status: `not_started`
- Priority: `P1`
- Owner: `ecosystem`
- Dependencies: `R-1201`, `R-2001`

### Scope

- nightly channel
- beta channel
- stable channel
- compatibility and deprecation policy
- CLI/package channel metadata

### Acceptance

- release channel policy is documented
- CLI and package metadata report channel and compatibility level
- deprecation warnings and migration guidance are tested

### Completed Implementation

- `examples/ai/linear_regression_train_export.spectra`
- `examples/ai/logistic_regression_train_export.spectra`
- `examples/ai/mlp_training_serving.spectra`
- `examples/ai/cnn_image_classifier.spectra`
- `examples/ai/toy_transformer_inference.spectra`
- `examples/ai/data_preprocessing_pipeline.spectra`
- `scripts/ai_examples_benchmark.py` emits a JSON timing report for all Phase 13
  AI examples.
- `run_tests.ps1` executes all six examples as gated Phase 13 checks.

### Validation

- `python scripts\ai_examples_benchmark.py --out target\ai-examples\benchmark.json --timeout-seconds 20`
- `.\run_tests.ps1`

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
