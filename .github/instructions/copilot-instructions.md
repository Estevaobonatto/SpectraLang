# SpectraLang Compiler Development Guide

## Project Overview

SpectraLang is a prototype compiler for a modern multi-paradigm language targeting JIT compilation. The codebase is a **Rust workspace** with three primary crates that form a traditional compiler frontend architecture:

- **`compiler/`** – Core compilation stages: lexer → parser → semantic analyzer (current frontier)
- **`runtime/`** – Future GC and runtime scaffolding (placeholder)
- **`tools/spectra-cli/`** – Multi-file CLI that drives the compilation pipeline

## Architectural Patterns

### Compilation Pipeline (lexer → parser → semantic)

The compiler uses a **three-phase error-collecting architecture**. Each phase returns `Result<T, Vec<Error>>` to batch all diagnostics before failing:

```rust
// tools/spectra-cli/src/main.rs entry point
Lexer::new(&source).tokenize()    // -> Result<Vec<Token>, Vec<LexError>>
Parser::new(&tokens).parse()       // -> Result<Module, Vec<ParseError>>
semantic::analyze_modules(&[&module])  // -> Result<(), Vec<SemanticError>>
```

**Key principle**: Never stop after the first error. Collect all issues per phase to provide comprehensive feedback.

### Span-Based Error Reporting

Every token, AST node, and semantic symbol tracks a `Span` with byte offsets and 1-based line/column positions:

```rust
pub struct Span {
    pub start: usize,           // byte offset
    pub end: usize,
    pub start_location: Location,  // line:column (1-based)
    pub end_location: Location,
}
```

When creating new AST nodes or error messages, **always compute and preserve spans** using `span_union()` helper functions (see `parser.rs`).

### Multi-Module Semantic Analysis

The semantic analyzer (`semantic.rs`) handles cross-file compilation with a **three-pass architecture**:

1. **Registration Pass**: `register_modules()` collects all module paths
2. **Export Collection**: `collect_exports()` builds symbol tables from `pub fn` and `pub let` items
3. **Validation Pass**: `analyze_module()` for each file with imported symbols materialized in scope

**Critical**: Always process all modules together via `analyze_modules(&[&module1, &module2])`, not individually.

### Symbol Tables and Scope Management

`ScopeStack` manages hierarchical scopes (module → function → block):

```rust
struct ScopeStack {
    frames: Vec<ScopeFrame>,  // Stack of {bindings, kind, span}
}
```

**Scoping rules**:
- Module-level symbols (functions, structs, constants) registered before body analysis
- Imports materialize into module scope after validation
- Each function gets a new scope frame; parameters shadow outer bindings
- Track symbol **usage** separately to report unused variables/parameters

### Type System (Current State)

The `Type` enum represents the partially-implemented type system:

```rust
enum Type {
    I32, F64, Bool, String, Void,  // Primitives
    Function(FunctionType),         // Signature with params + return
    Struct(String, HashMap<String, Type>),
    Enum(String),
    Unknown,  // Used for incomplete analysis or errors
}
```

**Type inference**: Propagates bottom-up from literals → binary/unary ops → returns. Not yet flow-sensitive across branches.

## Development Workflows

### Building and Testing

```powershell
# Format, lint, and test (CI requirements)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets

# Run the CLI with multiple files
cargo run --package spectra-cli -- examples/hello.spc examples/math.spc
```

**CI enforces** zero warnings and all tests passing on Ubuntu/Windows.

### Testing Strategy

Tests live in module-level `#[cfg(test)]` blocks (not separate files). Use helper functions:

```rust
// compiler/src/semantic.rs tests
fn analyze_ok(source: &str) -> () { /* ... */ }
fn analyze_err(source: &str) -> Vec<SemanticError> { /* ... */ }
fn module_from_source(source: &str) -> Module { /* ... */ }
```

**Coverage priorities**: Error paths (redefinitions, type mismatches) are equally important as happy paths.

### Adding Language Features

When extending the language (new syntax, semantic rules):

1. **Lexer** (`lexer.rs`): Add token types if needed, update `Keyword::from_identifier()`
2. **Parser** (`parser.rs`): Extend `parse_*` methods, add AST node variants in `ast.rs`
3. **Semantic** (`semantic.rs`): Update `analyze_*` methods, add type rules
4. **Tests**: Add cases to all modified modules' test sections
5. **Documentation**: Update ADR in `docs/decisions/` if architectural

**Example**: Struct literals required changes across `ast::Expr::StructLiteral`, `parser::parse_struct_literal()`, and `semantic::check_expr()` with type validation.

### Module System Conventions

SpectraLang modules use **dotted paths** (`module core.math;`) and **double-colon imports** (`import std.io; export std.io::print;`):

```spectra
module app.math;
pub fn add(a: i32, b: i32): i32 { return a + b; }

// In another file:
module app.main;
import app.math;
fn main() { let x = add(1, 2); }  // 'add' auto-imported
```

**Key semantic rules**:
- Only `pub fn` and immutable `pub let` can be exported/imported
- Exports create re-exports: `export app.math::add` makes `add` visible from current module
- Self-import is silently ignored to allow multi-file compilation
- Module names must be unique across compilation units

## Common Pitfalls

1. **Forgetting spans**: Every error and AST node needs a `Span`. Use `span_union()` to combine ranges.
2. **Single-module analysis**: Always use `analyze_modules(&[...])`, not `analyze()`, when working with imports.
3. **Scope frame leaks**: Every `push()` must have a matching `pop()` at scope boundaries.
4. **Type unknowns**: `Type::Unknown` indicates incomplete analysis, not user errors. Emit a diagnostic before returning `Unknown`.
5. **Test isolation**: Tests mutate global state if using module exports. Keep test sources fully qualified.

## Current Implementation Boundaries

**Implemented**: Lexer, parser (full grammar), semantic analysis (scopes, types, multi-module, call validation)

**Not Yet Implemented**:
- Control flow type narrowing (if/match branches)
- Generic types and trait bounds
- Struct/enum member access type checking
- Code generation (SIR/JIT backend)
- Runtime and GC

**Design documents**: See `docs/decisions/` for ADRs documenting lexer (0001), parser (0002), and semantic roadmap (0003).

## File Navigation Quick Reference

- **Entry point**: `tools/spectra-cli/src/main.rs`
- **Token definitions**: `compiler/src/token.rs` (keywords, operators)
- **AST structures**: `compiler/src/ast.rs` (Module, Expr, Stmt, Item)
- **Error types**: `compiler/src/error.rs` (LexError, ParseError, SemanticError)
- **Core semantics**: `compiler/src/semantic.rs` (2475 lines - analyzer, type checker, scope stack)
- **Test examples**: `examples/*.spc` (hello.spc, struct_demo.spc, math.spc)
