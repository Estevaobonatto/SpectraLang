# Parser & Lexer Coverage Audit

_Date: 2025-11-06_
_Branch: devlop_

## Scope
- Reviewed the current lexer (`compiler/src/lexer/mod.rs`) and parser modules (`compiler/src/parser/*`).
- Cross-referenced behaviour with the frozen alpha language reference (`docs/language-reference-alpha.md`).

## Lexer Findings
- Supports single-line `//` comments and skips whitespace/newlines; block comments and nested comments are not recognised.
- Tokenises string literals without escape-sequence handling; unterminated strings raise a lex error but do not recover inline.
- Numbers accept a single optional fractional part (`123.45`); there is no support for exponent notation, digit separators, or numeric suffixes.
- Identifiers follow the documented `[A-Za-z_][A-Za-z0-9_]*` pattern; Unicode identifiers are rejected.
- Keyword table includes reserved tokens (`foreach`, `repeat`, `until`, `cond`, `yield`, `goto`, `class`, `export`) that the parser does not currently consume.
- Symbols cover the documented operators plus `@`; `@` is lexed but unused downstream.

## Parser Findings

### Module & Imports
- Enforces `module <path>;` header before items; missing headers trigger a parse error followed by synchronisation.
- `import` accepts dotted paths only; aliasing (`import foo as bar;`) and glob imports are not parsed.

### Items & Visibility
- Handles `pub fn/struct/enum` correctly; `pub impl` falls back to inherent impl parsing but visibility is discarded (consistent with current AST).
- `class` keyword is recognised lexically but has no parser entry point.
- Generic parameters are parsed for functions, structs, and enums; there is no support for where clauses, default type parameters, or const generics.

### Traits & Trait Inheritance
- `trait Name: Parent + Another { .. }` is accepted with `+` separators. Comma-separated parent lists are rejected (lexer treats comma but parser stops at first non-`+`).
- Trait declarations lack generic parameter parsing; `trait Name<T> {}` currently produces an error at `<`.
- Default method bodies and receiver qualifiers (`self`, `&self`, `&mut self`) are parsed and recorded.
- Trait impls require the trait definition to appear earlier in the same file; cross-file trait references still parse but emit an error recorded on the impl span.
- Trait impls cannot express generics on either side (`impl<T> Trait<T> for Type<T>` is not supported).

### Impl Blocks
- Inherent impls parse method lists with receiver variants and typed parameters.
- `impl Type` assumes a simple identifier; qualified paths (`impl module::Type`) and generic type arguments (`impl Type<T>`) are unsupported.

### Statements & Control Flow
- Control-flow constructs implemented: `while`, `do { } while`, `for name in/of`, `loop`, `switch`, `break`, `continue`.
- Reserved keywords `foreach`, `repeat`, `until`, `yield`, `goto` remain unparsed despite being lexed.
- `switch` accepts `case` patterns and an `else` block; the parser expects `else` but error messages mention `default`, leading to confusing diagnostics.
- Assignments only accept identifiers or index expressions on the LHS; destructuring assignments are not allowed.

### Expressions & Calls
- Method chaining (`obj.method().field`) and tuple indexing (`tuple.0`) are supported.
- Struct literals differentiate from enum variants by disallowing `::` in field initialisers.
- Generic type arguments on identifiers (`Type::<T>::Variant`) are parsed, though the semantic layer handles association.
- There is no support for lambda/closure literals, spread operators, or inline `if` expressions without blocks.

### Pattern Ergonomics
- `match` patterns cover wildcard (`_`), identifier bindings, literal patterns, and enum variants with tuple payloads.
- Tuple destructuring, struct patterns, nested `if guard` clauses, and OR-patterns (`A | B`) are not parsed.
- `switch` patterns reuse full expressions rather than the restricted `match` pattern grammar, enabling constructs outside the documented subset.

## Gaps vs. Alpha Plan
1. Trait generics and impl generics remain unparsed despite being part of the planned surface.
2. Pattern ergonomics are limited; tuple/struct patterns and guards need parser support to meet ergonomic goals.
3. Control-flow keywords flagged in docs (`foreach`, `repeat`, `until`, `yield`, `goto`) lack parser stubs, so the lexer-only reservation should be called out as deferred.
4. Import aliasing and module path ergonomics are missing from the parser despite being called out as desirable in the alpha plan.
5. Error messages for `switch` defaults reference `default` while the grammar actually requires `else`, creating user-facing inconsistency.

## Suggested Follow-Up Tasks
1. Extend trait and impl parsing to accept generic parameter lists and apply them to the AST.
2. Introduce parser branches for the reserved control-flow keywords that emit deliberate "deferred" diagnostics instead of generic errors.
3. Expand pattern parsing with tuple/struct destructuring and OR-patterns to align with match ergonomics.
4. Add import aliasing grammar (`import foo.bar as baz;`) and produce placeholders or diagnostics until name resolution lands.
5. Adjust `switch` grammar or diagnostics so the keyword expectations (`else` vs. `default`) are consistent across docs, parser, and error messaging.
