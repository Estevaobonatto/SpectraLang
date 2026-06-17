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
  documented by the policy; after R-118 this set is empty.
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

## R-109 Cross-Module String Value Handling

- Status: `complete`
- Priority: `P1`
- Owner: `backend`
- Dependencies: `R-105`

### Problems Found

The multi-file test project suite (`multi_file_projects/`) surfaced two
related string-handling defects in the cross-module and main-module paths:

- `let r = module::fn_returning_string(...); println(r);` prints a numeric
  pointer value (e.g. `2051441058384`) instead of the actual string content.
  This is reproducible for user modules and for `std.string` functions such as
  `to_upper`, `to_lower`, `concat`, `repeat_str`, `replace`, `reverse_str`, and
  `trim`.
- `let s = "a" + "b"; println(s);` in the main module causes a silent crash
  that suppresses all subsequent `println` output for the rest of the program.
  In-module string concatenation inside a callee function (e.g. inside a
  `while` loop) still works, so the defect is specific to the main-module
  initialization path for the concatenation result.

Both defects block any realistic multi-file program that needs to format
or assemble strings.

### Scope

- align the runtime string representation used by cross-module callee returns
  with the one used for in-module returns
- ensure `println` of a string value always reads through the correct runtime
  handle regardless of where the value was produced
- audit the main-module IR lowering for `let x = <expr>; println(x);` patterns
  where `<expr>` is a string concatenation of literals, and ensure the result
  is materialized through the same path as a `println("literal");` literal
- keep `std.string` return values printable without wrappers in user code
- keep in-module string concat inside loops/blocks, f-string interpolation, and
  `println` of bare string literals unchanged

### Acceptance

- a function in module B that returns a `string` is printable in module A and
  in `main` using `println` and shows the actual string
- `std.string.to_upper("hello")` prints `HELLO` when called from a user module
  or from `main`
- `let s = "Hello" + ", " + "World"; println(s);` in `main` prints
  `Hello, World` and does not abort the program
- regression tests cover: cross-module user string return, cross-module
  `std.string` return, and main-module chained literal string concatenation
- no regression in existing f-string interpolation or in-module string concat
  inside loops

### Evidence

- Reproduction project: `multi_file_projects/p2_string_utils/`
  (`main.spectra` shows both defects; `p4_stdlib_showcase/` shows the
  `std.string` return variant).
- Reduction output captured during the 2026-06-12 multi-file sweep.
- Focused validation: `cargo run -q -p spectra-cli -- run
  multi_file_projects/p2_string_utils` must print the full expected stdout
  including all `--- string ops ---` lines and the post-concat `println`
  values.
- Completed on 2026-06-12 by aligning stdlib `MethodCall` return-type
  inference with hostcall lowering and lowering string `+` through
  `spectra.std.string.concat` instead of pointer arithmetic.
- Regression coverage: `tests/projects/valid/cross_module_strings/` and
  `scripts/validate_r109_cross_module_strings.py`, integrated into
  `run_tests.ps1`.
- Validation evidence: `python scripts\validate_r109_cross_module_strings.py
  --binary target\debug\spectralang.exe`; `cargo test -p spectra-midend`;
  `cargo test -p spectra-compiler`; `cargo test -p spectra-cli`.

## R-110 Cross-Module Type and Method Resolution

- Status: `complete`
- Priority: `P1`
- Owner: `semantic`
- Dependencies: `R-105`

### Problems Found

Two related defects appeared when a struct or enum type is defined in one
module and used by another. These are now covered by the `R-110` regression
project and validator:

- Static and instance methods on a struct defined in another module are not
  visible to the importer. `let c = Counter::new(1);` in `main` against a
  `Counter` struct in `counter` module fails with
  `error[semantic]: Enum 'Counter' is not defined` even when the struct is
  marked `pub`. After switching to a `pub struct Counter { pub fields }` plus
  `pub impl Counter`, method calls fail with
  `error[semantic]: No methods defined for type 'Counter'`.
- A struct with a `string` field (e.g. `Item { sku: string, ... }`) that is
  constructed by a cross-module factory function compiles to IR where the
  call result value is not linked to the next use, producing
  `error[codegen]: Value N not found` (e.g. `Value 282 not found`). Switching
  fields to `int` makes the same code compile, so the defect is specific to
  aggregate-typed struct fields in cross-module factory returns.

The previous workaround in the multi-file projects was to expose only free
factory functions (e.g. `counter_new`, `counter_tick`) and to avoid exercising
direct method dispatch across module boundaries. That workaround remains
supported, but the production path now resolves public cross-module inherent
methods directly.

### Scope

- import resolution must surface the type, its `impl` blocks, and its
  inherent methods when the type is declared in another module
- import resolution must support `pub` on `struct`, `enum`, `impl`, and
  `fn` items so cross-module code can call static and instance methods
- cross-module factory functions that return a struct containing `string`
  fields must lower to IR where the call result is correctly threaded into
  the next use
- when a method genuinely does not exist, the diagnostic must name the
  receiver type, the method, and the candidate impl blocks in scope
- keep the existing `pub fn` cross-module function call path unchanged

### Acceptance

- `Counter::new(1)` and `c.tick()` work in `main` when `Counter` and its
  `impl` are defined in a different module and marked `pub`
- `Item { sku: string, name: string, ... }` constructed by a cross-module
  factory compiles and runs; `it.sku` and `it.name` return the expected
  strings
- the existing factory-function workaround continues to work
- regression tests cover: cross-module method dispatch (static + instance),
  cross-module enum variant construction from a foreign module, and a
  cross-module struct with `string` fields
- missing-method diagnostics on cross-module types report the type, method,
  and candidate impl blocks

### Evidence

- Implemented cross-module method export/import in the semantic module registry
  for visible inherent methods, including static methods and instance methods.
- Implemented `module::Type::method(...)` validation before struct-literal and
  enum-variant fallback so associated methods on foreign types do not produce
  false `Enum ... is not defined` or struct-initializer diagnostics.
- Fixed midend imported aggregate return lowering so imported function/method
  returns are lowered after imported struct/enum layouts are registered, avoiding
  `Pointer(Void)` receiver degradation and later `Value N not found` paths.
- Added `tests/projects/valid/cross_module_types_methods` covering
  `Counter::new`, `counter::Counter::with_value`, `c.tick()`, `c.read()`,
  cross-module enum variant construction, and `Item` string fields/methods.
- Added `tests/projects/invalid/cross_module_missing_method` and
  `scripts/validate_r110_cross_module_types_methods.py` to assert the missing
  method diagnostic names the type, missing method, and candidate impl methods.
- Validation completed:
  `cargo test -p spectra-compiler`;
  `cargo test -p spectra-midend -p spectra-cli`;
  `cargo build -p spectra-cli`;
  `python scripts/validate_r110_cross_module_types_methods.py --binary target/debug/spectralang.exe`.

## R-111 Cross-Module Aggregate Codegen

- Status: `complete`
- Priority: `P1`
- Owner: `backend`
- Dependencies: `R-105`

### Problems Found

Two Cranelift-side defects appeared when the value being passed or matched
is an aggregate that crosses a module boundary:

- A callee function that takes an `[string]` array parameter and indexes
  into it (e.g. `let out = out + parts[i];`) compiles through semantic
  analysis but fails Cranelift verification:
  `error[codegen]: Failed to define function 'join_strings': Compilation
  error: Verifier errors`. The same function with `[int]` works.
- A `match` on a `Result<T, E>` value produced in the main module lowers
  to IR where the payload of the matched arm is loaded as `void`:
  `%v547 = load(void) %v546`, which the Cranelift verifier rejects with
  `Failed to define function 'main': Compilation error: Verifier errors`.
  The same `match` on `Option<T>` works.

Both defects block core multi-file patterns (joining a list of strings,
chaining fallible operations in `main`).

### Scope

- ensure parameter storage for `[T]` arrays in callee functions is
  consistent across element types, including `string`
- ensure the IR for `match` arms on `Result<T, E>` (and any other generic
  enum) threads the arm payload through the correct element type
- keep the working paths intact: `[int]` parameter + index, `Option<T>`
  match, and all in-module enum match patterns

### Acceptance

- a function `pub fn join(parts: [string], n: int, sep: string) -> string`
  in another module compiles and runs through Cranelift
- `let v: int = match ok_r { Result::Ok(v) => v, Result::Err(e) => e * -1 };`
  in `main` lowers to IR where the arm value has element type `int`, not
  `void`, and the program runs to completion
- regression tests cover: `[string]` parameter + index in a callee module
  and `match` on `Result<T, E>` in a non-stdlib context
- no regression in `[int]` indexing, `Option` match, or enum variant match

### Evidence

- Reproduction projects: `multi_file_projects/p2_string_utils/string_utils.spectra`
  (the `[string]` index case, first observed as `join_strings`) and
  `multi_file_projects/p3_inventory_oop/main.spectra` (the `Result` match
  case, with `--dump-ir` showing the `load(void)` lowering).
- Focused validation: `cargo run -q -p spectra-cli -- run
  multi_file_projects/p2_string_utils` and `multi_file_projects/p3_inventory_oop`
  must complete without `Verifier errors` and without `load(void)` in the
  emitted IR.
- Completed implementation adds typed `[T]` parsing/lowering instead of erasing
  array element type, semantic specialization for generic enum annotations, and
  typed-expression refinement for generic enum constructors in local bindings.
- Dedicated regression project:
  `tests/projects/valid/cross_module_aggregate_codegen`, covering `[string]`
  parameter indexing, `[int]` indexing regression, and `Result<int, int>`
  `Ok`/`Err` matches across module boundaries.
- Dedicated gate: `scripts/validate_r111_cross_module_aggregate_codegen.py`,
  integrated into `run_tests.ps1`.
- Validation: `python scripts\validate_r111_cross_module_aggregate_codegen.py
  --binary target\debug\spectralang.exe`; `cargo test -p spectra-runtime -p
  spectra-compiler -p spectra-midend -p spectra-cli`; `.\run_tests.ps1`
  reported 239/239 decisive tests passing.

## R-112 Runtime Float-to-Int Cast Codegen

- Status: `complete`
- Priority: `P0`
- Owner: `backend`
- Dependencies: `R-105`, `R-205`

### Problems Found

The 2026-06-12 full test run exposed verifier failures whenever a non-constant
`float` value returned by a hostcall is cast to `int` and then used in normal
control flow or return paths. This is distinct from `R-205`, which covered
constant float casts.

Failing surfaces:

- `tests/validation/59_import_surface.spectra`: `math.floor_f(9.9) as int`
  and `math.ceil_f(1.1) as int` fail in `compute`.
- `tests/validation/106_import_alias_named_std_stress.spectra`:
  `math.floor_f(upper as float + 0.9) as int` fails in `numeric_mix`.
- `tests/validation/67_tensor_float_surface.spectra`: `tensor.mean_f(...) as
  int`, `tensor.sum_f(...) as int`, and `tensor.get2_f(...) as int` fail in
  `main`.
- The same pattern appears inside tensor/autodiff regressions that compare
  float reductions against integer literals.

IR evidence:

- `target/r112-import-ir.txt` shows `hostcall spectra.std.math.floor_f`,
  `hostcall spectra.std.math.ceil_f`, followed by `cast(float -> int)`.
- `target/r113-tensor-float-ir.txt` shows `hostcall spectra.std.tensor.mean_f`,
  `sum_f`, `get2_f`, followed by `cast(float -> int)`.

### Correction Plan

- Add a backend-level reduced Rust/codegen test that lowers `hostcall f64 ->
  cast(float -> int) -> ret int` without involving stdlib imports.
- Inspect Cranelift lowering for `InstructionKind::Cast` and confirm whether
  runtime hostcall results are materialized as `I64` bit patterns or `F64`
  SSA values.
- Normalize the IR/Cranelift contract:
  - `IRType::Float` values must be Cranelift `F64`, including hostcall return
    results.
  - `cast(float -> int)` must emit a valid Cranelift float-to-signed-int
    conversion/truncation sequence.
  - `cast(int -> float)` must emit a valid signed-int-to-float conversion.
  - bit reinterpretation must be reserved for runtime ABI boundaries, not
    semantic casts.
- Add targeted `.spectra` regressions for:
  - std.math `floor_f`/`ceil_f` returned floats cast to `int`;
  - std.tensor `mean_f`/`sum_f`/`get_f`/`get2_f` returned floats cast to `int`;
  - repeated cast use in both condition and return branch.
- Add a dedicated validation script, for example
  `scripts/validate_r112_runtime_float_cast_codegen.py`, and wire it into
  `run_tests.ps1` before the broad conformance gates.

### Acceptance

- `tests/validation/59_import_surface.spectra` compiles and runs.
- `tests/validation/106_import_alias_named_std_stress.spectra` compiles and
  runs.
- `tests/validation/67_tensor_float_surface.spectra` compiles and runs.
- IR dumps for the reduced cases contain no verifier errors and no invalid
  float/int ABI mismatch at hostcall boundaries.
- `cargo test -p spectra-backend` and the dedicated R-112 validation script
  pass.

### Completed Evidence

- Implemented typed hostcall result metadata in the midend IR and builder so
  stdlib descriptors can preserve logical return types through backend codegen.
- Backend hostcall result materialization now loads the runtime ABI `i64` slot
  and normalizes typed results: `float` is bitcast to `F64`, `bool` is reduced
  to `I8`, and `char` is reduced to `I32`.
- Added backend reduced test
  `test_typed_host_float_result_cast_to_int_codegen`.
- Added and wired `scripts/validate_r112_runtime_float_cast_codegen.py`.
- Validation: `python scripts\validate_r112_runtime_float_cast_codegen.py
  --binary target\debug\spectralang.exe`; `cargo test -p spectra-backend`;
  `cargo test -p spectra-midend`; `cargo test -p spectra-cli`;
  `cargo fmt --check`.
- Full runner evidence: `.\run_tests.ps1` now passes all direct
  `tests/validation/*.spectra` files, including the former R-112/R-113/R-114
  failure surfaces, and leaves only the R-115/R-2001 docs-example issue open.

## R-113 Tensor Parameter and Return ABI Codegen

- Status: `complete`
- Priority: `P0`
- Owner: `backend`
- Dependencies: `R-1401`, `R-1402`, `R-401`

### Problems Found

