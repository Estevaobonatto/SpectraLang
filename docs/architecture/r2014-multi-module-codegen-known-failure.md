# R-2014 Multi-Module Codegen Recovery

This document records the resolved `R-2014` failure for a valid multi-module
`.spectra` package that previously failed during backend code generation.

Promoted regression:

```powershell
C:\Users\estev\.cargo\bin\cargo.exe run -p spectra-cli -- package test --root tests\projects\valid\integrated_basic_deep_components
```

Original observed failure:

```text
error[codegen]: Value 13 not found during backend code generation
```

The same project shape also failed under `spectralang package test` before the
fix landed.

Root cause:

- semantic import reconstruction preserved tuple enum payloads but dropped
  struct-style enum payload metadata by recreating imported variants with
  `struct_data: None`
- the midend then lowered `PaymentState::Partial { due } => due` without
  binding `due`, producing an undefined IR value (`%v13`)
- the backend reported the missing value and then Cranelift cleanup panicked
  because generation aborted inside an active `FunctionBuilder`

Fix:

- preserve `enum_struct_variants` when reconstructing imported enum AST
  definitions for downstream midend layout registration
- fall back to source order for named enum patterns if field-order metadata is
  unavailable
- emit concrete `ConstInt 0` sentinels for value-less reachable `if`/`unless`
  expression merges instead of raw `next_value()` IDs
- verify undefined IR operands before backend generation

The source uses multiple modules with:

- cross-module struct construction and method calls
- enum tuple and struct payload variants
- trait implementation and concrete trait-method dispatch
- `match`, `while let`, `unless`, and mutable loop state
- package/project compilation through the normal CLI path

The project now lives at
`tests/projects/valid/integrated_basic_deep_components` and is registered in
the R-2008 integrated matrix.
