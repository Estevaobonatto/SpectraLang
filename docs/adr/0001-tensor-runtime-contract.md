# ADR 0001: Tensor Runtime Contract

Status: Accepted

Date: 2026-05-22

## Context

SpectraLang targets AI and machine learning workloads, so tensors need production semantics before higher-level autodiff and ML layers can be trusted. The compiler does not yet have a generic type-system surface capable of expressing `Tensor<T, Shape>` as a first-class syntax form without introducing a large type-system dependency into Phase 3.

Phase 3 is therefore defined as the production tensor core contract for the current compiler architecture, not as the final generic tensor syntax.

## Decision

The accepted Phase 3 tensor contract is:

- Public language API: `std.tensor` functions and exported `Tensor` type metadata.
- ABI representation: opaque integer handles returned by `std.tensor` constructors and operations.
- Runtime representation: dtype, shape, strides, layout, storage handle, and base offset.
- Default storage: CPU host storage.
- View semantics: reshape, flatten where contiguous, transpose, permute, and slice create safe shared-storage views where possible.
- Mutation semantics: `set` and `set2` use copy-on-write when storage is shared, preventing unsafe alias mutation through views.
- Shape validation: invalid rank, axis, reshape, matmul, concat, stack, and slice operations fail with deterministic host errors.
- Device model: CPU host is the only active production device in Phase 3. Device placement and accelerator execution remain Phase 7 work.

This keeps Phase 3 complete and production-usable for current Spectra programs without pretending that future generic tensor syntax already exists.

## Consequences

- `Tensor<T>` and static shape syntax are not part of the Phase 3 completion gate.
- Future type-system work may introduce typed tensor syntax on top of the current runtime handles.
- Runtime handles remain the stable ABI boundary for compiler lowering, CLI validation, and host calls.
- View and shape semantics are implemented in the runtime and validated by Rust unit tests and `.spectra` validation programs.

## Acceptance Evidence

- Runtime tests cover allocation/destruction, view lifetime safety, copy-on-write mutation, transform operations, shape errors, concat, stack, argmax, and batched matmul.
- `tests/validation/70_tensor_phase3_production.spectra` compiles through the public `std.tensor` API.
- Phase 4 kernel and allocator work builds on this runtime contract instead of replacing it.