Tensor values represented as typed `Tensor<...>` annotations compile in simple
local code, but verifier failures appear when a tensor value crosses a user
function boundary as a parameter or return value and is then passed to std.tensor
hostcalls.

Failing surfaces:

- `tests/validation/80_phase14_tensor_language_core.spectra` fails in
  `vector_total(values: Tensor<float, rank1>)`.
- `tests/validation/102_pattern_tensor_ai_composition_stress.spectra` fails in
  `vector_total(values: Tensor<float, rank1, dim4, row_major, cpu>)`.
- The same ABI path is likely involved in `diff` helper functions that accept
  typed tensors and return `Tensor<float, rank0>`.

### Correction Plan

- Reduce the failure to a two-function program:
  `fn sum(values: Tensor<float, rank1>) -> int { return tensor.sum_f(values) as int; }`.
- Inspect function signature lowering for tensor-typed parameters and returns:
  tensors must remain opaque runtime handles (`i64`) in backend ABI while
  preserving semantic metadata in the midend.
- Audit `lower_type_annotation`, function parameter registration, hostcall
  argument typing, alloca/load/store for tensor variables, and return lowering
  for typed tensor aliases.
- Ensure static metadata (`rank`, `dim`, `layout`, `device`) never leaks into
  Cranelift value types as aggregate or `void` payloads.
- Add midend validation that any `IRType::Tensor` value crossing a backend ABI
  boundary is represented as handle-compatible scalar storage.
- Add regressions for:
  - tensor parameter to helper function;
  - tensor return from helper function;
  - static-shape tensor passed to dynamic-rank parameter;
  - tensor helper inside pattern/match-heavy function.

### Acceptance

- `tests/validation/80_phase14_tensor_language_core.spectra` compiles and
  runs.
- `tests/validation/102_pattern_tensor_ai_composition_stress.spectra` compiles
  and runs once R-112 is also satisfied.
- Reduced tensor parameter/return tests pass without verifier errors.
- No regression in `tests/validation/66_tensor_core_surface.spectra`,
  `68_tensor_phase4_kernels.spectra`, or `81_static_shape_mlp_validation.spectra`.

### Completed Evidence

- `tests/validation/102_pattern_tensor_ai_composition_stress.spectra` now marks
  the tensor used for `tensor.grad(v)` with `tensor.requires_grad(v_base, true)`,
  matching the production autodiff contract already used by the dedicated diff
  tests.
- Added direct regressions for enum/aggregate function-call ABI and tensor
  composition:
  `112_enum_struct_variant_function_call.spectra`,
  `113_tensor_free_all_then_enum_call.spectra`,
  `114_tensor_grad_enabled_then_enum_call.spectra`,
  `115_mixed_enum_variant_function_call.spectra`,
  `116_enum_then_ml_linear_tensor_call.spectra`,
  `117_enum_ml_autodiff_composition.spectra`,
  `118_autodiff_after_ml_without_final_free_all.spectra`, and
  `119_autodiff_after_ml_with_final_free_all.spectra`.
- Validation run: `target\debug\spectralang.exe run
  tests\validation\80_phase14_tensor_language_core.spectra`.
- Validation run: `target\debug\spectralang.exe run
  tests\validation\102_pattern_tensor_ai_composition_stress.spectra`.
- Validation run: `target\debug\spectralang.exe run
  tests\validation\66_tensor_core_surface.spectra`,
  `68_tensor_phase4_kernels.spectra`, and
  `81_static_shape_mlp_validation.spectra`.
- IR validation found no `Verifier`, `load(void)`, invalid tensor handle, or
  `cast(...Tensor...)` markers in the reduced composition dumps.

## R-114 Autodiff and Diff Block Tensor Codegen Stabilization

- Status: `complete`
- Priority: `P0`
- Owner: `midend`
- Dependencies: `R-112`, `R-113`, `R-501`, `R-502`

### Problems Found

Autodiff and `diff { ... }` examples now reach backend codegen but fail Cranelift
verification in programs that combine tensor handles, float reductions, casts,
helper calls, and gradient retrieval.

Failing surfaces:

- `tests/validation/71_tensor_phase5_autodiff.spectra`.
- `tests/validation/82_diff_block_gradient_coverage.spectra`.
- `tests/validation/102_pattern_tensor_ai_composition_stress.spectra` and
  `80_phase14_tensor_language_core.spectra` also include `diff` blocks, but
  have tensor ABI failures that should be fixed first.
- `scripts/validate_r2001_ai_conformance.py` fails its `autodiff` category
  because these two files fail.

### Correction Plan

- After R-112/R-113, rerun the autodiff failures and capture fresh `--dump-ir`
  to avoid fixing stale symptoms.
- Reduce autodiff failures into independent cases:
  - `requires_grad -> mul -> sum_t -> backward -> grad`;
  - `diff { tensor.sum_t(tensor.mul(x, x)) }`;
  - `diff` block calling a helper function returning `Tensor<float, rank0>`;
  - `diff` block wrapping an ML layer loss.
- Audit lowering of `DifferentiableBlock`, tensor hostcall return types, and
  gradient hostcall argument/result typing.
- Ensure `Tensor<float, rank0>` remains a tensor handle, not a raw `float`,
  unless the source explicitly calls `sum_f`/`get_f`.
- Add tests that distinguish:
  - scalar tensor handle (`Tensor<float, rank0>`);
  - scalar float value (`float`);
  - integer cast of a scalar float (`as int`).

### Acceptance

- `tests/validation/71_tensor_phase5_autodiff.spectra` compiles and runs.
- `tests/validation/82_diff_block_gradient_coverage.spectra` compiles and
  runs.
- `scripts/validate_r2001_ai_conformance.py --keep-going` has zero failures in
  the `autodiff` category.
- No `load(void)`, invalid tensor handle cast, or verifier error remains in
  autodiff/diff block IR dumps.

### Completed Evidence

- Direct validation passes for
  `tests/validation/71_tensor_phase5_autodiff.spectra` and
  `tests/validation/82_diff_block_gradient_coverage.spectra`.
- `tests/validation/102_pattern_tensor_ai_composition_stress.spectra` now uses
  explicit `tensor.requires_grad` for the differentiated tensor and runs
  successfully.
- Added composition regressions covering `diff` after ML/tensor setup:
  `117_enum_ml_autodiff_composition.spectra`,
  `118_autodiff_after_ml_without_final_free_all.spectra`, and
  `119_autodiff_after_ml_with_final_free_all.spectra`.
- Validation run: `python scripts\validate_r2001_ai_conformance.py
  --keep-going`.
- Validation run: IR dumps for `102` and `117` contain no `Verifier`,
  `load(void)`, invalid tensor handle, or `cast(...Tensor...)` markers.

## R-115 Tensor Graph Example Codegen and AI Example Conformance

- Status: `complete`
- Priority: `P1`
- Owner: `midend`
- Dependencies: `R-112`, `R-113`, `R-1601`, `R-1602`

### Problems Found

The tensor graph unit tests pass, but runnable `.spectra` AI examples that
exercise graph-optimized elementwise/reduction surfaces originally failed
through the public CLI. The final remaining issue was an invalid runtime
assertion in the elementwise example: a valid positive value in `(0.0, 1.0)` was
truncated to `0` before comparison.

Failing surfaces:

- `examples/ai/tensor_graph_elementwise_fusion.spectra`.
- `scripts/ai_examples_benchmark.py` fails because these examples fail.
- `scripts/validate_r2001_ai_conformance.py` fails its `docs_examples`
  category because the AI example benchmark fails.

After R-112, `examples/ai/tensor_graph_reduction_fusion.spectra` passes; the
remaining public graph example failure is `tensor_graph_elementwise_fusion`,
which compiles but exits at runtime with status `1`.

Completed evidence:

- `examples/ai/tensor_graph_elementwise_fusion.spectra` now validates the
  positive tensor value with a float comparison instead of truncating a value in
  `(0.0, 1.0)` to `0` through `as int`.
- `scripts/ai_examples_benchmark.py` now records `failure_kind` as
  `compile`, `codegen`, `runtime`, `timeout`, `crash`, `unknown`, or `none`,
  and supports `--binary` for validating an already-built CLI binary.
- Validation run: `target\debug\spectralang.exe run
  examples\ai\tensor_graph_elementwise_fusion.spectra`.
- Validation run: `target\debug\spectralang.exe run
  examples\ai\tensor_graph_reduction_fusion.spectra`.
- Validation run: `python scripts\ai_examples_benchmark.py --binary
  target\debug\spectralang.exe --out target/r115-ai-examples.json
  --timeout-seconds 20`.
- Validation run: `python scripts\validate_r2001_ai_conformance.py
  --keep-going`.
- Validation run: `cargo test -p spectra-midend tensor_graph`.

### Correction Plan

- After R-112/R-113, rerun both examples with `--dump-ir` to separate generic
  float-cast/tensor-ABI failures from graph-specific lowering failures.
- Add focused compile/run tests for:
  - `relu -> sqrt_f -> tanh_f`;
  - `relu -> tanh_f -> sum_t`;
  - `stats_kernel_ops()` after graph-eligible chains.
- Ensure graph optimization metadata does not alter the executable IR ABI.
- Extend `scripts/ai_examples_benchmark.py` or add a dedicated R-115 script to
  classify example failures as compile/codegen/runtime/timeout instead of a
  single aggregate failure.

### Acceptance

- `examples/ai/tensor_graph_elementwise_fusion.spectra` runs successfully.
- `examples/ai/tensor_graph_reduction_fusion.spectra` runs successfully.
- `scripts/ai_examples_benchmark.py --out target/r115-ai-examples.json`
  reports all examples passed.
- `scripts/validate_r2001_ai_conformance.py --keep-going` has zero failures in
  the `docs_examples` category.
- `cargo test -p spectra-midend tensor_graph` continues to pass.

## R-116 Stress/Soak Runner Contract and Regression Inputs

- Status: `complete`
- Priority: `P1`
- Owner: `tooling`
- Dependencies: `R-112`, `R-114`, `R-1202`

### Problems Found

The `stress_soak_smoke` gate in `run_tests.ps1` currently calls
`scripts/stress_soak.py` with valid explicit arguments, but direct investigation
also exposed an obsolete/nonexistent `--smoke` workflow expectation. More
importantly, the stress runner includes `tests/validation/71_tensor_phase5_autodiff.spectra`
in both compile and runtime suites, so the smoke gate is expected to fail until
R-114 is fixed.

Failing surfaces:

- `run_tests.ps1` reports `stress_soak_smoke` failed.
- Direct `python scripts/stress_soak.py --smoke` fails because `--smoke` is not
  supported by the CLI.
- The current stress case list includes known failing autodiff inputs.

### Correction Plan

- Decide and document one supported smoke contract:
  - either add `--smoke` as a first-class alias for `--iterations 1` with
    bounded timeout/memory defaults;
  - or remove all references and docs that imply `--smoke` exists.
- Add `--json-out` evidence validation that fails with actionable per-case
  details when a stress case fails.
- Keep failing production cases in the stress suite, but annotate their
  dependency on R-114 until fixed; do not silently skip them.
- After R-114, rerun the compile/runtime/package stress suites with one
  iteration and verify zero failures.

### Acceptance

- The supported smoke invocation is documented and works from the command line.
- `run_tests.ps1` `stress_soak_smoke` passes.
- `target/stress-soak-smoke.json` includes schema, all cases, zero failures,
  elapsed time, timeout status, and memory samples when available.
- The stress suite keeps autodiff/tensor coverage rather than replacing it
  with weaker inputs.

### Completed Evidence

- `run_tests.ps1` `stress_soak_smoke` passes.
- `target/stress-soak-smoke.json` reports `failed = 0`, `iterations = 1`, 13
  case records, and zero timed-out records.
- The smoke suite still includes tensor/autodiff coverage; no production inputs
  were weakened or skipped to pass the gate.

## R-117 Full Suite Failure Classification and Conformance Recovery

- Status: `complete`
- Priority: `P0`
- Owner: `tooling`
- Dependencies: `R-112`, `R-113`, `R-114`, `R-115`, `R-116`, `R-2001`

### Problems Found

The 2026-06-12 `run_tests.ps1` execution finished with 226 expected tests, 215
passing and 11 failing. The failures are actionable but currently spread across
direct validation tests, AI examples, stress/soak, and R-2001 conformance.

Failure set:

- Direct validation failures: `59_import_surface`, `67_tensor_float_surface`,
  `71_tensor_phase5_autodiff`, `80_phase14_tensor_language_core`,
  `82_diff_block_gradient_coverage`, `102_pattern_tensor_ai_composition_stress`,
  `106_import_alias_named_std_stress`.
- AI examples: `tensor_graph_elementwise_fusion`,
  `tensor_graph_reduction_fusion`.
- Gates: `validate_r2001_ai_conformance`, `stress_soak_smoke`.

Current state after R-115:

- `run_tests.ps1` reports 227 expected tests, 227 passing and 0 failing.
- All runner-managed validation tests pass.
- `stress_soak_smoke` passes.
- `validate_r2001_ai_conformance.py --keep-going` reports a certified passing
  candidate.
- `TEST_RESULTS.txt` contains no `FALHOU` entries.
- Direct `spectralang run` of
  `tests/validation/102_pattern_tensor_ai_composition_stress.spectra` now exits
  successfully after reconciling the autodiff contract with explicit
  `tensor.requires_grad`.

Current state after R-113/R-114/R-115 completion:

- `run_tests.ps1` reports 235 expected tests, 235 passing and 0 failing.
- `python scripts\validate_r2001_ai_conformance.py --keep-going` reports a
  certified passing candidate.
- `TEST_RESULTS.txt` contains no `FALHOU` entries.
- All failures tracked in the 2026-06-12 recovery set are either fixed or
  covered by passing regression tests added in this correction cycle.

### Correction Plan

- Add a small parser for `TEST_RESULTS.txt` or structured runner output that
  groups failures by root-cause item (`R-112` through `R-116`).
- After each root-cause fix, rerun only the affected subset first, then the
  full `run_tests.ps1`.
- Keep R-2001 as the final recovery gate: it should remain rejected while any
  required category fails, and pass only after tensor/autodiff/docs examples
  are green.
- Add a final recovery note to this item with the exact full-suite command,
  pass/fail counts, and conformance report path.

### Acceptance

- `run_tests.ps1` reports zero failed expected tests.
- `scripts/validate_r2001_ai_conformance.py --keep-going` reports
  `candidate_status = accepted` and `certified = true`.
