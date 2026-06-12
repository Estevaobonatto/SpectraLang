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

## R-107 Struct Literal Shorthand Contract

- Status: `complete`
- Priority: `P2`
- Owner: `frontend`
- Dependencies: `R-105`, `R-203`

### Problem Found

During the advanced regression-test expansion, a candidate test using
`Boxed { value }` failed to parse. The language now supports this shorthand as
the production contract: `Type { field }` is equivalent to
`Type { field: field }` and resolves the right-hand side through normal local
binding lookup.

The parser lookahead is intentionally conservative so block-like constructs such
as `match value { ... }` are not misclassified as struct literals.

### Scope

- support explicit and shorthand struct literal fields in the parser
- lower shorthand fields to identifier expressions using the same local binding semantics as `field: field`
- keep `match value { ... }` and other block-like expressions parsed correctly
- align docs and examples with the chosen contract

### Acceptance

- `Type { field }` shorthand is supported and documented as equivalent to `Type { field: field }`
- parser and semantic regression tests cover accepted explicit field syntax and shorthand behavior
- undefined shorthand bindings fail with the normal stable undefined-variable diagnostic
- language reference and examples do not imply unsupported struct literal syntax

### Evidence

- Parser regression tests cover `Point { x, y: 2 }` and ensure `match value { ... }` is not treated as a struct literal.
- Positive validation test: `tests/validation/104_nested_scope_shadowing_pattern_stress.spectra` uses `Boxed { value }`.
- Negative validation test: `tests/errors/struct_literal_shorthand_undefined_binding.spectra` verifies missing shorthand bindings fail semantically.
- Focused validation: `cargo test -p spectra-compiler`; `spectralang compile` for the positive and negative examples.

## R-108 Diagnostic Classification Hardening

- Status: `complete`
- Priority: `P1`
- Owner: `frontend`
- Dependencies: `R-105`, `R-203`

### Problems Found

- `tests/errors/trait_bound_missing_method_stress.spectra` currently fails with
  a midend diagnostic for a user-level trait bound violation.
- `tests/errors/std_alias_unknown_member.spectra` currently fails with a generic
  "unknown or uninferrable type" diagnostic instead of a precise missing member
  diagnostic for `math.not_a_function`.

Both cases now fail during semantic analysis with stable codes and without
cascading fallback diagnostics.

### Scope

- route trait-bound specialization failures through semantic diagnostics before midend lowering
- improve qualified module/member lookup diagnostics for imports, stdlib modules, and aliases
- keep candidate export hints for known modules
- add assertions or validation coverage for diagnostic family/category

### Acceptance

- trait bound violations in user code are reported as semantic diagnostics, not internal or midend errors
- unknown qualified module members report the missing member and candidate module exports
- regression tests assert diagnostic category, stable code, message, and lack of cascading diagnostics for these cases

### Evidence

- `tests/errors/trait_bound_missing_method_stress.spectra` now emits one `error[E010]` semantic diagnostic.
- `tests/errors/std_alias_unknown_member.spectra` now emits one `error[E011]` semantic diagnostic with available `math` exports.
- `compiler/tests/stage_smoke.rs` asserts both regressions are semantic, coded, non-cascading, and not midend.
- `scripts/validate_r108_diagnostic_classification.py` validates the CLI JSON contract and is integrated into `run_tests.ps1`.
- Focused validation: `cargo test -p spectra-compiler`; `python scripts\validate_r108_diagnostic_classification.py --binary target\debug\spectralang.exe`.

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

## R-205 Float Const Cast Codegen

- Status: `not_started`
- Priority: `P1`
- Owner: `backend`
- Dependencies: `R-201`, `R-202`

### Problem Found

A reduced valid program using `const FLOATY: f64 = 7.75;` followed by
`let truncated: int = FLOATY as int;` reaches codegen and fails with
`Failed to define function 'main': Compilation error: Verifier errors`.

The normal validation suite avoids this exact lowering path for now, but the
reproducer is preserved in
`tests/known_issues/float_const_to_int_cast_codegen_verifier.spectra`.

