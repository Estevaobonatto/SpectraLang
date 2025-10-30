# ADR 0001: Lexer foundation

## Context

The SpectraLang prototype requires an immediately available lexer capable of supporting the phase 1 grammar subset (functions, variable bindings, control flow, and literals). We need predictable error reporting and span tracking to power diagnostics throughout the compiler pipeline.

## Decision

- Implement a bespoke hand-written lexer in Rust rather than depending on external generators.
- Track byte offsets and 1-based line/column locations for each token via `Span` and `Location` structures.
- Emit tokens for core syntax constructs (keywords, identifiers, literals, punctuation) and provide early support for inline comments (`//`) and block comments (`/* */`).
- Normalize numeric literals by stripping `_` separators at lexing time to simplify downstream parsing.
- Return `Vec<Token>` alongside aggregated `LexError` entries to facilitate batch diagnostics without stopping after the first failure.

## Status

Accepted – initial implementation merged into the `compiler` crate.

## Consequences

- Parser and future semantic passes can rely on consistent span information for user-facing diagnostics.
- Extending the lexer to new tokens will require manual updates but keeps total control over emitted structures and performance characteristics.
- Introducing Unicode-aware identifiers or advanced numeric literal forms will necessitate targeted enhancements to the lexer utility functions.