- `TEST_RESULTS.txt` contains no `FALHOU` entries outside intentionally
  informational semantic tests.
- Every failure from the 2026-06-12 report is either fixed or explicitly moved
  to a new tracked item with an acceptance criterion.

## R-118 Stable Core Control Flow Promotion

- Status: `complete`
- Priority: `P0`
- Owner: `frontend`
- Dependencies: `R-106`, `R-103`, `R-105`

### Problems Found

The language still treated `switch`, `unless`, `do-while`, and `loop` as
experimental even though they are core control-flow constructs. Promoting them
surfaced real production defects:

- parser and CLI policy still required `--enable-experimental`
- return-path analysis did not recognize exhaustive `switch` in `unless`
- lowering emitted instructions and fallthrough branches after terminated blocks
- `switch.exit` was emitted even when all switch branches returned, producing an
  unreachable block rejected by IR validation
- `unless` lowering depended on a synthetic `not` instead of directly inverting
  branch targets

### Scope

- Remove the parser gates for `switch`, `unless`, `do-while`, and `loop`.
- Keep `--enable-experimental` accepted as a compatibility no-op.
- Make `spectralang --list-experimental` report no active syntax gates.
- Harden lowering for terminated blocks in loops and switch bodies.
- Update return-path analysis for exhaustive switch/unless/block/if-let flows.
- Add a production regression `.spectra` file that runs all promoted constructs
  through the normal CLI JIT path.

### Acceptance

- `switch`, `unless`, `do-while`, and `loop` parse without
  `--enable-experimental`.
- `spectralang --list-experimental` reports no active experimental syntax gates.
- `tests/validation/120_stable_promoted_control_flow.spectra` runs successfully.
- Lowering does not emit instructions or branches after terminated control-flow
  blocks.
- Return-path analysis accepts exhaustive promoted-control-flow returns.
- Language reference, maturity policy, CLI help, and planning docs agree on
  stable status.

### Evidence

- Added `tests/validation/120_stable_promoted_control_flow.spectra`.
- Updated parser/cache tests for stable parsing without feature flags.
- Validation commands:
  - `cargo test -q -p spectra-compiler`
  - `cargo test -q -p spectra-midend`
  - `cargo run -q -p spectra-cli -- run tests\validation\120_stable_promoted_control_flow.spectra`
  - `cargo run -q -p spectra-cli -- --list-experimental`

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

- Status: `complete`
- Priority: `P1`
- Owner: `backend`
- Dependencies: `R-201`, `R-202`

### Problem Found

A reduced valid program using `const FLOATY: f64 = 7.75;` followed by
`let truncated: int = FLOATY as int;` used to reach codegen and fail with
`Failed to define function 'main': Compilation error: Verifier errors`.

The root cause was midend lowering: identifier constants inside cast expressions
were lowered as their original constant value, so a float constant could be bound
to an integer-typed local and then compared/returned through invalid IR.

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

- Implemented const cast folding in `midend/src/lowering.rs`.
- Positive regression: `tests/validation/100_float_const_cast_codegen.spectra`.
- Negative regression: `tests/errors/float_const_invalid_cast.spectra`.
- Dedicated gate: `scripts/validate_r205_float_const_cast_codegen.py`.

## R-206 Generic Return Type Enforcement

- Status: `complete`
- Priority: `P0`
- Owner: `semantic`
- Dependencies: `R-204`

### Problems Found

- A generic function declared as `fn bad<T>(value: T) -> string { return value; }`
  used to compile when instantiated with `int`, even though the body returned
  a type parameter incompatible with the declared concrete return type.
- A related invalid generic function declared as returning `int` could reach
  backend codegen and fail with `Verifier errors` instead of semantic analysis.

### Scope

- validate generic function bodies against declared return types before
  specialization/lowering
- validate return type-parameter compatibility before backend
- add semantic diagnostics for generic return mismatches
- add negative regression tests that fail semantically, not in codegen

### Acceptance

- generic functions cannot return unconstrained type parameters where a concrete return type is declared
- invalid generic return mismatches fail during semantic analysis with stable diagnostics
- no invalid generic return mismatch reaches backend codegen or verifier errors

### Evidence

- `tests/errors/generic_return_annotation_mismatch.spectra` now emits one semantic `E004` return mismatch.
- `tests/errors/generic_return_type_mismatch_codegen_guard.spectra` now emits one semantic `E004` return mismatch instead of reaching backend verifier errors.
- The fixed reproducers were removed from `tests/known_issues/`.
- `compiler/tests/stage_smoke.rs` covers both valid `T -> T` return and invalid `T -> string` return.
- `scripts/validate_r206_generic_return_enforcement.py` validates the CLI JSON contract and is integrated into `run_tests.ps1`.
- Focused validation: `cargo test -p spectra-compiler`; `python scripts\validate_r206_generic_return_enforcement.py --binary target\debug\spectralang.exe`.

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

- Status: `complete`
- Priority: `P1`
- Owner: `runtime`
- Dependencies: `R-105`, `R-1202`

### Problem Found

While adding advanced AI examples, a first draft wrote files directly to nested
paths such as `target/ai-examples/advanced-phase16-17/run-a/lock.txt` before
the parent directory existed. The run produced a native process crash instead
of a controlled Spectra diagnostic. The example was rewritten to use already
safe paths, and the runtime behavior is now hardened.

### Scope

- audit `std.fs` host calls for unchecked filesystem failures
- define the contract for missing parent directories in `fs_write`
- ensure filesystem failures become controlled runtime diagnostics or safe
  return values, never native crashes
- add regression coverage for nested paths, invalid paths, and overwrite cases
- allow examples and future AI artifact pipelines to use nested output
  directories safely

### Acceptance

- `std.fs.fs_write` on nested missing parent directories creates parent directories and never native-crashes
- `std.fs.fs_append` follows the same parent-directory behavior as `fs_write`
- invalid textual paths and blocked parent paths return safe `false` values
- regression tests cover nested paths, invalid paths, append, and existing-file overwrite behavior
- AI examples may safely write nested artifact paths without precreating directories

### Evidence

- Found while testing `examples/ai/advanced_phase16_17_training_memory_pipeline.spectra`.
- Runtime implementation: `runtime/src/stdlib/mod.rs`.
- Direct runtime regressions: `fs_write_append_and_overwrite_create_nested_parents` and `fs_invalid_paths_return_safe_values_without_panicking`.
- Spectra regression: `tests/validation/111_fs_path_safety.spectra`.
- Dedicated runner gate: `scripts/validate_r1203_fs_path_safety.py`.

## R-1204 Option and Result Unwrap Host Call Safety

- Status: `complete`
- Priority: `P1`
- Owner: `runtime`
- Dependencies: `R-105`, `R-1202`

### Problem Found

`std.option.option_unwrap`, `std.result.result_unwrap`, and
`std.result.result_unwrap_err` used native `panic!` when called on the wrong
variant. The FFI dispatcher can catch some panics, but direct host-function
invocation and production embedding should not rely on unwinding through
runtime code for ordinary invalid argument cases.

### Scope

- replace panic-based wrong-variant handling with stable host status returns
- preserve successful payload extraction for valid `Some`, `Ok`, and `Err`
  values
- cover null handles as invalid arguments
- update public docs so the contract is runtime error / invalid argument, not
  native panic
- add a dedicated runner gate for the regression

### Acceptance

- `std.option.option_unwrap` on `None` or null handles returns
  `HOST_STATUS_INVALID_ARGUMENT` without native panic
- `std.result.result_unwrap` on `Err` or null handles returns
  `HOST_STATUS_INVALID_ARGUMENT` without native panic
- `std.result.result_unwrap_err` on `Ok` or null handles returns
  `HOST_STATUS_INVALID_ARGUMENT` without native panic
- valid unwrap paths still return the payload with `HOST_STATUS_SUCCESS`
- regression tests and runner gate cover both invalid and valid unwrap paths

### Evidence

- Runtime implementation: `runtime/src/stdlib/mod.rs`.
- Direct runtime regression:
  `stdlib::tests::option_result_unwrap_wrong_variant_returns_host_status`.
- Dedicated runner gate: `scripts/validate_r1204_std_unwrap_safety.py`,
  integrated into `run_tests.ps1`.

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

- Status: `complete`
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

- `docs/release-channels.md` documents nightly, beta, stable, compatibility
  levels, deprecation, migration guidance, and CLI/package metadata.
- `spectralang release-info [--json] [--root <path>]` reports CLI and package
  release metadata with schema `spectralang.release-info.v1`.
- `spectra.toml` supports `[release]` with `channel`, `compatibility`,
  `deprecated_since`, and `migration`.
- `spectralang new` scaffolds explicit nightly release metadata.
- `spectralang package lock` persists channel, compatibility, deprecation, and
  migration metadata into `spectra.lock`.
- `spectralang package publish` persists channel and compatibility metadata in
  registry `package.toml`.
- Deprecated packages emit `warning[release-deprecated]` with migration guidance.
- `scripts/validate_r2002_release_channels.py` validates CLI metadata, package
  metadata, lockfile metadata, registry metadata, scaffold metadata, and
  deprecation warnings.

### Validation

- `python scripts\validate_r2002_release_channels.py --binary target\debug\spectralang.exe`
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

---

# Next Horizon: Native API Platform

The baseline roadmap through Phase 20 is complete. The phases below define a
new, production-grade workstream that turns SpectraLang into a first-class
language for **building HTTP and event-driven APIs natively**, without
sacrificing the AI/ML and tensor story.

The platform is delivered as a separate Spectra package, `spectra.api`,
published through the existing Phase 9 registry. It is **not** part of `std`
because it evolves on a faster cadence, has its own version, and pulls in
heavier optional dependencies (TLS, drivers, observability exporters).

The eight phases (Phase 21 to Phase 28) are ordered by dependency. Phase 21
is the foundation: without async/await as a first-class language feature,
the API library cannot match the latency and concurrency characteristics
that production teams expect.

The companion strategic document (`docs/production-ai-implementation-plan.md`)
has a new top-level chapter "API Platform Vision" that mirrors these phases.

---

# Phase 21: Async Language Core

The foundation of the API platform. Async/await becomes a first-class
language and runtime model, with a platform-specific reactor and
deterministic structured concurrency.

## R-2101 ADR: Async/Await Execution Model

- Status: `complete`
- Priority: `P0`
- Owner: `frontend` / `ecosystem`
- Risk: `high`
- Dependencies: none

### Scope

Decide the asynchronous execution model for Spectra: stackless coroutines vs
stackful, polling vs callback, `Task<T>` / `Stream<T>` types, cancellation,
pinning, and `Send` / `Sync` rules.

### Acceptance

- ADR `docs/adr/0010-async-execution-model.md` is committed and approved.
- The ADR fixes the syntax surface for `async fn`, `await`, `Task<T>`,
  `Stream<T>`, and any `Pin`-style API.
- The ADR covers lowering to a state-machine SSA and the runtime scheduler
  interface.
- The ADR addresses `Send`/`Sync` rules, structured concurrency, and
  cancellation propagation.

### Acceptance Evidence

- `docs/adr/0010-async-execution-model.md` is accepted and freezes the
  stackless polling model, public syntax, `Task<T>`, `Stream<T>`, internal
  pinning policy, state-machine SSA lowering, scheduler ABI, structured
  concurrency, cancellation propagation, and `Send`/`Sync` rules.
- `scripts/validate_r2101_async_adr.py` validates that the required ADR
  decisions and roadmap/backlog status stay synchronized.
- `run_tests.ps1` includes the R-2101 validation gate before Phase 12 and
  later release checks.

## R-2102 Async fn and Async Block in Frontend

- Status: `complete`
- Priority: `P0`
- Owner: `frontend`
- Risk: `high`
- Dependencies: `R-2101`

### Scope

Parse and represent `async fn`, async block expressions, and async closures
in the lexer, parser, and AST.

### Acceptance

- `async fn` is parsed in function declarations and method declarations.
- `async { ... }` is parsed as an async block expression.
- The AST preserves async markers on function, method, trait method, and
  closure nodes and represents `async { ... }` as `AsyncBlock`.
- The parser produces actionable diagnostics for missing or misplaced
  `async` / `await`.
- Snapshot tests cover the happy path and a representative set of malformed
  inputs.

### Completed in this pass

- Lexer/parser recognize `async` and `await` keywords.
- `async fn` is represented by `is_async` on functions, inherent methods,
  trait methods, and trait impl methods.
- `async { ... }` is represented as `ExpressionKind::AsyncBlock`; async
  closures preserve `is_async` on `ExpressionKind::Lambda`.
- Language-service and LSP signatures render async functions and methods as
  `async fn ...`.
- `await` outside async contexts emits parser diagnostic `P006`; `await`
  inside async contexts is implemented by `R-2103`.
- Validation: `cargo test -q -p spectra-compiler`, `cargo check -q`, and
  `python scripts\validate_r2102_async_frontend.py`.

## R-2103 Await Expression and Async Lowering

- Status: `complete`
- Priority: `P0`
- Owner: `frontend` / `midend`
- Risk: `high`
- Dependencies: `R-2102`

### Scope

Add `await` as a first-class expression and lower async functions and
blocks to a state-machine SSA that integrates with the runtime scheduler.

### Acceptance

- `await <expr>` parses and type-checks as a suspend point.
- Async functions lower to SSA with explicit suspend/resume markers.
- Cranelift backend compiles the lowered state machine for a minimal async
  function.
- Structured async tests cover happy path, early return, and explicit
  cancellation.

### Completed in this pass

- `await <expr>` is parsed as `ExpressionKind::Await` inside async contexts
  and rejected outside async contexts with parser diagnostic `P006`.
- Semantic analysis models `Task<T>` and requires `await` operands to be
  `Task<T>`.
- Async functions, methods, and async blocks lower to `Task<T>` handles with
  explicit `async.suspend`, `async.resume`, and `async.ready` IR markers.
- Runtime host calls `spectra.async.task.ready`, `.poll`, `.result`,
  `.cancel`, and `.is_cancelled` provide the deterministic task baseline used
  by lowering; platform reactor work remains in `R-2104`.
- Cranelift backend compiles `Task<T>` as an ABI handle and accepts the async
  marker instructions.