### Scope

- inspect lowering/backend value kinds for float constants used in casts
- support f32/f64 const-to-int casts without invalid Cranelift IR
- ensure invalid casts still fail semantically before backend
- add regression coverage once fixed

### Acceptance

- `const X: f64 = ...; let y: int = X as int;` compiles without Cranelift verifier errors
- f32 and f64 const-to-int casts have semantic and backend regression tests
- invalid casts still fail with semantic diagnostics rather than backend verifier failures

### Evidence

- Found while creating `tests/validation/109_numeric_cast_const_boundaries_stress.spectra`.
- Known issue file: `tests/known_issues/float_const_to_int_cast_codegen_verifier.spectra`.

## R-206 Generic Return Type Enforcement

- Status: `not_started`
- Priority: `P0`
- Owner: `semantic`
- Dependencies: `R-204`

### Problems Found

- A generic function declared as `fn bad<T>(value: T) -> string { return value; }`
  currently compiles when instantiated with `int`, even though the body returns
  a type parameter incompatible with the declared concrete return type.
- A related invalid generic function declared as returning `int` can reach
  backend codegen and fail with `Verifier errors` instead of semantic analysis.

### Scope

- validate generic function bodies against declared return types before
  specialization/lowering
- validate monomorphized return substitutions before backend
- add semantic diagnostics for generic return mismatches
- add negative regression tests that fail semantically, not in codegen

### Acceptance

- generic functions cannot return unconstrained type parameters where a concrete return type is declared
- invalid generic return mismatches fail during semantic analysis with stable diagnostics
- no invalid generic return mismatch reaches backend codegen or verifier errors

### Evidence

- Known issue file: `tests/known_issues/generic_return_annotation_not_enforced.spectra`.
- Known issue file: `tests/known_issues/generic_return_type_mismatch_codegen_verifier.spectra`.
- Negative suite replacement: `tests/errors/generic_wrong_return_type_stress.spectra` covers the currently stable semantic diagnostic path.

---

# Phase 3: Tensor Core

## R-301 Tensor Type Design

- Status: `complete`
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

## R-1203 Filesystem Host Call Path Safety

- Status: `not_started`
- Priority: `P1`
- Owner: `runtime`
- Dependencies: `R-105`, `R-1202`

### Problem Found

While adding advanced AI examples, a first draft wrote files directly to nested
paths such as `target/ai-examples/advanced-phase16-17/run-a/lock.txt` before
the parent directory existed. The run produced a native process crash instead
of a controlled Spectra diagnostic. The example was rewritten to use already
safe paths, but the runtime behavior should be hardened.

### Scope

- audit `std.fs` host calls for unchecked filesystem failures
- define the contract for missing parent directories in `fs_write`
- ensure filesystem failures become controlled runtime diagnostics or safe
  return values, never native crashes
- add regression coverage for nested paths, invalid paths, and overwrite cases
- allow examples and future AI artifact pipelines to use nested output
  directories safely

### Acceptance

- `std.fs.fs_write` on nested missing parent directories either creates parents or returns a controlled runtime diagnostic, never a native process crash
- regression tests cover nested paths, invalid paths, and existing-file overwrite behavior
- AI examples may safely write nested artifact paths without precreating directories

### Evidence

- Found while testing `examples/ai/advanced_phase16_17_training_memory_pipeline.spectra`.
- Initial process exit was a native crash code before the example was adjusted.
- Current examples avoid the crash path; this item tracks the runtime fix.

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

- Status: `complete`
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
- Rank, dtype, static shape, layout, and device mismatches on explicitly typed tensor bindings fail during semantic analysis with stable JSON diagnostic codes `E1401` through `E1405`.
- Device/layout annotations use the same `Tensor<...>` surface, for example `Tensor<float, rank2, dim2, dim3, row_major, cpu>`.
- Existing `std.tensor` handle calls remain accepted through the handle compatibility layer.

### Completion evidence

