# Spectra Tooling

This document describes the Phase 10 tooling baseline.

## LSP

The language server lives in `tools/spectra-lsp`.

Implemented LSP capabilities:

- diagnostics
- hover
- go to definition
- references
- rename
- document highlights
- document symbols
- workspace symbols
- formatting
- completion
- signature help
- inlay hints
- semantic tokens
- quick fixes for selected diagnostics and lints

Rename behavior:

- uses semantic definition links when the compiler exposes them
- falls back to a bounded lexical block rename for local identifiers without definition spans
- validates Spectra identifier syntax and rejects keywords
- avoids identifier substring replacements such as renaming `value` inside `value_extra`

Validation:

```powershell
cargo test -p spectra-lsp
```

## Runtime Diagnostics

`spectralang run` emits a runtime diagnostic when a program exits with a non-zero status:

```text
error[runtime]: program exited with status 7
  --> path/to/file.spectra:3:5
   |
   = help: inspect the main entry point and rerun with '--timings' for pipeline context
```

This is the current source-aware runtime baseline. Full trap stacks and AOT debugger integration remain future work.

## Benchmarking

Use:

```powershell
spectralang bench --bench-json target/bench.json tests/validation/01_basic_syntax.spectra
```

The JSON report includes:

- per-module frontend timings
- lexing/parsing/semantic/backend timings
- lowering/codegen timings
- optimization pass timings
- aggregate totals

Package workspaces can use:

```powershell
spectralang package bench --root .
```

The JSON output is intentionally stable enough for CI scripts to compare totals and detect regressions.