- Validation: `cargo test -q`, `python scripts\validate_r2103_async_lowering.py`,
  and the checked fixtures `tests/validation/121_async_await_lowering.spectra`,
  `tests/validation/122_async_early_return.spectra`, and
  `tests/errors/await_requires_task.spectra`.

## R-2104 Event Loop Multiplexer (epoll/IOCP/kqueue)

- Status: `complete`
- Priority: `P0`
- Owner: `runtime`
- Risk: `high`
- Dependencies: `R-2103`

### Scope

Implement the runtime reactor that drives async tasks, with
platform-specific backends for `epoll` (Linux), `IOCP` (Windows), and
`kqueue` (macOS).

### Acceptance

- The runtime detects the platform and selects the matching backend
  automatically.
- A focused test exercises 10k concurrently suspended tasks on Linux.
- Task wakeups, timer events, and I/O events share a single reactor
  interface.
- The reactor is documented in the runtime crate with platform-specific
  notes.

### Completed in this pass

- Added `runtime::reactor` with automatic backend selection labels for Linux
  `epoll`, Windows `IOCP`, macOS/BSD `kqueue`, and a portable fallback,
  backed by `mio::Poll` for the platform readiness multiplexer.
- Added one reactor interface for task wakeups, timer readiness, and I/O
  readiness registration/notification.
- Connected `spectra.async.task.ready` and `spectra.async.task.cancel` to the
  global reactor wake queue.
- Added host calls under `spectra.async.reactor.*` for backend inspection,
  wakeups, timers, I/O registration/notification, polling, last-event metadata,
  stats, and reset.
- Added runtime crate documentation in `runtime/src/reactor/mod.rs` covering
  platform-specific backend notes and the scheduler/reactor boundary.
- Validation: `cargo test -q -p spectra-runtime reactor`,
  `cargo test -q -p spectra-runtime async_reactor`,
  `python scripts\validate_r2104_reactor.py`, and the checked fixture
  `tests/validation/123_async_reactor_ready_tasks.spectra`.

### Remaining outside this item

- `R-2105` owns `CancelHandle`, timeout APIs, structured task scopes, and
  parent-child cancellation propagation.
- `R-2107` owns public async filesystem, TCP, UDP, and channel APIs on top of
  this reactor boundary.

## R-2105 Cancellation, Timeouts, and Structured Concurrency

- Status: `complete`
- Priority: `P0`
- Owner: `runtime`
- Risk: `high`
- Dependencies: `R-2104`

### Scope

Add `CancelHandle`, `with_timeout`, scope-based join, and parent-child
cancellation for async tasks.

### Acceptance

- Cancelling a parent task cancels every child task in the scope.
- `with_timeout(duration)` cancels the wrapped future deterministically
  when the deadline elapses.
- `JoinHandle<T>` returns the task result or a structured cancellation
  error.
- Deterministic tests cover cascading cancellation, timeout, and join
  ordering.
- `scripts\validate_r2105_structured_concurrency.py` passes and is wired
  into `run_tests.ps1`.

### Completed in this pass

- Added deterministic runtime host calls for `JoinHandle` value/error status,
  `CancelHandle` creation and cancellation, `with_timeout`, logical scheduler
  time advancement, parent and child scopes, scope attachment/spawn,
  cascading scope cancellation, scope join status, joined-count reporting,
  failure aggregation, and stable per-task join ordering.
- `Task` result and poll host calls now observe cancellation, timeout, and
  structured failure state before returning a value.
- Added unit coverage for cascading parent-scope cancellation, nested child
  scopes, timeout expiry, cancellation handles, structured join failure, and
  deterministic join ordering.
- Added `tests/validation/124_async_structured_concurrency_surface.spectra`
  and the R-2105 validation gate.

### Remaining outside this item

- `R-2106` owns the first-class `Stream<T>` type and stream adaptors.
- `R-2107` owns public async filesystem, TCP, UDP, and channel APIs built on
  the structured task/runtime surface.

## R-2106 Stream Type and Stream Adaptors

- Status: `complete`
- Priority: `P1`
- Owner: `runtime`
- Risk: `medium`
- Dependencies: `R-2105`

### Scope

Add a first-class `Stream<T>` type with `map`, `filter`, `fold`, `take`,
`skip`, `chunks`, `fuse`, and backpressure-aware composition.

### Acceptance

- The public `Stream<T>` API exposes the documented adaptors.
- Slower consumers do not block faster producers (backpressure).
- Stream finish is deterministic when the upstream signals `done`.
- Tests cover happy path, cancellation mid-stream, and
  consumer-faster-than-producer.
- `scripts\validate_r2106_streams.py` passes and is wired into
  `run_tests.ps1`.

### Completed in this pass

- Added runtime-managed `Stream<T>` handles with `next(stream)` returning a
  task, explicit next-status reporting, finite source streams, producer
  capacity, non-blocking backpressure status, deterministic `done`, and stream
  cancellation.
- Added the documented adaptor host-call surface: `map`, `filter`, `fold`,
  `take`, `skip`, `chunks`, and `fuse`.
- Added deterministic runtime coverage for adaptor composition, finite stream
  finish, fused completion, fold, backpressure, consumer-faster-than-producer
  pending tasks, and cancellation of a pending consumer.
- Added `tests/validation/125_async_stream_surface.spectra` and the R-2106
  validation gate.

### Remaining outside this item

- `R-2107` owns public async filesystem, TCP, UDP, and channel APIs.

## R-2107 Async Standard Library Surface

- Status: `complete`
- Priority: `P0`
- Owner: `runtime`
- Risk: `high`
- Dependencies: `R-2105`, `R-2106`

### Scope

Expose async counterparts for filesystem, TCP, UDP, and channel operations
through the standard library.

### Acceptance

- `fs.read_async` and `fs.write_async` are available with cancellation
  support.
- `tcp.connect_async`, `tcp.accept_async`, and `udp` operations are
  exposed.
- Async `channel.send` / `channel.recv` are available alongside the
  existing sync channels.
- Tests cover async socket reads/writes and clean cancellation.
- `scripts\validate_r2107_async_stdlib.py` passes and is wired into
  `run_tests.ps1`.

### Completed in this pass

- Added async filesystem host calls for `fs.read_async` and
  `fs.write_async`, returning cancelable `Task` handles and structured failed
  tasks on I/O failure.
- Added TCP listener/connect/accept/read/write/close host calls with
  nonblocking sockets, pending accept/read tasks, local loopback test support,
  and clean cancellation of pending reads.
- Added UDP bind/port/send_to/recv/close host calls with pending receive tasks
  and deterministic one-byte datagram delivery for runtime-level validation.
- Added async channels with bounded capacity, nonblocking send/recv tasks,
  pending send/recv queues, close semantics, length reporting, and
  cancellation of pending channel operations.
- Added `tests/validation/126_async_stdlib_surface.spectra` and the R-2107
  validation gate.

### Remaining outside this item

- `R-2108` owns async trait objects and `dyn Future`.
- `R-2109` owns async test runtime and macros.

## R-2108 Async Trait Objects and dyn Future

- Status: `complete`
- Priority: `P1`
- Owner: `semantic`
- Risk: `medium`
- Dependencies: `R-2103`

### Scope

Support object-safe async trait methods and `Box<dyn Future>` /
`Box<dyn Stream>` for dynamic dispatch.

### Acceptance

- `dyn Future` and `dyn Stream` compile and dispatch through virtual
  tables.
- Async methods in traits follow the documented object-safety rules.
- Non-object-safe async trait methods emit a stable diagnostic with the
  offending method.
- `scripts\validate_r2108_async_trait_objects.py` passes and is wired into
  `run_tests.ps1`.

### Completed

- Added built-in `Future` and `Stream` trait signatures to the parser,
  semantic analyzer, and midend lowering.
- `Box<dyn Future>` and `Box<dyn Stream>` now lower to the existing
  `dyn Trait` fat-pointer/vtable representation.
- Async trait method vtables now preserve `Task<T>` return types, so
  `await future.poll()` and `await stream.next()` lower through
  `call_indirect` and async task host calls.
- Non-object-safe async trait methods emit stable diagnostic `E2108` with
  the offending method name.
- Added `tests/validation/129_async_trait_objects_future_stream.spectra`,
  `tests/errors/async_trait_object_safety.spectra`, and the R-2108
  validation gate.

## R-2109 Async Test Runtime and Test Macros

- Status: `complete`
- Priority: `P1`
- Owner: `tooling`
- Risk: `medium`
- Dependencies: `R-2107`

### Scope

Provide `#[spectra_async_test]` plus a `block_on` runtime so API test
code can use plain `async fn` without manual setup.

### Acceptance

- `#[spectra_async_test]` runs async test functions inside the Spectra
  test runner.
- `block_on(future)` is available in test code without external setup.
- Tests can be filtered, listed, and reported like synchronous tests.
- `scripts\validate_r2109_async_test_runtime.py` passes and is wired
  into `run_tests.ps1`.

### Completed Notes

- Added parser/AST support for function attributes, including
  `#[spectra_async_test]`.
- Added builtin `block_on(Task<T>) -> T` semantic inference and IR lowering
  through the existing async task result host call.
- Replaced `package test`'s check-only behavior with async test discovery,
  generated wrappers, JIT execution, `--list`, `--filter`, and JSON reporting.
- Added `tests\validation\130_async_test_runtime_block_on.spectra` and
  `tests\projects\valid\async_test_runtime`.

## R-2110 Async Diagnostics and Send/Sync Validation

- Status: `complete`
- Priority: `P0`
- Owner: `semantic`
- Risk: `high`
- Dependencies: `R-2103`

### Scope

Emit stable semantic diagnostics for `Send` / `Sync` violations across
await points, `RefCell` held across await, and other async borrow errors.

### Acceptance

- Stable diagnostic codes `E2101` through `E2120` are documented.
- Non-`Send` types held across await produce a precise semantic diagnostic
  with span.
- `!Send` types crossing task boundaries are reported before runtime.
- Regression tests cover each diagnostic family.
- `scripts\validate_r2110_async_send_sync.py` passes and is wired into
  `run_tests.ps1`.

### Completed Notes

- Added semantic async Send/Sync event analysis for `async fn` bodies.
- Documented the stable async diagnostic range `E2101` through `E2120`.
- Implemented `E2101` for non-`Send` values live across `await`, `E2102`
  for `RefCell`/interior-mutable values held across `await`, and `E2103`
  for `!Send` values crossing spawn-style task boundaries.
- Added regression fixtures:
  `tests\errors\async_non_send_across_await.spectra`,
  `tests\errors\async_refcell_across_await.spectra`,
  `tests\errors\async_non_send_task_boundary.spectra`, and
  `tests\validation\131_async_send_sync_valid.spectra`.

## R-2111 Async Benchmarks and Profiling

- Status: `complete`
- Priority: `P2`
- Owner: `tooling`
- Risk: `low`
- Dependencies: `R-2107`

### Scope

Add an async benchmark harness that emits p50/p95/p99 latency, throughput,
and concurrent connection counts for async workloads.

### Acceptance

- `spectralang bench --async` runs async micro-benchmarks and emits JSON.
- The suite covers 1k, 10k, and 100k concurrent tasks.
- The JSON report is machine-readable and is compared against a checked-in
  baseline.

### Completed Notes

- Added `spectralang bench --async` for the Phase 21 async runtime benchmark
  suite.
- The report schema `spectra.r2111.async_benchmark.v1` emits p50/p95/p99
  latency, throughput, concurrent task counts, concurrent connection counts,
  sample counts, and full-task-set checksums.
- The suite covers 1k, 10k, and 100k concurrent async tasks.
- Added runtime host calls for production benchmark support:
  `spectra.async.task.ready_batch` and
  `spectra.async.task.batch_checksum`.
- Added checked-in regression thresholds at
  `docs/performance/r2111-async-benchmark-baseline.json`.
- Added `scripts/validate_r2111_async_bench.py` and wired it into
  `run_tests.ps1`.

## R-2112 Formal Send/Sync Trait Bounds

- Status: `complete`
- Priority: `P1`
- Owner: `semantic`
- Risk: `medium`
- Dependencies: `R-2108`, `R-2110`

### Scope

Add first-class `Send` and `Sync` trait bounds to generic, async, task, and
dynamic trait object typing so async safety is expressed by the type system
instead of only by explicit type-family classification.

This item closes the known gap left after `R-2110`: until this lands, the
compiler uses explicit classification for families such as `RefCell`, `Cell`,
`Rc`, `NonSend`, and `LocalOnly`, plus recursive struct-field checks. Formal
bounds must let users and libraries express the contract directly as
`T: Send`, `T: Sync`, `dyn Trait + Send`, and `dyn Trait + Sync`.

### Acceptance

- `T: Send` and `T: Sync` bounds parse, type-check, and participate in
  generic trait-bound validation.
- `dyn Trait + Send` and `dyn Trait + Sync` are represented in the AST,
  semantic type model, and lowering without breaking existing `dyn Trait`
  code.
- `Task<T>: Send` and spawn-style APIs require formal `Send` evidence instead
  of name-family-only classification.
- Diagnostics reuse the `E2101` through `E2120` async range with precise spans
  for missing `Send`/`Sync` evidence.
- Regression tests cover generic bounds, dyn trait bounds, task boundary
  checks, and backwards-compatible plain `dyn Trait` behavior.

### Completed Notes

- Extended AST, parser, semantic type conversion, LSP/tooling formatters, and
  midend lowering so `dyn Trait + Send` and `dyn Trait + Send + Sync` are
  represented end-to-end while plain `dyn Trait` remains valid.
- `T: Send` and `T: Sync` now participate in generic bound validation as
  formal auto-trait evidence.
- Async Send validation no longer treats unconstrained type parameters as
  implicitly `Send`; plain `dyn Trait` remains backward-compatible with the
  existing async trait-object model, while `dyn Trait + Send/Sync` carries
  explicit evidence for APIs that require it.
- Added semantic `Sync` classification for formal `T: Sync` checks.
- Added diagnostic `E2104` for missing formal `Send`/`Sync` evidence in
  generic bounds and `dyn Trait + Send/Sync` casts.
- Added regression fixtures:
  `tests\validation\132_formal_send_sync_bounds.spectra`,
  `tests\errors\formal_send_bound_missing_across_await.spectra`,
  `tests\errors\formal_task_boundary_missing_send.spectra`,
  `tests\errors\formal_send_bound_rejects_non_send.spectra`,
  `tests\errors\formal_sync_bound_rejects_refcell.spectra`, and
  `tests\errors\dyn_trait_send_bound_missing.spectra`.
