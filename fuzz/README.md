# SpectraLang Fuzz Targets

This directory contains the R-104 fuzz targets for the compiler test pyramid.
They are intentionally outside the default workspace so normal `cargo test`
remains fast and does not require `cargo-fuzz`.

## Targets

- `parser`: lexes and parses arbitrary UTF-8 input.
- `semantic`: runs lexer, parser, and semantic analysis on parseable modules.
- `pipeline`: runs the production compilation pipeline with the no-op backend.
- `lowering`: lowers parseable AST modules into midend IR.

## Running

Install once:

```powershell
cargo install cargo-fuzz
```

Run a target:

```powershell
cargo fuzz run parser
cargo fuzz run semantic
cargo fuzz run pipeline
cargo fuzz run lowering
```

Crash artifacts should be minimized with `cargo fuzz tmin` and converted into
checked-in regression tests before closing the bug.
