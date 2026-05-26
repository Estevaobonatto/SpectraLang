# Language Feature Maturity Policy

Updated: 2026-05-21  
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
- closures/lambdas in the currently validated surface
- qualified stdlib calls such as `std.io.println(...)`
- `std.tensor` production baseline runtime API for tensor handles, safe views, shape metadata, elementwise ops, reductions, transforms, 2D matmul, and batched matmul
- `std.tensor` production baseline reverse-mode autodiff for float tensor handles, scalar tensor losses, gradient accumulation, and inference/no-grad mode
- `std.ml` production baseline runtime API for modules, layers, losses, optimizers, LR scheduling, tensor-backed datasets, and dataloaders

### Beta

- class syntax footprint
- `static` item surface
- closure/runtime representation as an optimization target
- first-class tensor language design beyond the current stdlib handle/autodiff API

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
- production tensor syntax, static shape types, device placement, and GPU kernels

## Synchronization Rules

When a feature changes maturity:

1. update this file
2. update the user-facing reference docs
3. update examples if their required invocation changes
4. update CLI help or `--list-experimental` if the change affects experimental gating
5. add or adjust tests in `tests/validation`, `tests/errors`, `tests/cli`, or `examples`