- `tests/validation/80_phase14_tensor_language_core.spectra` covers first-class Tensor annotations, literals, dynamic dimensions, layout/device annotations, and `diff { ... }`.
- `tests/errors/tensor_rank_mismatch.spectra`, `tensor_dtype_mismatch.spectra`, `tensor_shape_mismatch.spectra`, `tensor_layout_mismatch.spectra`, and `tensor_device_mismatch.spectra` cover stable Tensor diagnostics.
- `.\run_tests.ps1` is the acceptance gate for the integrated language/CLI suite.

## R-1402 Shape and DType Type System

- Status: `complete`
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

- Static rank metadata, dtype metadata, static/dynamic dimension metadata, layout metadata, and device metadata are represented for `Tensor<float, ...>`.
- Rectangular rank2 tensor literal shape mismatches are rejected during semantic analysis.
- Tensor-returning `std.tensor` operations now expose compiler-visible Tensor return types for core autodiff paths.
- Static shape checks cover declared tensor compatibility, elementwise tensor operations, `tensor.matmul`, `tensor.reshape`, and `ml.linear`.
- `tests/validation/81_static_shape_mlp_validation.spectra` validates a neural-network linear layer with static shapes end-to-end.

### Completion evidence

- `tests/errors/tensor_operation_shape_mismatch.spectra`, `tensor_matmul_shape_mismatch.spectra`, `tensor_reshape_shape_mismatch.spectra`, and `ml_linear_shape_mismatch.spectra` cover static operation-level shape diagnostics.
- `.\run_tests.ps1` is the integrated acceptance gate.

## R-1403 Differentiable Language Blocks

- Status: `complete`
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
- Unsupported qualified stdlib operations inside `diff { ... }` produce stable diagnostic `E1406`.
- Gradient coverage includes tensor math, helper functions, control flow, and `std.ml` loss/layer integration.

### Completion evidence

- `tests/validation/82_diff_block_gradient_coverage.spectra` covers differentiable tensor math, control flow, helper calls, and ML loss/layer execution.
- `tests/errors/diff_block_unsupported_operation.spectra` verifies `E1406` for non-differentiable stdlib calls inside a differentiable region.
- Block syntax is the documented Phase 14 production surface; separate differentiable function annotations remain a future extension, not a Phase 14 completion gate.

---

# Phase 15: Production Numerical Performance

## R-1501 Numerical Performance Benchmark Suite

- Status: `complete`
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

### Completion evidence

- `runtime/examples/numerical_performance_bench.rs` runs the release-mode runtime benchmark suite and emits schema `spectra.r1501.benchmark.v1` JSON.
- `docs/performance/r1501-benchmark-baseline.json` stores checked-in thresholds for every required benchmark category.
- `scripts/validate_r1501_bench.py` runs the release benchmark, writes `target/r1501-benchmark-report.json`, checks correctness, verifies category coverage, and fails when `ns_per_iter` exceeds configured thresholds.
- `run_tests.ps1` includes `validate_r1501_bench` as the `phase15-performance` gate.

## R-1502 Memory Planner and Tensor Lifetime Analysis

- Status: `complete`
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

### Completion evidence

- `runtime/src/stdlib/mod.rs` tracks tensor allocation/release lifetimes in the runtime tensor registry, including dtype, shape, bytes, allocation step, release step, active status, and allocation site.
- `std.tensor.memory_report()` returns schema `spectra.tensor.memory_report.v1` JSON with peak bytes, active bytes, reuse rate, allocation-site count, and tensor lifetime records.
- `std.tensor.stats_lifetime_records`, `stats_released_lifetimes`, `stats_allocation_sites`, and `stats_reuse_rate_per_mille` expose machine-checkable memory-planner metrics.
- `docs/performance/r1502-memory-planner.md` documents the JSON schema, public metrics, validation commands, and current runtime-backed scope.
- `tests/validation/83_tensor_memory_planner.spectra` validates buffer reuse and bounded memory behavior through a repeated training loop.
- `tensor_runtime_phase15_memory_report_tracks_lifetimes_sites_and_reuse` validates the report contents in runtime unit tests.

