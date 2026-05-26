# Lowering and Backend Coverage Audit

Updated: 2026-05-21  
Roadmap item: `R-103`

This document maps frontend and semantic surface area to lowering support and backend behavior. It also converts unsupported production paths into explicit backlog references instead of leaving them implicit.

Status labels:

- `supported`: lowers and reaches backend successfully in the validated suite
- `partial`: works for the current suite but is not yet a production-ready substrate
- `deferred`: intentionally outside the current implementation envelope

## AST to Lowering Coverage

| Surface | Status | Notes |
| --- | --- | --- |
| Function bodies and blocks | supported | Includes final-expression handling and explicit `return`. |
| Local bindings and assignments | supported | Shadowing bugs identified during stabilization were fixed. |
| Arithmetic and comparison expressions | supported | |
| Branching (`if`, `elif`, `else`) | supported | |
| Loops (`while`, `for`, `loop`, `do-while`) | supported | Current gated loop forms lower correctly. |
| `break` / `continue` | supported | |
| Function calls | supported | Direct and imported stdlib calls lower. |
| Method calls | supported | Includes current trait-object paths used in the suite. |
| `match` expressions/statements | supported | Current validated examples and tests compile successfully. |
| Struct construction and field access | supported | |
| Enum construction and destructuring | supported | Includes nested generic enum inference cases in the test suite. |
| Trait objects / vtables | supported | Current suite passes for dyn-trait examples. |
| `Drop` flows | supported | Current destructor examples compile successfully. |
| Closures | partial | Non-capturing closures are stored, passed, invoked, and returned in validated paths; captured environments are deferred. |
| Const/static lowering | partial | Top-level `const` values lower as literals at use sites; `static` remains a surface/global item model. |
| Tensor/ndarray primitives | supported baseline | `std.tensor` Phase 3/4 lowers as stdlib host calls with safe runtime views; first-class tensor IR and device lowering remain deferred. |
| Tensor autodiff | supported baseline | `std.tensor` Phase 5 autodiff lowers as stdlib host calls; compiler-native autodiff IR remains deferred. |
| ML framework layer | supported baseline | `std.ml` Phase 6 lowers as stdlib host calls for modules, layers, losses, optimizers, and dataloaders. |

## IR Verification Coverage

| Area | Status | Notes |
| --- | --- | --- |
| Block and terminator structure | supported | Midend verification tests exist. |
| SSA well-formedness | supported | Recent fixes eliminated known `Value N not found` regressions in the validated suite. |
| Dead-code elimination interaction | supported | Regression covered after the cast/DCE bug fix. |
| Pattern-match lowering correctness | supported | Covered by validation examples and lowering tests. |
| Shadowing across branches | supported | Regression fixed and covered by tests. |
| Trait object method dispatch | supported | Current dyn-trait suite passes. |

## Backend Coverage

| Area | Status | Notes |
| --- | --- | --- |
| Cranelift-based code generation | supported | Primary backend path. |
| JIT execution | supported | Used by `run` and REPL workflows. |
| Object emission | supported | CLI can emit native object files. |
| Executable linking | supported | CLI supports `--emit-exe` on supported host setups. |
| Primitive integer/float/bool/char handling | supported | Includes the recent bool/char fixes. |
| Host-call bridge to stdlib | supported | Current std modules, including `std.tensor` Phase 3/4 ops, compile and run through the host-call surface. |
| SIMD/vectorized scientific kernels | deferred | Covered by Phase 4 and later. |
| GPU kernels and device lowering | deferred | Covered by Phase 7. |

## Unsupported Paths Converted to Backlog

| Gap | Backlog item |
| --- | --- |
| Exact-width numeric runtime semantics | post-`R-201` production hardening |
| Shape/size const contexts | `R-202` follow-up under tensor/type-system work |
| Tensor-first lowering model | `std.tensor` host-call bridge complete in `R-301` through `R-304`; first-class tensor IR remains future work |
| CPU numerical kernels | `R-401` through `R-403` |
| Autodiff graph and gradient lowering | `R-501` through `R-503` |
| GPU/device lowering | `R-701` through `R-703` |

## Operational Conclusion

For the current general-purpose language surface, lowering and backend coverage are now in a healthy Phase 1 state. For the stated AI/ML product goal, production baseline tensor, autodiff, and ML framework paths now exist through stdlib host calls; the remaining production work is explicit roadmap work in first-class tensor IR and acceleration.
