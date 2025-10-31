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

> O comando `new` gera um esqueleto de projeto com `src/main.spc` e um stub de biblioteca em `src/std/console.spc`. O `build` compila todo o diretório, valida a assinatura de `main` e gera um artefato em `target/<profile>/<bin>.build.txt` mapeando módulos e fontes para binários.

Selecione explicitamente o módulo de entrada quando houver múltiplos `fn main()`:

```powershell
cargo run --package spectra-cli -- build hello_cli --main app.beta
```

Gere manifestos para todos os módulos com `main` de uma só vez:

```powershell
cargo run --package spectra-cli -- build hello_cli --all
```

### Console I/O helpers

The runtime crate already exposes minimal console helpers. Importe `std.console` e use `print/println/print_err/println_err/read_line` diretamente (ou com o prefixo `std.console::` se preferir) além dos utilitários `std.args`. Veja exemplos práticos em `docs/console-io-recipes.md`.

### Rodando builds dentro do projeto gerado

```powershell
cd hello_cli
cargo run --package spectra-cli -- build .
```

Compila os arquivos em `src/` da pasta atual, reportando diagnósticos léxicos, sintáticos e semânticos antes de produzir o artefato do console app.

## Front-end capabilities (Oct 2025)

- Lexing/parsing for modules, functions, structs, enums, arrays, typed bindings, async/await, `try`/`catch`, `using`, and `defer`, all com diagnósticos sensíveis a spans.
- Semantic analyzer com rastreamento de escopos, checagem de tipos para primitivos, arrays, structs e futuros (`Future<T>`), validação de contexto `async`/`await`, regras de RAII (`using`/`defer`) e visibilidade cross-module para funções, constantes, structs e enums públicos.
- Detecção de conflitos para redefinições, colisões de import/export e reexportações encadeadas.
- Suíte de testes (`cargo test --workspace`) cobrindo caminhos felizes e diagnósticos de erro para as capacidades recentes, incluindo async/await e gerenciamento automático de recursos.

## Continuous integration

GitHub Actions workflow (`.github/workflows/ci.yml`) runs formatting, lint checks, tests, and builds on push and pull requests.

## Contributing

1. Install the Rust toolchain via [rustup](https://rustup.rs/).
2. Run `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` before opening a pull request.
3. Discuss significant design changes with the team and capture them as ADRs in `docs/decisions/`.