## R-1503 Numerical Correctness and Determinism Certification

- Status: `complete`
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

### Completion evidence

- `std.tensor.set_deterministic_mode`, `deterministic_mode`, `tolerance_abs`, and `tolerance_rel` expose deterministic-mode and tolerance policy hooks.
- `runtime/examples/numerical_correctness_cert.rs` emits schema `spectra.r1503.correctness.v1` portable correctness artifacts for RNG, reductions, matmul, convolution, and optimizer checks.
- `docs/performance/r1503-correctness-baseline.json` stores the checked-in tolerance policy and expected portable results.
- `scripts/validate_r1503_correctness.py` runs the release certifier and compares observed artifacts against the baseline.
- `tests/validation/84_numerical_determinism.spectra` validates seeded RNG and exact matmul behavior through the language.
- `run_tests.ps1` includes `validate_r1503_correctness` as the `phase15-correctness` gate.

---

# Phase 16: Accelerator and Graph Compilation

## R-1601 Tensor Graph IR

- Status: `complete`
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

### Completion evidence

- `spectra_midend::TensorGraph::from_ir_module` extracts tensor-producing SSA host calls into graph nodes with operator, shape, dtype, layout, device, dependency, and source metadata.
- `TensorGraph::validate()` catches cycles, invalid dependencies, matmul shape mismatches, elementwise/loss shape mismatches, and same-device violations.
- `TensorGraph::stable_dump()` produces deterministic graph dumps; `midend/tests/snapshots/tensor_graph.snap` locks the snapshot format.
- `midend/tests/tensor_graph_tests.rs` covers a real lowered tensor program plus negative shape, device, and cycle cases.
- `run_tests.ps1` includes the `phase16-graph` gate through `cargo test -p spectra-midend --test tensor_graph_tests`.

## R-1602 Graph Optimization and Fusion

- Status: `complete`
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

### Completion evidence

- `TensorGraph::optimize()` performs deterministic graph-level fusion for single-consumer elementwise chains and elementwise chains feeding reductions.
- `TensorGraphOptimizationReport` records original/optimized node counts, fused groups, fused elementwise op count, fused reduction count, reusable edges, and `1e-9` absolute/relative tolerance policy.
- `TensorGraph::compare_optimized()` compares observable optimized outputs against the original graph.
- `midend/tests/tensor_graph_tests.rs` covers elementwise fusion, reduction-adjacent fusion, optimized/unoptimized comparison, and stable optimized snapshots.
- `examples/ai/tensor_graph_elementwise_fusion.spectra` and `examples/ai/tensor_graph_reduction_fusion.spectra` provide runnable Spectra examples for the optimized graph patterns.
- `run_tests.ps1` includes the `phase16-optimization` gate through `cargo test -p spectra-midend --test tensor_graph_tests optimizer`.

## R-1603 Production GPU Backend

- Status: `complete`
- Priority: `P0`
- Owner: `numerics`
- Dependencies: `R-702`, `R-1601`, `R-1503`

### Scope

- production accelerator execution for core ops
- CPU fallback
- device capability detection
- accelerator diagnostics

### Acceptance

- GPU execution supports tensor transfer, matmul, reductions, elementwise ops, convolution, and autodiff-required backward kernels.
- CPU fallback remains available and produces equivalent results within tolerance.
- Device capability detection and error reporting are documented and tested.

### Completed Evidence

- `runtime/src/stdlib/mod.rs` exposes `device_status`, `stats_gpu_kernel_ops`, and `stats_cpu_fallbacks`, and records successful WGPU kernels separately from CPU fallbacks.
- Optional WGPU kernels for elementwise ops, unary ops, reductions, `matmul`, and `std.ml.conv2d` fall back to CPU on dispatch failure instead of returning an internal operation failure.
- `compiler/src/semantic/builtin_modules.rs` and `midend/src/lowering.rs` expose the new public tensor diagnostics through normal Spectra compilation.
- `tests/validation/91_tensor_phase16_gpu_backend.spectra` validates the public API and skips accelerator-only execution safely when WGPU is unavailable.
- `scripts/validate_r1603_gpu_backend.py` runs the default CPU diagnostics test and the optional `--features gpu` backend test.
- `run_tests.ps1` includes the `phase16-gpu` gate.