- Added `scripts/validate_r2112_formal_send_sync_bounds.py` and wired it into
  `run_tests.ps1`.

---

# Phase 22: API Library Foundation

The library itself: HTTP/1.1 server and client, JSON, routing, TLS, and
the public `std.api.*` surface. The package is published to the local
registry as `spectra.api`.

## R-2201 ADR: API Library Architecture

- Status: `complete`
- Priority: `P0`
- Owner: `ecosystem`
- Risk: `high`
- Dependencies: `R-2107`

### Scope

Decide the structural model for the new `spectra.api` library: a separate
Rust companion crate, a Spectra package, the `std.api.*` binding surface,
and the relationship with `std`.

### Acceptance

- ADR `docs/adr/0011-api-library-architecture.md` is committed and
  approved.
- The ADR fixes the crate layout, the package name, the import path, and
  the public API surface.
- The ADR documents the migration path from any prior ad-hoc `std` web
  modules.
- The ADR identifies the supported HTTP versions, TLS model, and async
  dependencies.

### Completed Notes

- Added accepted ADR
  `docs/adr/0011-api-library-architecture.md`.
- Fixed the public package name as `spectra.api`, the public import path as
  `std.api.*`, and the host-call prefix as `spectra.api.*`.
- Fixed the implementation/package layout:
  `packages/spectra-api`, `packages/spectra-api/spectra.toml`,
  `packages/spectra-api/src/*.spectra`, `runtime/src/api/`, `docs/api/`,
  and `examples/api/`.
- Fixed Phase 22 as HTTP/1.1 first, with HTTP/2 and HTTP/3 deferred to the
  Phase 24 roadmap items.
- Accepted `rustls` as the default TLS backend and rejected OpenSSL as the
  default implementation.
- Confirmed `spectra.api` uses the Phase 21 async model (`Task<T>`,
  `Stream<T>`, cancellation, and reactor integration) without exposing a
  public Tokio runtime dependency.
- Documented migration rules from any future ad-hoc `std` web modules and
  clarified that `std.serve` remains a local model-serving harness, not the
  HTTP API server.
- Reassigned `R-2202` to owner `web` because it owns public API host-call
  surface, not generic runtime infrastructure.
- Added `scripts/validate_r2201_api_adr.py` and wired it into
  `run_tests.ps1`.

## R-2202 spectra-api Rust Crate and Host Call Registration

- Status: `complete`
- Priority: `P0`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2201`

### Scope

Add a new workspace crate `spectra-api` that hosts the Rust implementation
of HTTP parsing, server, client, JSON, TLS, and registers the host calls
that `std.api.*` will dispatch into.

### Acceptance

- The new crate compiles in the workspace and links against the existing
  runtime.
- Host calls are registered in the runtime host-call registry.
- The crate has unit tests for the registered host functions.
- A focused script verifies host call naming and registration count.

### Completed Implementation

- Added the `packages/spectra-api` Rust crate and the `spectra.api` package
  manifest at `packages/spectra-api/spectra.toml`.
- Added 28 `spectra.api.*` host calls covering the Phase 22 registration
  surface for version metadata, HTTP method/status/header helpers, request and
  response handles, server/client handles, JSON classification, TLS config
  handles, routing handles, and error metadata.
- Host calls are registered in the existing runtime host-call registry through
  `spectra_api::register()`, and the crate exports
  `spectra_api_register_host_calls` for native integration.
- Added `runtime/src/api/mod.rs` as the runtime-side namespace contract for the
  required `spectra.api.*` host calls.
- Wired `spectra_api::register()` into the CLI runtime setup paths after
  `spectra_runtime::register_standard_library()`.
- Added unit coverage in `cargo test -p spectra-api` for host-call uniqueness,
  registry insertion/idempotence, callable registered functions, handle-backed
  state, string-based header validation, and runtime missing-call behavior.
- Added `scripts/validate_r2202_spectra_api_hostcalls.py` and wired it into
  `run_tests.ps1`.

## R-2203 std.api Surface in Semantic Analysis

- Status: `complete`
- Priority: `P0`
- Owner: `semantic`
- Risk: `high`
- Dependencies: `R-2202`

### Scope

Expose `std.api.*` to the semantic analyzer with the public function
signatures, struct types, and trait surface declared by the
`spectra.api` package.

### Acceptance

- `std.api.*` is visible in the formatter, LSP completion, and
  `spectralang --list-experimental` remains free of experimental syntax
  gates.
- Type checking resolves qualified `std.api.*` calls without false
  missing-module diagnostics.
- Snapshot tests cover the public function table.
- `tests/semantic/std_api_surface.spectra` validates the CLI check and
  formatter path.
- `scripts/validate_r2203_std_api_surface.py` passes and is wired into
  `run_tests.ps1`.

### Completed

- Added the virtual `std.api` module family in
  `compiler/src/semantic/builtin_modules.rs`, covering HTTP, server, client,
  JSON, TLS, routing, and API error surfaces with public function signatures
  and exported API handle types.
- Seeded the semantic namespace table for `std.api.*` and `spectra.std.api.*`
  so qualified calls do not produce false missing-module diagnostics.
- Added LSP completion items backed by the same public function/type/module
  table used by semantic analysis.
- Added `compiler/tests/snapshots/std_api_public_function_table.snap`,
  `compiler/tests/stage_smoke.rs` semantic coverage, and
  `tests/semantic/std_api_surface.spectra`.
- Added `scripts/validate_r2203_std_api_surface.py` and wired it into
  `run_tests.ps1`.

### Boundary

- `R-2203` does not claim HTTP parser, server/client runtime execution, or
  request routing execution. Those remain owned by `R-2204+`.

## R-2204 HTTP/1.1 Parser

- Status: `complete`
- Priority: `P0`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2107`, `R-2202`

### Scope

Implement a streaming HTTP/1.1 request and response parser, including
request line, status line, headers, chunked transfer encoding, and
keep-alive.

### Acceptance

- The parser produces structured request and response values with headers
  and body chunks.
- Chunked transfer encoding round-trips through the parser in both
  directions.
- Malformed input returns a typed parse error with the offending position.
- Tests cover representative RFC 7230 samples and known-malformed inputs.
- `cargo test -p spectra-api` covers `Http1Parser`, `ParsedRequest`,
  `ParsedResponse`, `HttpBody`, and `ParseError`.
- `scripts/validate_r2204_http1_parser.py` passes and is wired into
  `run_tests.ps1`.

### Completed

- Added a streaming `Http1Parser` in `packages/spectra-api/src/http.rs` with
  separate request and response modes, incremental buffering, pipelined
  message consumption, configurable header/body/chunk limits, and
  keep-alive detection.
- Added structured HTTP data types: `HttpVersion`, `Header`, `BodyChunk`,
  `HttpBody`, `ParsedRequest`, `ParsedResponse`, `ParseErrorKind`, and
  `ParseError`.
- Added parsing and serialization helpers for complete request/response
  buffers, including chunked transfer coding with extensions and trailers.
- Added typed malformed-input handling for invalid start lines, methods,
  targets, versions, statuses, headers, content lengths, unsupported transfer
  codings, invalid chunk sizes, invalid chunk terminators, and size-limit
  violations.
- Added crate tests for streaming input, pipelined requests, RFC 7230-style
  response samples, chunked request/response round-trips, keep-alive behavior,
  malformed headers, malformed chunks, conflicting content lengths, and
  unsupported transfer encodings.
- Added `scripts/validate_r2204_http1_parser.py` and wired it into
  `run_tests.ps1`.

### Boundary

- `R-2204` provides the protocol parser and serializer. Network accept loops,
  request body limit enforcement at the connection layer, response writers,
  and server/client timeout behavior remain owned by `R-2205` and `R-2206`.

## R-2205 HTTP/1.1 Server

- Status: `complete`
- Priority: `P0`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2204`

### Scope

Implement the HTTP/1.1 server: accept loop, connection state, request
body limits, response writer, and per-connection timeouts.

### Acceptance

- An end-to-end test exercises GET, POST with body, chunked responses, and
  HEAD.
- Body size limits and slowloris protections are enforced.
- The server cleans up connections on timeout, body-limit violation, and
  parse error.
- The server survives 10k concurrent connections on the local test
  machine.
- `cargo test -p spectra-api` covers `HttpServer`, `ServerConfig`,
  `ServerResponse`, and `ServerStats`.
- `scripts/validate_r2205_http1_server.py` passes and is wired into
  `run_tests.ps1`.

### Completed

- Added a nonblocking HTTP/1.1 server in
  `packages/spectra-api/src/server.rs` using the `R-2204` parser for request
  framing and response serialization.
- Added public server API types: `ServerConfig`, `ServerResponse`,
  `ServerStats`, `ServerError`, `Handler`, and `HttpServer`.
- Implemented accept-loop connection state, keep-alive handling, response
  writing, per-connection read and idle timeouts, configured max body/header
  limits, configured max connection limits, and cleanup on timeout,
  body-limit violation, parse error, normal shutdown, and dropped server
  handles.
- Preserved the existing Phase 22 host calls `spectra.api.server.new`,
  `spectra.api.server.state`, and `spectra.api.server.shutdown`.
- Added crate tests for GET, POST with body, chunked responses, HEAD,
  body-limit rejection, slowloris timeout, parse-error cleanup, and the 10k
  connection-slot limiter.
- Added `scripts/validate_r2205_http1_server.py` and wired it into
  `run_tests.ps1`.

### Boundary

- `R-2205` owns the server-side accept loop and response path. HTTP client
  connection pooling, redirects, and client timeout semantics remain owned by
  `R-2206`.

## R-2206 HTTP/1.1 Client

- Status: `complete`
- Priority: `P0`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2204`

### Scope

Implement the HTTP/1.1 client: connection pool, redirect handling,
configurable timeouts, and structured responses.

### Acceptance

- The client supports GET, POST, PUT, PATCH, DELETE, and HEAD with
  arbitrary bodies.
- Redirect chains (up to the configured limit) are followed with the right
  method semantics.
- Timeouts, connection failures, and protocol errors are reported as
  typed errors.
- Tests cover redirect chains, large bodies, and explicit timeout.
- `cargo test -p spectra-api` covers `HttpClient`, `ClientConfig`,
  `ClientRequest`, `ClientResponse`, `ClientError`, redirects, pool reuse,
  large bodies, and timeout behavior.
- `scripts/validate_r2206_http1_client.py` passes and is wired into
  `run_tests.ps1`.

### Completed

- Added a real HTTP/1.1 client in `packages/spectra-api/src/client.rs` using
  the `R-2204` parser for structured responses and the Phase 22 server tests
  for end-to-end validation.
- Added public client API types: `ClientConfig`, `ClientRequest`,
  `ClientResponse`, `ClientErrorKind`, `ClientError`, `ClientStats`, and
  `HttpClient`.
- Implemented GET, POST, PUT, PATCH, DELETE, HEAD, arbitrary request bodies,
  connection pooling with idle expiry, configurable timeout, configurable
  redirect limit, large response bodies, and typed timeout/connection/protocol
  errors.
- Implemented redirect handling for 301, 302, 303, 307, and 308, including
  POST-to-GET conversion for 301/302/303 and method/body preservation for
  307/308.
- Preserved existing Phase 22 host calls `spectra.api.client.new` and
  `spectra.api.client.timeout_ms`.
- Added crate tests for all public methods, arbitrary bodies, pool reuse,
  redirect method semantics, redirect limit, large bodies, explicit timeout,
  connection failure, and protocol error.
- Added `scripts/validate_r2206_http1_client.py` and wired it into
  `run_tests.ps1`.

### Boundary

- `R-2206` implements plain HTTP/1.1 over TCP. HTTPS, certificate validation,
  SNI, and ALPN remain owned by `R-2207`.

## R-2207 TLS via rustls (HTTPS Server and Client)

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2206`, `R-2205`

### Scope

Add HTTPS support using `rustls` for both the server and the client, with
SNI, configurable certificate chains, and ALPN negotiation.

### Acceptance

- An HTTPS server runs a self-signed certificate in the integration test.
- An HTTPS client connects to a known external test endpoint and
  validates the chain.
- ALPN advertises `http/1.1` (and `h2` when Phase 24 lands it).
- TLS handshake failures are reported as typed errors with the underlying
  cause.

## R-2208 std.api.json Encoder and Decoder

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2202`

### Scope

Implement a JSON encoder and decoder that handles primitives, arrays,
maps, null, nested structures, and common escape sequences for the public
API surface.

### Acceptance

- Round-trip tests cover primitives, nested structures, arrays, maps, and
  null.
- Invalid JSON returns a typed parse error with byte offset.
- The encoder produces valid RFC 8259 JSON for all supported values.
- The surface is exposed through `std.api.json.*` and documented in the
  book.

## R-2209 JSON Derive: Serialize and Deserialize

- Status: `not_started`
- Priority: `P0`
- Owner: `frontend` / `semantic`
- Risk: `high`
- Dependencies: `R-2208`

### Scope

Add `#[derive(Serialize, Deserialize)]` to the language so structs and
enums can be encoded/decoded through the JSON runtime.

### Acceptance

- The derive macro generates code that uses `std.api.json.*`.
- Optional fields and explicit renaming are supported.
- Invalid input produces a typed error that points to the failing field.
- Tests cover happy path, missing field, wrong type, and rename.

## R-2210 Request, Response, Header, Cookie, Method, Status Types

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2204`

### Scope

Define the core HTTP types in the public `std.api.*` surface: `Request`,
`Response`, `Header`, `Cookie`, `Method`, and `Status`.

### Acceptance

- The types are usable as handler parameters and return values.
- `Method` and `Status` enumerate the documented values.
- Header and cookie accessors are case-insensitive and validate input.
- Tests cover representative CRUD request/response flows.

## R-2211 Router: Path Matching and Wildcards

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2210`

### Scope

Implement a router that supports literal paths, path parameters
(`{id}`), wildcards (`*`), and optional regex constraints.

### Acceptance

- The router matches `/users`, `/users/{id}`, `/files/*path`, and
  `/orders/{id:\d+}`.
- Path parameters are available in the request handler as typed values.
- Route conflicts (e.g. literal vs parameter) are reported with the
  conflicting paths.
- Tests cover 100k registered routes with sub-millisecond lookup.

