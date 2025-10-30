# SpectraLang

Prototype implementation workspace for the SpectraLang programming language. This repository hosts the compiler front-end, runtime scaffolding, CLI tooling, and documentation assets described in the project plan.

## Workspace layout

- `compiler/` – Rust crate containing the lexer, parser, AST definitions, and the evolving semantic analyzer.
- `runtime/` – Placeholder crate for the future SpectraLang runtime and garbage collector integrations.
- `tools/spectra-cli/` – Early command-line interface used to lex and parse SpectraLang source files.
- `docs/` – Planning material, specifications, and architectural decision records.

## Build requirements

- Rust toolchain (1.75+ recommended).

## Quick start

```powershell
cargo run --package spectra-cli -- examples/hello.spc
```

> Provide a SpectraLang source file as input; the CLI now reports lexical, parsing, or semantic issues (including function call/`import` diagnostics) and summarizes parsed modules/functions.

## Continuous integration

GitHub Actions workflow (`.github/workflows/ci.yml`) runs formatting, lint checks, tests, and builds on push and pull requests.

## Contributing

1. Install the Rust toolchain via [rustup](https://rustup.rs/).
2. Run `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` before opening a pull request.
3. Discuss significant design changes with the team and capture them as ADRs in `docs/decisions/`.