---

# Phase 17: Data and Experiment Platform

## R-1701 Dataset and DataFrame Runtime

- Status: `complete`
- Priority: `P1`
- Owner: `runtime`
- Dependencies: `R-602`, `R-802`, `R-1101`

### Scope

- dataframe APIs
- CSV, JSONL, NPY, directory-backed datasets
- batching, shuffling, transforms, train/test split, deterministic seeding

### Acceptance

- CSV, JSONL, NPY, and directory-backed datasets can be loaded through stable APIs.
- Batching, shuffling, map/filter transforms, train/test split, and deterministic seeding are tested.
- Tabular preprocessing example trains end-to-end without Python glue.

### Completed Evidence

- `std.ml` exposes `dataset_from_csv`, `dataset_from_jsonl`, `dataset_from_npy`, `dataset_from_directory`, dataset transforms, train/test splits, and numeric dataframe APIs.
- Runtime datasets materialize into existing `std.tensor` handles, so dataloaders and training APIs work without a separate data execution path.
- `runtime/src/stdlib.rs` includes a focused R-1701 test that creates CSV, JSONL, NPY, and directory-backed fixtures and validates transforms, splits, dataframe column extraction, and deterministic dataloader batches.
- `tests/validation/92_ml_phase17_data_runtime.spectra` validates the public language surface and runs tabular training from file-backed data.
- `examples/ai/tabular_dataset_training.spectra` provides an AI example that trains from checked-in tabular fixtures.
- `scripts/validate_r1701_data_runtime.py` and `run_tests.ps1` include the `phase17-data` gate.

## R-1702 Experiment Tracking and Reproducibility

- Status: `complete`
- Priority: `P1`
- Owner: `ml`
- Dependencies: `R-901`, `R-1701`

### Scope

- run manifests
- configs, metrics, artifacts, seeds, lockfiles, model outputs
- run comparison
- exact reproduction command

### Acceptance

- Training runs emit a structured experiment manifest.
- Metrics and artifacts can be compared across runs.
- A documented command reproduces a reference training result from lockfile and manifest.

### Completed Evidence

- `std.ml` exposes `experiment_start`, config/metric/artifact logging, lockfile/model output attachment, `experiment_finish`, manifest path, reproduction command, and manifest comparison APIs.
- The runtime writes schema `spectra.ml.experiment.v1` with seed, configs, metrics, artifacts, lockfile, model output, manifest path, and reproduction command.
- Artifact, lockfile, and model output records include size and FNV-1a 64-bit content hash.
- `ml.experiment_compare_manifests` compares configs, metrics, artifacts, lockfile, model output, and seed while ignoring run directory differences.
- `tests/validation/93_ml_phase17_experiment_tracking.spectra` validates the public language API.
- `examples/ai/experiment_tracking_reproducibility.spectra` emits a tracked AI training-run manifest.
- `scripts/validate_r1702_experiment_tracking.py` parses the example manifest and validates schema, metrics, artifacts, lockfile, model output, seed, and reproduction command.
- `run_tests.ps1` includes the `phase17-experiments` gate.

## R-1703 Distributed Training Foundations

- Status: `complete`
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

### Completed