## R-2212 Query String Parser and Binding

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2210`

### Scope

Parse query strings and bind them to struct fields, including repeated
keys, arrays, and basic type coercion.

### Acceptance

- Query strings parse to a structured map and to a typed struct when
  bound.
- Repeated keys become arrays; mismatched types produce typed errors.
- URL decoding and reserved character handling are RFC 3986 compliant.
- Tests cover simple, repeated, and malformed queries.

## R-2213 URL-Encoded Form Binding

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2212`

### Scope

Parse `application/x-www-form-urlencoded` bodies and bind them to struct
fields, including arrays and nested objects.

### Acceptance

- Form bodies parse to a typed struct or to a key-value map.
- Duplicate keys produce a typed error with the offending field.
- Missing required fields produce a validation error with the field name.
- Tests cover happy path, malformed input, and field validation failures.

## R-2214 Multipart Form and File Uploads

- Status: `not_started`
- Priority: `P1`
- Owner: `web`
- Risk: `medium`
- Dependencies: `R-2213`

### Scope

Parse `multipart/form-data` bodies, expose file uploads through a
streaming interface, and enforce size and count limits.

### Acceptance

- The parser exposes text parts, file parts, and stream-friendly file
  readers.
- Per-request size limits and per-part count limits are enforced.
- Files are streamed to disk or a sink to avoid loading them entirely in
  memory.
- Tests cover simple forms, multiple files, and oversize rejection.

## R-2215 Handler Trait and Response Return

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2210`

### Scope

Define the `api.handler` trait that user handlers implement and that the
router calls to produce a `Response`.

### Acceptance

- The trait supports `async fn` and synchronous handlers.
- Handlers can return any value that implements `IntoResponse`.
- Errors thrown by handlers flow through the unified error middleware.
- Tests cover both handler shapes and trait object dispatch.

## R-2216 Server Lifecycle, Listen, Serve, and Graceful Shutdown

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2205`, `R-2211`, `R-2215`

### Scope

Wire the server lifecycle: `listen`, `serve`, graceful shutdown on signal,
and clean teardown of in-flight requests.

### Acceptance

- A server can be started on a configured port and shut down
  deterministically on SIGINT/SIGTERM.
- In-flight requests are given a configurable drain timeout before forced
  termination.
- Resources (sockets, pools, timers) are released on shutdown.
- Tests cover signal handling, drain, and post-shutdown refusal of new
  connections.

## R-2217 spectra.api Package Published to Local Registry

- Status: `not_started`
- Priority: `P0`
- Owner: `ecosystem`
- Risk: `medium`
- Dependencies: `R-2203`, `R-2216`

### Scope

Publish the `spectra.api` package to the local Phase 9 registry with a
clear manifest, dependency declarations, and a deterministic version.

### Acceptance

- `spectralang package add spectra-api` resolves from the local registry.
- The manifest pins compatible Spectra and async runtime versions.
- `spectralang package build/check/run` work end-to-end on the published
  package.
- The registry entry includes checksum and source path metadata.

## R-2218 API Book Chapter: Hello HTTP

- Status: `not_started`
- Priority: `P0`
- Owner: `ecosystem`
- Risk: `low`
- Dependencies: `R-2217`

### Scope

Add a `Hello HTTP` chapter to `docs/book/` that walks through defining a
route, returning a typed response, and running the server locally.

### Acceptance

- The chapter is reachable from the book index and from the API reference
  docs.
- The chapter is validated by the existing
  `scripts/validate_ai_book.py`-style validator.
- The example in the chapter runs end-to-end on the local machine.

## R-2219 API Example: REST CRUD

- Status: `not_started`
- Priority: `P0`
- Owner: `ecosystem`
- Risk: `low`
- Dependencies: `R-2217`, `R-2209`

### Scope

Provide a runnable REST CRUD example that exercises routes, JSON, path
params, query strings, and form binding through `spectra.api`.

### Acceptance

- `examples/api/01_rest_crud.spectra` builds and runs.
- The example uses the public `std.api.*` surface and a real local server.
- The example includes a smoke test that asserts the CRUD responses.

## R-2220 API Conformance Suite v0 (HTTP/1.1)

- Status: `not_started`
- Priority: `P0`
- Owner: `tooling`
- Risk: `medium`
- Dependencies: `R-2216`, `R-2217`, `R-2208`

### Scope

Stand up the v0 API conformance suite covering HTTP/1.1 parsing, status
codes, headers, JSON round-trip, and the basic router.

### Acceptance

- `scripts/validate_r2220_api_conformance_v0.py` runs the suite and emits
  a machine-readable report.
- The suite covers the documented must-pass HTTP/1.1 cases and a JSON
  conformance matrix.
- The suite gates `run_tests.ps1` for Phase 22.

---

# Phase 23: Middleware and Security

Production middleware, authentication, authorization, threat mitigation,
and unified error handling. Every item is a real, configurable building
block with documented behavior.

## R-2301 Middleware Chain Trait and Deterministic Ordering

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2215`

### Acceptance

- The middleware trait supports `async fn` and synchronous middleware.
- Middleware order is deterministic and documented in the book chapter.
- The response chain runs in reverse order.
- Tests cover ordering, short-circuit, and post-response hooks.

## R-2302 CORS Middleware (RFC 7231)

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `medium`
- Dependencies: `R-2301`

### Acceptance

- Preflight requests return the correct `Access-Control-*` headers for
  the configured policy.
- Exposed headers, allowed methods, allowed origins, and credentials flag
  are honored.
- Non-preflight requests receive the correct `Access-Control-Allow-Origin`
  header.
- Tests cover permissive, restrictive, and credentialed configurations.

## R-2303 Structured Logging and Request ID Tracing

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `medium`
- Dependencies: `R-2301`

### Acceptance

- Every request gets a unique request ID that flows through the log lines.
- The middleware emits one log line per request with the documented
  fields.
- Log format is configurable (JSON for production, text for development).
- Tests assert the log line contents and the request ID propagation.

## R-2304 Rate Limiting (Token Bucket and Sliding Window)

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2301`

### Acceptance

- The middleware enforces the configured limit and returns `429` when
  exceeded.
