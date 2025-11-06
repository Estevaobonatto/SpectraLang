# SpectraLang Alpha Language Reference

_Date frozen: 2025-11-06_

This document captures the currently implemented SpectraLang surface that defines the alpha language scope. It reflects the behaviour of the existing lexer, parser, semantic analyser, and midend as of commit time. Anything not listed here should be considered out of scope for the alpha milestone unless otherwise noted.

## Source File Layout
- Every source file must begin with a `module` declaration: `module path.to.module;`. The parser treats the first tokens as the module header and emits an error if it is missing.
- Zero or more `import` statements may follow. Imports accept dotted paths but do not yet support aliasing or glob patterns.
- Top-level items supported in Alpha:
  - Function definitions (`fn`), with optional `pub` visibility and generic parameters `<T>`.
  - Struct definitions (`struct`), optionally generic, with named fields.
  - Enum definitions (`enum`), optionally generic, with unit or tuple-style variants.
  - Implementation blocks (`impl Type { ... }`) defining methods, including `self`, `&self`, and `&mut self` receivers.
  - Trait declarations (`trait`) with optional parent traits and default method bodies.
  - Trait implementations (`impl Trait for Type { ... }`).
- `class` and `export` keywords are reserved but currently unused.

### Module and Package Semantics
- Module paths use dot-separated identifiers (`module physics.vector;`). By convention the path should mirror the folder hierarchy, but the compiler does not enforce this yet.
- Each source file is compiled independently by the CLI. Cross-file linkage is limited: imports are parsed so the structure is known, but no name resolution or automatic loading of imported files occurs in alpha.
- Duplicate module names within the same compilation session are not detected; the caller is responsible for organising files to avoid conflicts.
- Future package metadata (versioning, manifests) is out of scope for alpha and will be introduced alongside the package manager tooling.

## Lexical Elements
- Line comments start with `//` and run to end-of-line.
- String literals use double quotes; escape sequences are not yet processed.
- Numeric literals support integers and decimals (`123`, `3.14`).
- Identifiers follow the pattern `[A-Za-z_][A-Za-z0-9_]*`.
- All keywords recognised by the lexer:
  `module import export fn struct enum impl class trait let pub mut Self match switch case cond if else elif elseif unless while do for foreach in of repeat until loop return break continue yield goto true false`
- Only the constructs described in this reference are currently parsed; unsupported keywords are listed under Deferred Features.

## Types
- Primitive types: `int`, `float`, `bool`, `string`, `char`.
- The unit type is implicit for statements that yield no value.
- Composite types:
  - Arrays: `[T; N]` syntax is not yet parsed; arrays appear as `Type::Array` internally when created via array literals.
  - Tuples: `(T1, T2, ...)`.
  - Structs: `StructName { field: value, ... }`.
  - Enums: `EnumName::Variant` with optional tuple payload.
- Generic type parameters are accepted in function, struct, enum, and trait signatures, but semantic resolution of generic arguments is limited.
- `Self` denotes the implementing type inside trait/impl contexts.

## Declarations
### Functions
```spectra
pub fn name<T>(param: Type) -> ReturnType {
    // body
}
```
- Parameters may omit type annotations; such cases default to `Unknown` in the semantic analyser.
- Return type defaults to unit if omitted.

### Structs
```spectra
struct Point<T> {
    x: T,
    y: T,
}
```
- All fields require explicit type annotations.

### Enums
```spectra
enum Option<T> {
    Some(T),
    None,
}
```
- Tuple-style variants are supported; struct-style variants are not yet parsed.

### Traits
- Traits may specify parent traits (`trait Child: Parent { ... }`).
- Methods support default implementations.
- Trait methods can use `self`, `&self`, or `&mut self` receivers.

### Impl Blocks
- Inherent impls (`impl Type { ... }`) define methods associated with a type.
- Trait impls (`impl Trait for Type { ... }`) must implement all non-default trait methods; this is validated semantically.

## Statements
- Variable binding: `let name: Type = expression;` (type and initializer optional).
- Assignment: `identifier = expression;` or `array[index] = expression;`.
- Return: `return expr;` or `return;`.
- Expression statements allow trailing `if`/`unless` expressions without semicolons.
- Loops and control flow:
  - `while condition { ... }`
  - `do { ... } while condition;`
  - `for item in iterable { ... }` and `for item of iterable { ... }`
  - `loop { ... }`
  - `switch value { case pattern => { ... } else => { ... } }`
  - `break;`, `continue;`
- `goto`, `repeat`, `until`, `foreach`, and `yield` are reserved but not implemented.

## Expressions
- Literals: integers, floats, strings, booleans.
- Binary operators: `+`, `-`, `*`, `/`, `%`, comparison (`==`, `!=`, `<`, `>`, `<=`, `>=`), logical `&&`, `||`.
- Unary operators: `-`, `!`.
- Grouping: `(expr)`.
- Arrays: `[expr, expr, ...]`.
- Tuples: `(expr1, expr2, ...)`.
- Tuple access: `tuple.0`, `tuple.1`, ...
- Field access: `object.field`.
- Method calls: `object.method(args)`.
- Function calls: `identifier(args)`.
- Index access: `array[index]`.
- Struct literals: `StructName { field: value, ... }`.
- Enum variants: `EnumName::Variant(args)`.
- Conditional expressions: `if / elif / else`, `unless`, with block bodies.
- Pattern matching: `match scrutinee { Pattern => expr, ... }`.

## Pattern Matching
- Match arms use `=>` and support the following patterns:
  - `_` wildcard.
  - Literal patterns (numbers, booleans, strings).
  - Identifier bindings (`value`).
  - Enum variants with nested patterns (`Option::Some(x)`).
- Exhaustiveness checking exists for enums but remains partial.

## Semantics Overview
- Symbol tables are scoped lexically within blocks.
- Basic type checking exists for arithmetic, logical, and assignment expressions; unsupported combinations produce semantic errors.
- Trait implementations are validated to ensure signatures match the trait definition and that all required methods are present.
- Method calls resolve against inherent impls and trait defaults recorded in the semantic analyser.
- Generic argument inference is limited; many cases remain `Unknown` and will be refined post-alpha.

## Deferred / Unsupported Features
These keywords or constructs are tokenised but not yet parsed or semantically validated in alpha:
- `export`
- `class`
- `foreach`
- `repeat` / `until`
- `cond`
- `yield`
- `goto`
- Weak typing mode directives (not implemented)
- Struct-style enum variants (`Variant { field: value }`)
- Module aliasing/import renaming
- Weak typing directives (`#pragma weak` style or attribute-based opt-outs)

## Weak Typing Mode Status
- All compilation currently uses strict (strong) typing rules; there is no directive or flag to switch to a weak mode.
- Any attempt to emulate weak typing (e.g., by omitting annotations) results in `Unknown` types that must be resolved by later passes; implicit coercions are not performed.
- Planned directives for weakening type checks are deferred; once designed, they will be documented as language attributes and gated behind explicit flags.

## Known Limitations for Alpha
- String escape sequences, character literals, and byte literals are not yet supported.
- There is no visibility checking beyond `pub` markers.
- Generic type checking and trait bounds enforcement are incomplete.
- The standard library is not yet defined; examples rely on user-defined constructs.
- Error recovery in the parser is basic; multiple syntax errors may cascade.

## Next Steps
- Track implementation work for deferred features via the alpha checklist (`docs/alpha-checklist.md`).
- Update this reference whenever language surface changes are merged; alpha is considered frozen once dependent tasks are complete.