- `std.ml` exposes `distributed_session_start`, `distributed_worker_step`, `distributed_global_step`, `distributed_worker_step_count`, `distributed_checkpoint_save`, `distributed_resume`, and `distributed_summary`.
- The supported topology is explicitly scoped to deterministic single-machine simulated workers; networked multi-process training, GPU collectives, sharding, and elastic membership remain non-goals for this item.
- Checkpoint JSON uses schema `spectra.ml.distributed_checkpoint.v1` and records topology, seed, worker count, global step, interrupted worker, checkpoint path, and per-worker state.
- `ml.distributed_resume` restores a new session handle from checkpoint contents and reactivates workers after an interruption.
- `tests/validation/94_ml_phase17_distributed_training.spectra` validates the public language API.
- `examples/ai/distributed_training_checkpoint.spectra` provides an AI reference example for checkpoint/resume.
- `scripts/validate_r1703_distributed_training.py` runs the runtime test, public Spectra validation, AI example, and parses the generated checkpoint.
- `run_tests.ps1` includes the `phase17-distributed` gate.

---

# Phase 18: Model Ecosystem and LLM Workloads

## R-1801 ONNX Import and Export

- Status: `complete`
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

### Completed

- `std.ml` exposes `onnx_export`, `onnx_import_summary`, `onnx_validate`, and `onnx_roundtrip`.
- Export writes binary ONNX `ModelProto` protobuf artifacts for supported model kinds.
- Import parses the supported ONNX subset and returns a machine-readable summary with graphs, nodes, inputs, outputs, ops, dtype, and ranked-shape status.
- Round-trip preserves a validated supported ONNX artifact.
- Covered model kinds are `linear`, `conv`, `activation`, `normalization`, and `transformer`.
- `tests/validation/95_ml_phase18_onnx_import_export.spectra` validates the public language API.
- `examples/ai/onnx_transformer_export.spectra` provides an AI reference example for transformer ONNX export/import.
- `scripts/validate_r1801_onnx_import_export.py` runs runtime, Spectra, and example validation and independently parses generated `.onnx` files.
- `run_tests.ps1` includes the `phase18-onnx` gate.

## R-1802 Transformer and LLM Runtime Primitives

- Status: `complete`
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

### Completed

- `std.ml` exposes `embedding_lookup`, `positional_encoding`, `layer_norm`, `gelu`, `swiglu`, `attention`, `kv_cache_new`, `kv_cache_append`, `kv_cache_keys`, `kv_cache_values`, `kv_cache_len`, and `logits_sample`.
- Runtime implementations operate on real `std.tensor` handles and validate dtype/shape contracts before execution.
- Scaled dot-product attention, layer norm, GELU/SwiGLU, sinusoidal positional encoding, KV cache append/materialization, and softmax temperature sampling are covered by runtime tests.
- The toy transformer AI example now uses real transformer primitives instead of placeholder dot/matmul arithmetic.
- `tests/validation/96_ml_phase18_transformer_primitives.spectra` validates the public language API.
- `scripts/validate_r1802_transformer_primitives.py` runs runtime, public Spectra, and AI example validation.
- `run_tests.ps1` includes the `phase18-transformers` gate.

## R-1803 Tokenization, Embeddings, and RAG Toolkit

- Status: `complete`
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

### Completed

- `std.ml` exposes `tokenizer_wordpiece`, `tokenizer_encode`, `tokenizer_decode`, `text_embed`, `vector_index_new`, `vector_index_insert`, `vector_index_query`, `vector_index_persist`, `vector_index_load`, `rag_chunk_text`, `rag_build_prompt`, and `rag_evaluate_answer`.
- WordPiece-style tokenization uses deterministic greedy longest-match segmentation and deterministic decode with `##` continuation merging.
- Text embeddings use deterministic normalized hashing so RAG examples run without Python glue or external model downloads.
- Vector indexes support cosine insert/query plus JSON persist/load.
- RAG utilities cover deterministic chunking, prompt assembly, model-call boundary integration, and token-overlap F1 evaluation.
- `tests/validation/97_ml_phase18_rag_toolkit.spectra` validates the public language API.
- `examples/ai/rag_retrieval_pipeline.spectra` runs retrieval, prompt assembly, model-call boundary, answer evaluation, and persistence end-to-end.
- `scripts/validate_r1803_rag_toolkit.py` runs runtime, public Spectra, and AI example validation and parses persisted vector index evidence.
- `run_tests.ps1` includes the `phase18-rag` gate.

