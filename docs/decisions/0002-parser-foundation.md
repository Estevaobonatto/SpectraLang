# ADR 0002: Parser and AST foundation

## Context

Phase 1 requires a resilient parser capable of handling SpectraLang's core constructs: module declarations, top-level bindings, functions, basic expressions, and block-based control flow placeholders. We also need AST structures rich enough to feed semantic analysis and later stages while keeping spans for diagnostics.

## Decision

- Implement a hand-written recursive-descent/Pratt hybrid parser in Rust to reuse the existing token stream and span metadata from the lexer.
- Extend the AST with:
  - `Module` metadata (optional module path plus a list of top-level items).
  - `Item` enum covering `Function` definitions and statement items to keep evaluation order explicit.
  - Rich statement variants (`Let`, `Return`, `Block`, expression statements) and `Block` nodes holding nested statements.
  - Function signatures with typed parameters, optional return types, and body blocks.
  - `TypeName` path nodes to represent qualified type identifiers for future semantic checks.
- Support parsing of:
  - Optional `module path;` header restricted to the file prologue.
  - `fn name(param: Type, ...) [: ReturnType] { ... }` definitions with empty or populated parameter lists.
  - Block-scoped `let/var` bindings, `return` statements, and expression statements requiring explicit semicolons.
- Preserve spans for every AST node to drive upcoming diagnostics and tooling (LSP, formatter, etc.).

## Status

Accepted – parser implementation merged into the `compiler` crate together with the expanded AST.

## Consequences

- The CLI and future tooling can identify module names, function counts, and statement inventories without semantic analysis.
- Adding new statements or expression forms will require localized extensions to the parser combinators while reusing span helpers.
- Semantic analysis can rely on structured blocks when introducing scopes, symbol tables, and type checking.
- The strict requirement for semicolons keeps the grammar deterministic for the initial prototype but may be revisited once statement inference or expression-bodied functions are considered.
