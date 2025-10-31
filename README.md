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
cargo run --package spectra-cli -- new hello_cli
cargo run --package spectra-cli -- build hello_cli
```

> O comando `new` gera um esqueleto de projeto com `src/main.spc`. O `build` compila todo o diretório, valida a assinatura de `main` e gera um artefato em `target/<profile>/<bin>.build.txt` mapeando módulos e fontes.

Selecione explicitamente o módulo de entrada quando houver múltiplos `fn main()`:

```powershell
cargo run --package spectra-cli -- build hello_cli --main app.beta
```

### Rodando builds dentro do projeto gerado

```powershell
cd hello_cli
cargo run --package spectra-cli -- build .
```

Compila os arquivos em `src/` da pasta atual, reportando diagnósticos léxicos, sintáticos e semânticos antes de produzir o artefato do console app.

## Front-end capabilities (Oct 2025)

- Lexing/parsing for modules, functions, structs, enums, arrays, typed bindings, and import/export declarations (with span-aware diagnostics).
- Semantic analyzer with scope tracking, type checking for primitives, arrays, and struct literals, plus cross-module visibility for public functions, constants, structs, and enums.
- Conflict detection for duplicate definitions and import collisions, including re-export propagation across modules.
- Test suite (`cargo test --workspace`) covering happy paths and error reporting for recent features such as typed annotations, array inference, and multi-module imports.

## Continuous integration

GitHub Actions workflow (`.github/workflows/ci.yml`) runs formatting, lint checks, tests, and builds on push and pull requests.

## Contributing

1. Install the Rust toolchain via [rustup](https://rustup.rs/).
2. Run `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` before opening a pull request.
3. Discuss significant design changes with the team and capture them as ADRs in `docs/decisions/`.