---

# Phase 19: AI Operations and Evaluation

## R-1901 Model Evaluation and Metrics Suite

- Status: `complete`
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

### Completed

- `std.ml.metrics_classification`, `metrics_regression`, `metrics_ranking`, `metrics_generation`, and `serving_metrics` emit deterministic JSON metric payloads covering the required classification, regression, ranking, generation, latency, and throughput fields.
- `std.ml.evaluation_report` writes a versioned machine-readable JSON report plus a human-readable `.txt` companion report.
- `tests/validation/98_ml_phase19_evaluation_metrics.spectra` validates the public language API.
- `examples/ai/model_evaluation_report.spectra` provides a runnable AI reference example that gates model progression on evaluation evidence before serving/export workflows.
- `scripts/validate_r1901_evaluation_metrics.py` runs runtime, public Spectra, and AI example validation and parses the generated report.
- `run_tests.ps1` includes the `phase19-evaluation` gate.

## R-1902 AI Safety and Guardrail Runtime

- Status: `complete`
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

### Completed

- `std.serve.server_set_input_policy`, `server_set_output_policy`, `server_set_rate_limit`, and `server_set_fallback` attach deterministic guardrails to serving servers.
- `server_enqueue` enforces input policy and rate-limit failures before queueing; `server_process_batch` enforces output policy before returning model output.
- Guardrail failures complete requests with the configured fallback value, so callers receive degraded safe behavior instead of internal errors.
- `std.serve.server_last_diagnostic` emits structured JSON diagnostics and `server_audit_log` emits versioned JSON audit evidence.
- `tests/validation/99_phase19_ai_safety_guardrails.spectra` validates the public language API.
- `examples/ai/safe_serving_guardrails.spectra` covers blocked input, blocked output, and fallback behavior in a runnable AI serving example.
- `scripts/validate_r1902_ai_safety_guardrails.py` runs runtime, public Spectra, and AI example validation and parses generated audit evidence.
- `run_tests.ps1` includes the `phase19-safety` gate.

## R-1903 Model Monitoring and Drift Detection

- Status: `complete`
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

### Completed

- `std.serve.server_set_model_version` attaches model-version metadata to local serving servers.
- `std.serve.server_monitoring_snapshot` emits request, completed, blocked, cancelled, error, batch, pending, latency, throughput, and model-version metrics as JSON.
- `std.serve.server_distribution_summary` emits input/output distribution summaries for drift baselines and live traffic.
- `std.serve.drift_check` compares reference and live distribution summaries against a per-mille threshold and emits structured drift JSON.
- `std.serve.export_monitoring` writes a versioned JSON observability artifact with snapshot, distribution, drift, and audit data.
- `tests/validation/100_phase19_model_monitoring.spectra` validates the public language API.
- `examples/ai/model_monitoring_drift_detection.spectra` provides a runnable AI monitoring/drift example.
- `scripts/validate_r1903_model_monitoring.py` runs runtime, public Spectra, and AI example validation and parses the generated observability artifact.
- `run_tests.ps1` includes the `phase19-monitoring` gate.

---

# Phase 20: Production Certification

## R-2001 AI Conformance Suite

- Status: `complete`
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

### Completed Implementation

- `scripts/validate_r2001_ai_conformance.py` runs the production conformance gates for compiler, runtime, tensors, autodiff, graph, interop, package, serving, tooling, and docs/examples.
- The suite emits `target/r2001-conformance/conformance-report.json` with schema `spectralang.ai_conformance_report.v1` and conformance version `R-2001/v1`.
- Release-candidate certification is enforced by the script exit code: failed, timed-out, missing-category, or invalid-report gates reject the candidate.
- `run_tests.ps1` includes the `phase20-conformance` gate.
- `docs/architecture/r2001-ai-conformance-suite.md` documents the report contract, required categories, and certification rule.

### Validation

- `python scripts\validate_r2001_ai_conformance.py --keep-going`
- `.\run_tests.ps1`

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