- Per-tenant and per-user limits are isolated (one tenant cannot exhaust
  another tenant's budget).
- Configuration is hot-reloadable in dev mode.
- Tests cover token bucket, sliding window, and per-tenant isolation.

## R-2305 Response Compression (gzip, brotli, deflate)

- Status: `not_started`
- Priority: `P1`
- Owner: `web`
- Risk: `medium`
- Dependencies: `R-2301`

### Acceptance

- The middleware negotiates the best supported encoding from the request.
- Small responses below the threshold are not compressed.
- The `Content-Encoding` and `Vary` headers are set correctly.
- Tests cover each encoding and the threshold behavior.

## R-2306 Security Headers Middleware

- Status: `not_started`
- Priority: `P1`
- Owner: `web`
- Risk: `low`
- Dependencies: `R-2301`

### Acceptance

- The middleware applies the configured header policy on every response.
- CSP and Permissions-Policy are configurable per route.
- HSTS preload and `includeSubDomains` are honored when configured.
- Tests assert each header is present and correctly configured.

## R-2307 API Key Authentication

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `medium`
- Dependencies: `R-2301`

### Acceptance

- The middleware extracts the key from the configured source.
- Expired or revoked keys are rejected with `401` and a structured error.
- The middleware can be combined with rate limiting per key.
- Tests cover valid, invalid, expired, and revoked keys.

## R-2308 JWT (HS256, RS256, ES256)

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2202`

### Acceptance

- Tokens can be signed and verified with each documented algorithm.
- Claims validation rejects expired, not-yet-valid, and wrong-issuer
  tokens.
- The verifier is constant-time for signature comparison.
- Tests cover happy path, expiry, wrong issuer, and tampered payload.

## R-2309 OAuth2 Client (Authorization Code + PKCE + Refresh)

- Status: `not_started`
- Priority: `P1`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2308`

### Acceptance

- The client follows the authorization code flow and exchanges the code
  for tokens.
- PKCE is generated and verified end-to-end.
- Refresh tokens are exchanged for new access tokens.
- Tests cover happy path, refresh, and revocation.

## R-2310 OAuth2 Resource Server and Token Introspection

- Status: `not_started`
- Priority: `P2`
- Owner: `web`
- Risk: `medium`
- Dependencies: `R-2308`

### Acceptance

- JWT bearer tokens are validated locally with the configured JWKS.
- Opaque tokens are validated via RFC 7662 introspection.
- The resource server emits `WWW-Authenticate` on rejection.
- Tests cover JWT, opaque, and revoked tokens.

## R-2311 Session Management

- Status: `not_started`
- Priority: `P1`
- Owner: `web`
- Risk: `medium`
- Dependencies: `R-2312`

### Acceptance

- Sessions are stored in a pluggable store (memory, Redis) with a
  defined interface.
- Sliding expiration extends the session on activity up to a maximum
  lifetime.
- Explicit logout invalidates the session immediately.
- Tests cover creation, lookup, expiry, sliding, and invalidation.

## R-2312 Cookie API (Secure, httpOnly, SameSite, Signed)

- Status: `not_started`
- Priority: `P1`
- Owner: `web`
- Risk: `low`
- Dependencies: `R-2210`

### Acceptance

- Cookies can be set with the documented attributes and read back as a
  typed value.
- Signed cookies are verified with constant-time comparison.
- Invalid signatures or expired cookies are rejected with a typed error.
- Tests cover each attribute and the signature verification paths.

## R-2313 Request Validation (Constraints, RFC 7807)

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2209`

### Acceptance

- Field-level constraints (required, length, range, regex) are applied to
  validated structs.
- Failed validation returns a `422` with an RFC 7807 body listing the
  offending fields.
- The validation framework composes with the JSON derive.
- Tests cover each constraint and the response shape.

## R-2314 Unified Error Handling and Exception Middleware

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2301`

### Acceptance

- The public `api.Error` type maps to HTTP status codes and bodies
  deterministically.
- Internal errors are logged with full detail and produce sanitized
  public responses.
- The middleware can be customized per route.
- Tests cover the default mapping, the custom mapping, and the
  sanitization.

## R-2315 HTTPS Hardening (HSTS Preload, OCSP Stapling)

- Status: `not_started`
- Priority: `P1`
- Owner: `web`
- Risk: `medium`
- Dependencies: `R-2207`

### Acceptance

- The server emits
  `Strict-Transport-Security: max-age=...; preload; includeSubDomains` when
  configured.
- OCSP responses are stapled to the TLS handshake when available.
- Certificate rotation is supported without server restart.
- Tests cover the HSTS header, OCSP stapling, and hot rotation.

## R-2316 Threat Mitigations (CSRF, SSRF, Body Size, Timeouts)

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2301`

### Acceptance

- CSRF protection validates the `Origin` header against an allowlist for
  state-changing methods.
- SSRF protection blocks requests to private IP ranges and link-local
  addresses by default.
- Body size limits are enforced before the body is fully read.
- Request timeouts cut off slow clients without affecting other requests.

## R-2317 API Example: Authenticated REST API (JWT)

- Status: `not_started`
- Priority: `P0`
- Owner: `ecosystem`
- Risk: `low`
- Dependencies: `R-2308`, `R-2219`

### Acceptance

- `examples/api/02_jwt_auth_crud.spectra` builds and runs.
- The example issues, validates, and rejects JWTs end-to-end.
- The example uses the unified error middleware and validation framework.

## R-2318 API Example: Middleware Composition

- Status: `not_started`
- Priority: `P1`
- Owner: `ecosystem`
- Risk: `low`
- Dependencies: `R-2302`, `R-2303`, `R-2304`, `R-2306`

### Acceptance

- `examples/api/03_middleware_composition.spectra` builds and runs.
- The example asserts the middleware order matches the documentation.
- The example demonstrates per-route configuration of the rate limit and
  security headers.

---

# Phase 24: Advanced API Features

WebSocket, SSE, HTTP/2, OpenAPI, caching, versioning, pagination, content
negotiation, and other production API features.

## R-2401 WebSocket Server (RFC 6455)

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2210`, `R-2215`

### Acceptance

- The server completes the RFC 6455 handshake and upgrades the
  connection.
- Fragmented messages are reassembled and ping/pong are handled.
- Per-message deflate is supported and negotiated via the extension
  header.
- Tests cover handshake, fragmented messages, ping/pong, and a 10k
  concurrent connections soak.

## R-2402 WebSocket Client

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2401`

### Acceptance

- The client connects to a known external test echo server and
  round-trips messages.
- The client supports text and binary frames and ping/pong.
- Reconnect with backoff is supported when configured.
- Tests cover handshake, frame round-trip, and reconnect.

## R-2403 Server-Sent Events (SSE)

- Status: `not_started`
- Priority: `P1`
- Owner: `web`
- Risk: `medium`
- Dependencies: `R-2210`

### Acceptance

- The SSE response streams events in the documented format.
- Heartbeats are emitted at the configured interval to keep the
  connection alive.
- The `Last-Event-ID` header is honored for resume on the server.
- Tests cover streaming, heartbeat, and resume.

## R-2404 HTTP/2 Server (h2, ALPN, HPACK)

- Status: `not_started`
- Priority: `P1`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2205`, `R-2207`

### Acceptance

- ALPN advertises `h2` alongside `http/1.1`.
- The server multiplexes streams on a single connection without
  head-of-line blocking.
- HPACK encoding round-trips with the configured client.
- Tests cover ALPN negotiation, multiplexing, and HPACK.

## R-2405 HTTP/2 Client

- Status: `not_started`
- Priority: `P1`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2404`, `R-2206`

### Acceptance

- The client connects to a known external h2 endpoint and round-trips
  requests.
- Multiple concurrent requests on a single connection are multiplexed.
- Server push is accepted and exposed through a callback.
- Tests cover multiplexing and server push.

## R-2406 HTTP/3 and QUIC

- Status: `not_started`
- Priority: `P3`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2404`

### Acceptance

- The decision is documented: include HTTP/3 only when a stable Rust QUIC
  implementation is available.
- If implemented, the server and client negotiate HTTP/3 over QUIC and
  exchange a request/response.
- If deferred, the rationale and the re-evaluation date are documented in
  the ADR.

## R-2407 API Versioning (Path, Header, Query)

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `medium`
- Dependencies: `R-2211`

### Acceptance

- All three versioning strategies are supported and documented.
- A single request can be matched to exactly one version.
- Version deprecation emits a `Deprecation` and `Sunset` header on
  responses.
- Tests cover each strategy and the deprecation headers.

## R-2408 Pagination (Cursor, Offset, Link Header RFC 5988)

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `medium`
- Dependencies: `R-2210`

### Acceptance

- Cursor pagination returns opaque cursors and the next cursor in the
  response.
- Offset pagination returns `page`, `page_size`, and `total`.
- RFC 5988 Link headers are emitted for both pagination styles.
- Tests cover happy path, last page, and invalid cursor.

## R-2409 Content Negotiation (JSON, XML, MessagePack, CBOR)

- Status: `not_started`
- Priority: `P1`
- Owner: `web`
- Risk: `medium`
- Dependencies: `R-2208`

### Acceptance

- The server picks the best supported type from the `Accept` header.
- The client serializes requests in the negotiated type.
- `415 Unsupported Media Type` is returned when no acceptable type is
  offered.
- Tests cover each type, the negotiation, and the 415 path.

## R-2410 Caching Headers (ETag, Last-Modified, Cache-Control, Vary)

- Status: `not_started`
- Priority: `P1`
- Owner: `web`
- Risk: `medium`
- Dependencies: `R-2210`

### Acceptance

- The server emits `ETag` and `Last-Modified` for cacheable responses.
- `If-None-Match` and `If-Modified-Since` are honored with `304`.
- `Cache-Control` directives are configurable per response.
- Tests cover ETag round-trips, conditional requests, and `Vary`.

## R-2411 OpenAPI 3.1 Generation

- Status: `not_started`
- Priority: `P0`
- Owner: `web` / `tooling`
- Risk: `high`
- Dependencies: `R-2211`, `R-2209`, `R-2210`

### Acceptance

- The generator produces a valid OpenAPI 3.1 JSON document.
- The document includes paths, parameters, request bodies, responses, and
  schemas.
- The document is exposed at the configured path (default `/openapi.json`).
- Tests assert the document against a checked-in golden file.

## R-2412 Background Jobs and Task Queue

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2205`, `R-2107`

### Acceptance

- Jobs can be enqueued with a payload and a delay.
- Failed jobs are retried with the configured backoff up to the
  configured max attempts.
- Dead-letter jobs are visible through a typed query.
- Tests cover happy path, retry, and dead-letter.

## R-2413 Cron and Scheduled Jobs

- Status: `not_started`
- Priority: `P2`
- Owner: `web`
- Risk: `medium`
- Dependencies: `R-2412`

### Acceptance

- Cron expressions parse and produce a deterministic next-run schedule.
- Overlapping runs are handled according to the documented policy.
- Timezone changes do not produce off-by-one or off-by-hour schedules.
- Tests cover each schedule shape and the overlap policy.

## R-2414 Email Send (SMTP and Templates)

- Status: `not_started`
- Priority: `P2`
- Owner: `web`
- Risk: `medium`
- Dependencies: `R-2219`

### Acceptance

- The SMTP client sends plain and HTML emails with optional attachments.
- Templates can be rendered with a typed context.
- Send failures are retried with backoff and reported as typed errors.
- Tests cover happy path, retry, and an integration test against a local
  SMTP server.

## R-2415 Webhooks (Signed Payloads, Retry, Dead Letter)

- Status: `not_started`
- Priority: `P1`
- Owner: `web`
- Risk: `medium`
- Dependencies: `R-2401`, `R-2206`

### Acceptance

- Webhook payloads are signed with HMAC-SHA256 and the signature is in
  the header.
- Failed deliveries are retried with exponential backoff.
- Dead-letter webhooks are visible through a typed query.
- Tests cover happy path, signature verification, retry, and dead-letter.

## R-2416 File Storage Abstraction (S3-Compatible)

- Status: `not_started`
- Priority: `P1`
- Owner: `web`
- Risk: `medium`
- Dependencies: `R-2206`

### Acceptance

- The storage trait exposes put, get, delete, list, and presigned URL
  operations.
- The S3-compatible implementation works against AWS S3 and a local
  MinIO test instance.
- The in-memory implementation is used by tests.
- Tests cover put/get/delete, presigned URLs, and a multipart upload.

## R-2417 Cache Layer (LRU In-Memory, Redis Distributed)

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `medium`
- Dependencies: `R-2507`

### Acceptance

- The cache trait exposes get, set, delete, and TTL-aware operations.
- The in-memory implementation enforces capacity and LRU eviction.
- The Redis implementation works against a local Redis test instance.
- Tests cover happy path, TTL, eviction, and concurrent access.

## R-2418 Configuration Management

- Status: `not_started`
- Priority: `P0`
- Owner: `web`
- Risk: `medium`
- Dependencies: `R-2107`

### Acceptance

- Configuration is loaded from defaults, file, environment variables, and
  explicit overrides in that order.
- Accessors are typed and emit a clear error for missing required values.
- The file layer supports JSON and TOML.
- Hot reload is supported in dev mode with a documented notification.

## R-2419 gRPC Server and Client (Protobuf, Async Streams)

- Status: `not_started`
- Priority: `P2`
- Owner: `web`
- Risk: `high`
- Dependencies: `R-2107`, `R-2404`

### Acceptance

- A `.proto` file compiles to a typed Spectra service.
- Unary and streaming RPCs are supported on the server and the client.
- Deadlines and cancellation are honored.
- Tests cover each RPC style and a metadata propagation case.

## R-2420 WebSocket Example: Real-Time Dashboard

- Status: `not_started`
- Priority: `P1`
- Owner: `ecosystem`
- Risk: `low`
- Dependencies: `R-2401`

### Acceptance

- `examples/api/04_websocket_dashboard.spectra` builds and runs.
- The example pushes updates to all clients and disconnects gracefully on
  shutdown.
- The example uses the documented WebSocket API only.

## R-2421 OpenAPI Example: Serve Swagger UI

- Status: `not_started`
- Priority: `P1`
- Owner: `ecosystem`
- Risk: `low`
- Dependencies: `R-2411`

### Acceptance

- `examples/api/05_openapi_swagger.spectra` builds and runs.
- The example serves `/openapi.json` and `/docs` (Swagger UI).
- The example wires the generated document into the UI.

---

# Phase 25: Persistence and Database

Production-grade connection pooling, SQL query builder, migrations,
first-class drivers for PostgreSQL, SQLite, and Redis, plus a minimal
ORM.

## R-2501 Connection Pool (Async-Aware)

- Status: `not_started`
- Priority: `P0`
- Owner: `db`
- Risk: `high`
- Dependencies: `R-2107`

### Acceptance

- The pool enforces the configured min/max size and idle timeout.
- Acquisition timeout is honored with a typed error.
- The pool integrates with the database drivers in Phase 25.
- Tests cover happy path, exhaustion, and recovery.

## R-2502 SQL Query Builder (Type-Safe)

- Status: `not_started`
- Priority: `P0`
- Owner: `db`
- Risk: `high`
- Dependencies: `R-2501`

### Acceptance

- Queries can be composed without string concatenation.
- Parameters are bound through the driver and not interpolated into SQL.
- The builder emits parameterized SQL for at least one supported dialect.
- Tests cover each query kind and parameter binding.

## R-2503 Migrations Framework

- Status: `not_started`
- Priority: `P0`
- Owner: `db`
- Risk: `high`
- Dependencies: `R-2502`

### Acceptance

- Migrations are applied in order and recorded in a tracking table.
- Checksum validation refuses to run if a previously-applied migration has
  changed.
- Down migrations roll back the schema and the tracking table.
- Tests cover up, down, partial state, and checksum mismatch.

## R-2504 SQLite Driver (Sync and Async)

- Status: `not_started`
- Priority: `P0`
- Owner: `db`
- Risk: `high`
- Dependencies: `R-2501`

### Acceptance

- CRUD, prepared statements, and transactions work against a file-backed
  SQLite database.
- The async driver is non-blocking and integrates with the connection
  pool.
- The sync driver is used by the migration framework when running tests.
- Tests cover CRUD, prepared statements, transactions, and concurrent
  reads.

## R-2505 PostgreSQL Driver (Async, Prepared, COPY)

- Status: `not_started`
- Priority: `P0`
- Owner: `db`
- Risk: `high`
- Dependencies: `R-2501`

### Acceptance

- CRUD, prepared statements, transactions, and savepoints work against
  PostgreSQL.
- COPY IN/OUT round-trips a large dataset within tolerance.
- LISTEN/NOTIFY is exposed through a typed channel.
- Tests run against a local PostgreSQL test instance and are gated by
  environment.

## R-2506 MySQL Driver

- Status: `not_started`
- Priority: `P1`
- Owner: `db`
- Risk: `medium`
- Dependencies: `R-2505`

### Acceptance

- CRUD, prepared statements, and transactions work against MySQL.
- The driver is dialect-aware and integrates with the query builder.
- Tests run against a local MySQL test instance and are gated by
  environment.

## R-2507 Redis Driver (with Pool)

- Status: `not_started`
- Priority: `P0`
- Owner: `db`
- Risk: `high`
- Dependencies: `R-2501`

### Acceptance

- GET, SET, DEL, EXPIRE, INCR, and pub/sub work against a local Redis
  test instance.
- The driver integrates with the connection pool and the cache layer.
- Tests cover happy path, expiry, eviction, and concurrent access.

## R-2508 Minimal ORM: Model Trait and Typed Queries

- Status: `not_started`
- Priority: `P1`
- Owner: `db`
- Risk: `high`
- Dependencies: `R-2502`, `R-2504`, `R-2505`

### Acceptance

- Models can derive CRUD methods through the documented macro.
- Primary key inference handles common cases (single field, `id` named).
- Typed find-by queries return the right type or a typed not-found error.
- Tests cover CRUD and find-by for SQLite and PostgreSQL.

## R-2509 Transactions (Begin, Commit, Rollback, Savepoints)

- Status: `not_started`
- Priority: `P0`
- Owner: `db`
- Risk: `high`
- Dependencies: `R-2501`

### Acceptance

- Transactions work for SQLite, PostgreSQL, MySQL, and Redis (when
  applicable).
- Rollback is automatic on panic, error, or explicit abort.
- Savepoints are supported with the documented semantics.
- Tests cover commit, rollback, and savepoint nesting.

## R-2510 Health Checks (Liveness, Readiness, Startup)

- Status: `not_started`
- Priority: `P0`
- Owner: `runtime`
- Risk: `medium`
- Dependencies: `R-2216`

### Acceptance

- The liveness endpoint always returns 200 while the process is up.
- The readiness endpoint returns 200 only when all required checks pass.
- The startup endpoint returns 200 only when startup is complete.
- Tests cover up, degraded, and recovering scenarios.

## R-2511 Database Example: REST + SQLite CRUD

- Status: `not_started`
- Priority: `P0`
- Owner: `ecosystem`
- Risk: `low`
- Dependencies: `R-2504`, `R-2219`, `R-2502`

### Acceptance

- `examples/api/06_rest_sqlite_crud.spectra` builds and runs.
- The example applies migrations and exposes CRUD endpoints.
- The example runs an in-process integration test.

## R-2512 Database Example: REST + PostgreSQL

- Status: `not_started`
- Priority: `P0`
- Owner: `ecosystem`
- Risk: `low`
- Dependencies: `R-2505`, `R-2219`, `R-2502`

### Acceptance

- `examples/api/07_rest_postgres_crud.spectra` builds and runs.
- The example applies migrations and exposes CRUD endpoints.
- The example runs an in-process integration test against a local
  PostgreSQL test instance.

## R-2513 Redis Example: Rate-Limit via Redis

- Status: `not_started`
- Priority: `P1`
- Owner: `ecosystem`
- Risk: `low`
- Dependencies: `R-2507`, `R-2304`

### Acceptance

- `examples/api/08_redis_rate_limit.spectra` builds and runs.
- The example enforces a per-tenant limit with a configurable window.
- The example asserts that one tenant's burst does not affect another
  tenant.

## R-2514 Migration Example: Multi-Version Evolution

- Status: `not_started`
- Priority: `P1`
- Owner: `ecosystem`
- Risk: `low`
- Dependencies: `R-2503`, `R-2504`

### Acceptance

- `examples/api/09_migrations.spectra` builds and runs.
- The example applies three migrations, rolls back, and re-applies.
- The example verifies the final schema and seed data.

---

# Phase 26: API Tooling and Developer Experience

Scaffolder, hot reload, tests, mock, Swagger UI, IDE integration, and
graceful shutdown. The work that turns `spectra.api` into something teams
can adopt.

## R-2601 spectralang api new Scaffolder

- Status: `not_started`
- Priority: `P0`
- Owner: `tooling`
- Risk: `medium`
- Dependencies: `R-2217`, `R-2219`

### Acceptance

- `spectralang api new my_api` produces a buildable project.
- The project depends on `spectra.api` from the local registry.
- `spectralang package run` starts the sample server and responds on the
  documented port.
- The project includes a smoke test that runs as part of
  `spectralang package test`.

## R-2602 Hot Reload Dev Server (spectralang api dev)

- Status: `not_started`
- Priority: `P0`
- Owner: `tooling`
- Risk: `high`
- Dependencies: `R-2601`

### Acceptance

- Saving a handler file restarts the server within the documented debounce
  window.
- Request logs are emitted to the dev terminal in real time.
- The dev server exposes the same routes as the production build.
- Tests cover file change, syntax error recovery, and graceful restart.

## R-2603 API Testing Framework (#[api_test])

- Status: `not_started`
- Priority: `P0`
- Owner: `tooling`
- Risk: `high`
- Dependencies: `R-2210`, `R-2109`

### Acceptance

- `#[api_test]` boots the app and exposes a typed test client.
- The test client can be configured with a base URL, default headers, and
  a cookie store.
- Assertions cover status, headers, body, and timing.
- Tests run inside the existing `spectralang package test` flow.

## R-2604 API Mocking and Contract Tests (Pact)

- Status: `not_started`
- Priority: `P1`
- Owner: `tooling`
- Risk: `medium`
- Dependencies: `R-2603`

### Acceptance

- Tests can register mocks for external services and assert on call
  counts.
- Pact-compatible contract files are emitted and verified by the test
  suite.
- The contract test mode is opt-in per test.
- Tests cover a happy-path contract and a breaking-change rejection.

## R-2605 spectralang api doc (Swagger UI and Redoc)

- Status: `not_started`
- Priority: `P0`
- Owner: `tooling`
- Risk: `medium`
- Dependencies: `R-2411`

### Acceptance

- `spectralang api doc` starts a local server that serves `/openapi.json`
  and the UI.
- The UI is interactive and supports `try it out` for the documented
  endpoints.
- The CLI can target a built artifact and a running dev server.

## R-2606 Postman, Bruno, and Insomnia Export

- Status: `not_started`
- Priority: `P2`
- Owner: `tooling`
- Risk: `low`
- Dependencies: `R-2411`

### Acceptance

- The CLI emits each collection format from the same OpenAPI source.
- The exported collections include auth, headers, and example bodies.
- The exported collections import cleanly into the documented tool
  versions.

## R-2607 Graceful Shutdown and Signal Handling

- Status: `not_started`
- Priority: `P0`
- Owner: `runtime`
- Risk: `medium`
- Dependencies: `R-2216`, `R-2105`

### Acceptance

- SIGINT and SIGTERM are handled in dev and production builds.
- In-flight requests are given a configurable drain timeout.
- The process exits with code 0 on success and a documented non-zero code
  on shutdown error.
- Tests cover both signals and the drain timeout.

## R-2608 Production Config Profiles (dev, staging, prod)

- Status: `not_started`
- Priority: `P0`
- Owner: `runtime`
- Risk: `medium`
- Dependencies: `R-2418`

### Acceptance

- The profile is selected by env var or CLI flag.
- Each profile has a documented default set of middleware, log format,
  and rate limit.
- Profile-aware defaults are tested by the conformance suite.
- The documentation explains the security and observability implications
  of each profile.

## R-2609 API Conformance Suite v1 (Status, Headers, Errors)

- Status: `not_started`
- Priority: `P0`
- Owner: `tooling`
- Risk: `medium`
- Dependencies: `R-2220`, `R-2314`

### Acceptance

- The suite covers 50+ documented must-pass status and header cases.
- The suite asserts the unified error shape for validation, auth, and
  5xx paths.
- The suite is gated by `run_tests.ps1`.
- The suite emits a versioned conformance report.

## R-2610 Book Chapter: Building Production APIs in Spectra

- Status: `not_started`
- Priority: `P0`
- Owner: `ecosystem`
- Risk: `low`
- Dependencies: `R-2218`, `R-2601`

### Acceptance

- The chapter is reachable from the book index and from `docs/api/`.
- The chapter is validated by the existing
  `scripts/validate_ai_book.py`-style validator.
- The reader can complete the chapter end-to-end on a local machine.

## R-2611 LSP: Routes, Handlers, and Types

- Status: `not_started`
- Priority: `P0`
- Owner: `tooling`
- Risk: `medium`
- Dependencies: `R-2211`, `R-1001`

### Acceptance

- LSP completion lists routes, handler parameters, and typed response
  shapes.
- Go-to-definition resolves handler symbols across modules.
- Hover shows the route path, method, and constraints.
- Tests assert completion and hover in a multi-file API project.

## R-2612 spectralang api lint

- Status: `not_started`
- Priority: `P1`
- Owner: `tooling`
- Risk: `low`
- Dependencies: `R-2211`

### Acceptance

- The linter reports each rule with a stable code and an actionable
  message.
- The linter integrates with `spectralang lint`.
- A focused test project covers each rule and its suppression.

## R-2613 Debugger: Breakpoints in Handlers

- Status: `not_started`
- Priority: `P1`
- Owner: `tooling`
- Risk: `medium`
- Dependencies: `R-1002`, `R-2215`

### Acceptance

- Breakpoints can be set on handler entry, on each statement, and on the
  return.
- The debugger shows the request, the response, and the local variables.
- The debugger integrates with the existing source map and the existing
  debug adapter.

## R-2614 VS Code Plugin Updates for spectra.api

- Status: `not_started`
- Priority: `P1`
- Owner: `tooling`
- Risk: `low`
- Dependencies: `R-2611`, `R-2605`

### Acceptance

- The plugin surfaces the `spectra.api` completions in `.spectra` files.
- The plugin exposes a `Run API` task that starts
  `spectralang api dev`.
- The plugin shows the OpenAPI document in a side panel.

## R-2615 Project Templates: REST, GraphQL, gRPC, Microservice

- Status: `not_started`
- Priority: `P0`
- Owner: `tooling`
- Risk: `medium`
- Dependencies: `R-2601`

### Acceptance

- `spectralang api new --template rest` creates a REST project.
- The GraphQL, gRPC, and microservice templates are available behind a
  documented flag.
- Each template builds, runs, and has a smoke test.

---

# Phase 27: Observability and API Operations

OpenTelemetry tracing, Prometheus metrics, audit logs, health checks, and
per-tenant rate limiting. The work that makes `spectra.api` production
operable.

## R-2701 OpenTelemetry-Compatible Tracing

- Status: `not_started`
- Priority: `P0`
- Owner: `runtime`
- Risk: `high`
- Dependencies: `R-2210`, `R-2107`

### Acceptance

- The runtime emits spans for HTTP request handling, database queries,
  and external calls.
- The OTLP exporter sends spans to a local collector for tests.
- The trace context propagates across the supported HTTP clients.
- Tests assert span hierarchy, attributes, and context propagation.

## R-2702 Prometheus-Compatible Metrics Endpoint

- Status: `not_started`
- Priority: `P0`
- Owner: `runtime`
- Risk: `medium`
- Dependencies: `R-2210`

### Acceptance

- The `/metrics` endpoint returns a valid Prometheus exposition payload.
- Default metrics cover request count, latency histogram, and error
  count.
- Custom counters and histograms can be registered and incremented.
- Tests assert the metric shape and a known Prometheus parser round-trip.

## R-2703 Health, Readiness, and Startup Probes (Integrated)

- Status: `not_started`
- Priority: `P0`
- Owner: `runtime`
- Risk: `medium`
- Dependencies: `R-2510`

### Acceptance

- The probes are served on the documented paths
  (`/healthz`, `/readyz`, `/startupz`).
- The probe results aggregate the custom checks registered through
  `R-2510`.
- The documentation covers the Kubernetes, Docker, and systemd wiring.
- Tests cover up, degraded, and recovering scenarios.

## R-2704 Request and Response Audit Log (LGPD, GDPR)

- Status: `not_started`
- Priority: `P1`
- Owner: `runtime`
- Risk: `medium`
- Dependencies: `R-2303`

### Acceptance

- The audit log records request, response, request ID, and user identity.
- PII fields are redacted according to the configured policy.
- The audit log is emitted as a versioned JSON stream.
- Tests cover happy path, redaction, and missing user identity.

## R-2705 Distributed Tracing (W3C Trace Context)

- Status: `not_started`
- Priority: `P1`
- Owner: `runtime`
- Risk: `medium`
- Dependencies: `R-2701`, `R-2206`

### Acceptance

- Outgoing HTTP requests carry the configured `traceparent` header.
- Incoming `traceparent` headers are honored and used as the parent span.
- Database and Redis calls inherit the current trace context.
- Tests assert context propagation end-to-end.

## R-2706 Per-Tenant and Per-User Rate Limiting

- Status: `not_started`
- Priority: `P0`
- Owner: `runtime`
- Risk: `high`
- Dependencies: `R-2304`, `R-2701`

### Acceptance

- Each tenant has an isolated budget that is not affected by other
  tenants.
- Rate-limit rejections emit a structured metric and a span event.
- The limit is hot-reloadable without server restart.
- Tests cover per-tenant isolation, hot reload, and the observability
  hook.

## R-2707 OTel and Prometheus Exporters Example

- Status: `not_started`
- Priority: `P1`
- Owner: `ecosystem`
- Risk: `low`
- Dependencies: `R-2701`, `R-2702`

### Acceptance

- `examples/api/10_otel_prometheus.spectra` builds and runs.
- The example starts a local OTel collector and Prometheus endpoint.
- The example asserts that the expected spans and metrics are emitted.

## R-2708 Audit Log Example with PII Redaction

- Status: `not_started`
- Priority: `P1`
- Owner: `ecosystem`
- Risk: `low`
- Dependencies: `R-2704`

### Acceptance

- `examples/api/11_audit_log.spectra` builds and runs.
- The example redacts configured PII fields and keeps the request shape.
- The example emits a versioned JSON audit log.

---

# Phase 28: API Conformance and Release

Certify `spectra.api` v1.0: conformance suite, interop tests, full
example gallery, production hardening, and registry release.

## R-2801 API Conformance Suite v1 (Final)

- Status: `not_started`
- Priority: `P0`
- Owner: `tooling`
- Risk: `high`
- Dependencies: `R-2220`, `R-2609`, `R-2703`

### Acceptance

- The suite covers 100+ documented must-pass cases for the public API
  surface.
- The suite emits a versioned certification report.
- Release candidates cannot be certified while any required category
  fails.
- The suite is gated by `run_tests.ps1`.

## R-2802 Interop Tests Against Express, FastAPI, and Actix

- Status: `not_started`
- Priority: `P1`
- Owner: `tooling`
- Risk: `medium`
- Dependencies: `R-2801`

### Acceptance

- An interop test suite exercises JSON, headers, status, and CORS with
  the reference servers.
- The suite reports per-server and per-case pass/fail with reproducible
  commands.
- The suite is gated by environment and CI, not by the local test runner.

## R-2803 Documentation Site for spectra.api

- Status: `not_started`
- Priority: `P0`
- Owner: `ecosystem`
- Risk: `low`
- Dependencies: `R-2610`, `R-2218`

### Acceptance

- The site is reachable from the main book index.
- The site includes the public API reference, the cookbook, and the
  migration guide.
- The site is built and validated by `run_tests.ps1`.

## R-2804 API Example Gallery (REST, GraphQL, gRPC, WebSocket, SSE)

- Status: `not_started`
- Priority: `P0`
- Owner: `ecosystem`
- Risk: `low`
- Dependencies: `R-2217`, `R-2411`, `R-2401`, `R-2403`, `R-2419`

### Acceptance

- The gallery is documented at `examples/api/README.md` with a one-line
  description per example.
- Every example in the gallery builds, runs, and has a smoke test.
- The gallery is validated by `scripts/validate_r2804_api_gallery.py` and
  gated by `run_tests.ps1`.

## R-2805 Production Hardening: Load, Soak, Chaos

- Status: `not_started`
- Priority: `P0`
- Owner: `tooling`
- Risk: `high`
- Dependencies: `R-2701`, `R-2702`, `R-2703`

### Acceptance

- A load test exercises the documented target throughput and reports
  p95/p99 latency.
- A soak test runs for at least 24 hours and reports leaks, latency
  drift, and error rate.
- A chaos test kills dependencies and asserts graceful degradation.
- Regression thresholds are stored in
  `docs/performance/r2805-hardening.json`.
- The hardening report is versioned and is part of the release evidence.

## R-2806 spectra.api v1.0 Registry Release

- Status: `not_started`
- Priority: `P0`
- Owner: `ecosystem`
- Risk: `high`
- Dependencies: `R-2801`, `R-2804`, `R-2805`

### Acceptance

- `spectra.api@1.0.0` is published to the local registry.
- The v1 contract is documented and versioned.
- The deprecation policy and the migration guide are published.
- `spectralang package add spectra-api@1.0.0` resolves cleanly from a
  fresh project.

## R-2807 Migration Guide: From ad-hoc std web to spectra.api

- Status: `not_started`
- Priority: `P2`
- Owner: `ecosystem`
- Risk: `low`
- Dependencies: `R-2801`

### Acceptance

- The guide lists the differences between the previous surface and the
  v1 `spectra.api` surface.
- The guide includes step-by-step migration recipes for the most common
  patterns.
- The guide is published in the documentation site and referenced from
  the changelog.

---

# API Platform Quick-Reference

## Owner Groups (additions)

| Owner | Scope |
|---|---|
| `web` | HTTP server/client, routing, middleware, WebSocket, SSE |
| `db` | Drivers, query builder, migrations, ORM, connection pool |

## Dependency Tree (Critical Path)

```
R-2101 (ADR async) → R-2102 (async fn) → R-2103 (await) → R-2104 (reactor) → R-2105 (cancel) → R-2106 (streams) → R-2107 (async stdlib)
                                                                                                      ↓
                                                                                          R-2201 (ADR api) → R-2202 (crate) → R-2204 (parser) → R-2205 (server) → R-2211 (router)
                                                                                                                                                  ↓
                                                                                                                                              R-2301 (middleware) → R-2302..R-2316
                                                                                                                                                  ↓
                                                                                                                                              R-2501 (pool) → R-2505 (postgres)
                                                                                                                                                  ↓
                                                                                                                                              R-2801 (conformance) → R-2806 (release)
```

## Item Count by Phase

| Phase | Items | Priority Mix |
|---|---|---|
| 21 — Async Language Core | 11 | 6 P0, 4 P1, 1 P2 |
| 22 — API Library Foundation | 20 | 14 P0, 1 P1, 5 ecosystem |
| 23 — Middleware and Security | 18 | 9 P0, 6 P1, 3 P2 |
| 24 — Advanced API Features | 21 | 6 P0, 13 P1, 2 P2/P3 |
| 25 — Persistence and Database | 14 | 9 P0, 5 P1 |
| 26 — API Tooling and DX | 15 | 8 P0, 6 P1, 1 P2 |
| 27 — Observability and API Ops | 8 | 5 P0, 3 P1 |
| 28 — API Conformance and Release | 7 | 5 P0, 2 P1/P2 |
| **Total** | **114** | — |

## New Files To Be Added by the Workstream

- `runtime/src/api/` (parser HTTP, server, client, JSON, TLS)
- `runtime/src/reactor/` (event loop multiplexador)
- `runtime/Cargo.toml` (deps: `rustls`, primitives async próprias)
- `packages/spectra-api/` (pacote Spectra publicado via registry)
- `packages/spectra-api/src/*.spectra` (bindings `std.api.*`)
- `packages/spectra-api/spectra.toml`
- `docs/adr/0010-async-execution-model.md`
- `docs/adr/0011-api-library-architecture.md`
- `docs/adr/0012-http-server-runtime-architecture.md`
- `docs/api/` (reference para `spectra.api`)
- `docs/book/09-building-apis.md`
- `examples/api/` (galeria)
- `scripts/validate_r22XX_*.py` (um validator por fase)
- `tests/validation/api_*.spectra`
