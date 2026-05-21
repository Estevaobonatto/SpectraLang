# Frontend Coverage Audit

Updated: 2026-05-21  
Roadmap item: `R-101`

This document is the Phase 1 source of truth for frontend language coverage. It classifies the lexer and parser surface against the currently documented SpectraLang syntax.

Status labels:

- `supported`: implemented and covered by automated tests
- `gated`: implemented behind `--enable-experimental <feature>`
- `partial`: parses in common cases, but has edge-case or ergonomics gaps
- `deferred`: intentionally not implemented yet

## Lexer Coverage

| Surface | Status | Notes |
| --- | --- | --- |
| ASCII identifiers and keywords | supported | Includes reserved words such as `as`, `dyn`, `pub`, `internal`, `match`, `switch`, `unless`, `loop`. |
| Integer literals | supported | Decimal form only. |
| Float literals | supported | Decimal with a single dot. Scientific notation is deferred. |
| String literals | supported | Basic escapes supported. |
| Char literals | supported | Includes common escapes. |
| F-string literals | supported | Parsed as a dedicated token kind and expanded in the parser. |
| Line comments `//` | supported | Ignored by the token stream. |
| Block comments `/* ... */` | supported | Unterminated comments emit coded lexical diagnostics. |
| Operators `+ - * / % == != <= >= && || -> => .. ..=` | supported | Current operator set matches documented syntax. |
| Symbols `(){}[],:;.=<>!&|?@` | supported | Used across parser and formatter. |
| Unicode identifiers | deferred | The lexer intentionally limits identifiers to ASCII today. |
| Numeric separators / hex / binary | deferred | Covered by future numeric expansion work. |
| Raw strings / multiline string modes | deferred | Not part of the current language contract. |

## Module-Level Syntax Coverage

| Syntax form | Status | Notes |
| --- | --- | --- |
| `module name;` | supported | Required file header. |
| `import std.io;` | supported | Basic module import. |
| `import std.math as math;` | supported | Alias import accepts `as` as a keyword. |
| `import { println, print } from std.io;` | supported | Named import surface is stable. |
| `pub import { println } from std.io;` | supported | Re-export surface is stable. |
| `internal import ...` | supported | Parsed and carried into the AST. |
| `pub fn`, `internal fn` | supported | Visibility-aware item parsing is implemented. |
| `struct`, `enum`, `trait`, `impl`, `type`, `const`, `static` | supported | Parsed in the current frontend. |
| `class` declarations | partial | The keyword is recognized, but the implementation surface is not production complete. |
| Cross-file workspace parsing | supported | Covered by parser workspace tests and project discovery in the CLI. |

## Type Syntax Coverage

| Syntax form | Status | Notes |
| --- | --- | --- |
| Simple types `int`, `string`, `Foo` | supported | |
| Qualified types `std.io.Writer` | supported | |
| Generic types `Option<int>` | supported | |
| Tuple types `(int, string)` | supported | |
| Function types `fn(int) -> int` | supported | |
| Trait objects `dyn Shape` | supported | |
| Higher-kinded types | deferred | |
| Lifetime syntax | deferred | Not part of the language today. |

## Statement Coverage

| Syntax form | Status | Notes |
| --- | --- | --- |
| `let` bindings | supported | Mutable bindings parse via `mut`. |
| Assignment statements | supported | Includes indexed and field-based targets through expression parsing. |
| `return`, `break`, `continue` | supported | |
| Expression statements | supported | Requires `;` when followed by additional statements. |
| `if / elif / else` | supported | |
| `if let` | supported | Implemented as a dedicated parser path. |
| `while` | supported | |
| `while let` | supported | Implemented as a dedicated parser path. |
| `for ... in ...` | supported | |
| `for ... of ...` | supported | Documented canonical collection iteration form. |
| `loop` | gated | Requires `--enable-experimental loop`. |
| `do { } while ...;` | gated | Requires `--enable-experimental do-while`. |
| `switch` | gated | Requires `--enable-experimental switch`. |
| `repeat/until`, `foreach`, `goto`, `yield` | deferred | Tokens exist, parser implementation is intentionally absent. |

## Expression Coverage

| Syntax form | Status | Notes |
| --- | --- | --- |
| Literals and identifiers | supported | |
| Arithmetic and comparison operators | supported | |
| Boolean operators `&&`, `||` | supported | |
| Unary operators `-`, `!` | supported | |
| Function calls | supported | |
| Qualified calls `std.io.println(...)` | supported | Parsed as chained call expressions. |
| Method-style calls `value.method(...)` | supported | |
| Field access | supported | |
| Type casts `expr as Type` | supported | |
| Blocks `{ ... }` as expressions | supported | |
| `if` expressions | supported | |
| `unless` expressions | gated | Requires `--enable-experimental unless`. |
| `match` expressions | supported | |
| Lambda/closure expressions | supported | Current parser surface accepts the documented closure syntax. |
| Array literals | partial | Lower stages do not yet treat them as a production-ready numeric container surface. |
| Comprehensions | deferred | |

## Pattern Coverage

| Syntax form | Status | Notes |
| --- | --- | --- |
| Wildcard `_` | supported | |
| Identifier binding | supported | |
| Literal patterns | supported | |
| Tuple patterns | supported | |
| Enum variant tuple patterns | supported | |
| Enum variant struct patterns | supported | |
| Guarded match arms `case pattern if cond => ...` | supported | |
| OR-patterns | deferred | |
| Slice patterns | deferred | |

## Recovery and Diagnostics Coverage

| Area | Status | Notes |
| --- | --- | --- |
| Missing `;`, `)`, `}` recovery | supported | Synthetic spans prevent stuck parsing. |
| Feature-gate errors | supported | Emit stable parse code `P004`. |
| Import syntax diagnostics | supported | Covered by parser tests for import forms. |
| Infinite-loop recovery regression | supported | Recovery now guarantees progress on error. |
| Full syntax snapshot testing | partial | Parser unit tests exist, but grammar-wide snapshots are part of ongoing test growth. |

## Deferred Frontend Backlog

- `R-203`: expand pattern ergonomics and destructuring beyond the current match surface
- `R-201`: richer numeric literal syntax
- future frontend item: Unicode identifier support
- future frontend item: raw strings and advanced literal modes
- future frontend item: parser snapshot generation from a broader corpus
