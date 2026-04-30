# SpectraLang — Complete Language Reference for AI Agents

**Version:** Alpha  
**Source:** Verified against the compiler source code and working example projects (`app_project`, `test_project`, `complex_demo`).  
**Purpose:** This document is the authoritative reference for AI agents generating SpectraLang code. Every rule is stated unambiguously. All code examples are known to compile and run correctly.

---

## Table of Contents

1. [Language Overview](#1-language-overview)
2. [Source File Structure](#2-source-file-structure)
3. [Module System & Imports](#3-module-system--imports)
4. [Primitive Types](#4-primitive-types)
5. [Variables](#5-variables)
6. [Operators](#6-operators)
7. [Functions](#7-functions)
8. [Control Flow](#8-control-flow)
9. [Arrays](#9-arrays)
10. [Tuples](#10-tuples)
11. [Structs](#11-structs)
12. [Enums](#12-enums)
13. [Impl Blocks & Methods](#13-impl-blocks--methods)
14. [Traits](#14-traits)
15. [Pattern Matching](#15-pattern-matching)
16. [Generics](#16-generics)
17. [Closures / Lambdas](#17-closures--lambdas)
18. [F-Strings](#18-f-strings)
19. [Type Casting](#19-type-casting)
20. [Constants & Statics](#20-constants--statics)
21. [Type Aliases](#21-type-aliases)
22. [Visibility](#22-visibility)
23. [Standard Library](#23-standard-library)
24. [Multi-Module Projects](#24-multi-module-projects)
25. [CLI Reference](#25-cli-reference)
26. [Complete Working Examples](#26-complete-working-examples)

---

## 1. Language Overview

SpectraLang is a statically typed, compiled, general-purpose language that:
- Compiles to native machine code via Cranelift (JIT and AOT modes)
- Has strict static typing — no implicit conversions except `int` → `float`
- Supports modules, generics, traits, enums, structs, and closures
- Every source file is one module; each module is one file
- The entry point is always `pub fn main() -> int { ... }`

**File extension:** `.spectra` or `.spc`

---

## 2. Source File Structure

Every `.spectra` file must follow this exact order:

```
1. module declaration   (MANDATORY — first non-comment line)
2. import statements    (optional, zero or more)
3. top-level items      (functions, structs, enums, traits, impls, consts, statics, type aliases)
```

### Module Declaration

```spectra
module module_name;
```

- The module name becomes the public identity of the file.
- Module names use dot-separated identifiers for logical grouping: `module app.utils;`
- By convention, the name mirrors the file path, but the compiler does not enforce this.
- The CLI uses the module name declared in the file for dependency resolution.

### Minimal Valid Program

```spectra
module hello;

import std.io;

pub fn main() -> int {
    println("Hello, World!");
    return 0;
}
```

### Rules

- `module` must be the first non-comment, non-blank line.
- Only `//` line comments are supported. Block comments (`/* */`) are **not** supported.
- The entry point function is `pub fn main() -> int { ... }`.
- A file without a `module` declaration uses the file stem as its module name (CLI behavior).

---

## 3. Module System & Imports

### Import Forms

```spectra
// Form 1: import entire module (use fully-qualified names)
import std.math;
// Usage: std.math.abs(-5)   or after import:  abs(-5)  when unqualified

// Form 2: import stdlib module — all exported names become available unqualified
import std.io;
// Usage: println("hello")

// Form 3: import user module — all exported names become available unqualified
import my_module;
// Usage: my_func(), MyStruct { ... }, MyEnum::Variant

// Form 4: qualified module access (cross-module calls)
import cx_geometry;
// Usage: cx_geometry::make_vec2(3, 4)
//        cx_geometry::Shape::Circle(5)
```

### Stdlib Module Names

| Import Statement | Module |
|---|---|
| `import std.io;` | I/O operations |
| `import std.math;` | Math functions |
| `import std.string;` | String operations |
| `import std.convert;` | Type conversions |
| `import std.collections;` | Dynamic lists |
| `import std.random;` | Random numbers |
| `import std.fs;` | File system |
| `import std.env;` | Environment & args |
| `import std.option;` | Option helpers |
| `import std.result;` | Result helpers |
| `import std.char;` | Char classification |
| `import std.time;` | Timestamps, sleep |

### User Module Import

To use functions/types from another `.spectra` file in the same project:
```spectra
// In main.spectra
import sa_math;   // imports sa_math.spectra
import sa_grades; // imports sa_grades.spectra

pub fn main() -> int {
    let g = gcd(48, 18);         // from sa_math (unqualified)
    let grade = score_to_grade(95); // from sa_grades (unqualified)
    return 0;
}
```

### Qualified Module Access

When calling a function or constructing a type from a named user module, use `ModuleName::`:
```spectra
import cx_geometry;

pub fn main() -> int {
    let s = cx_geometry::Shape::Circle(5);
    let area = cx_geometry::shape_area(s);
    return 0;
}
```

### Re-exports

```spectra
pub import some_module;  // makes some_module's public symbols available to importers of this module
```

---

## 4. Primitive Types

| Type | Description | Literal Examples |
|------|-------------|-----------------|
| `int` | 64-bit signed integer | `0`, `42`, `-7`, `1_000` |
| `float` | 64-bit IEEE 754 double | `3.14`, `-0.5`, `1.0` |
| `bool` | Boolean | `true`, `false` |
| `string` | UTF-8 string | `"hello"`, `""` |
| `char` | Unicode code point | `'a'`, `'\n'`, `'\t'` |

**Unit type:** Functions with no return value or returning no meaningful result use `unit` (written as `()` in type contexts or simply omitted as the return type).

### Type Compatibility Rules

- **Allowed implicit conversion:** `int` → `float` in arithmetic expressions only.
- **All other conversions are explicit** — use `std.convert` functions.
- `bool` is **not** an integer. `bool + int` is a compile error.
- `int` and `float` cannot be used in the same arithmetic expression without explicit conversion.

---

## 5. Variables

### Declaration

```spectra
let name = value;           // type inferred
let name: Type = value;     // explicit type
let name: Type;             // declared without initializer (type required)
```

### Reassignment

All local variables are mutable by default. Simply assign a new value:

```spectra
let x = 10;
x = 20;           // valid — x is now 20
x = x + 1;        // valid
```

### Rules

- `let` declares a new variable.
- Variables are scoped to their enclosing block `{ ... }`.
- There is no `const` evaluation for local variables — use top-level `const` for compile-time constants.
- `let mut` is accepted syntactically but all `let` variables are mutable by default.

### Examples

```spectra
module vars;

pub fn main() -> int {
    let count = 0;
    let name: string = "Alice";
    let flag: bool = true;
    let ratio: float = 1.5;

    count = count + 1;   // reassignment
    flag = false;

    return count;
}
```

---

## 6. Operators

### Arithmetic

| Operator | Operation | Types |
|----------|-----------|-------|
| `+` | Addition | `int`, `float` |
| `-` | Subtraction | `int`, `float` |
| `*` | Multiplication | `int`, `float` |
| `/` | Division (integer division for `int`) | `int`, `float` |
| `%` | Modulo | `int` |
| `-x` | Unary negation | `int`, `float` |

### Comparison

| Operator | Operation |
|----------|-----------|
| `==` | Equal |
| `!=` | Not equal |
| `<` | Less than |
| `>` | Greater than |
| `<=` | Less or equal |
| `>=` | Greater or equal |

### Logical

| Operator | Operation |
|----------|-----------|
| `&&` | Logical AND |
| `\|\|` | Logical OR |
| `!` | Logical NOT |

### Other

| Operator | Meaning |
|----------|---------|
| `..` | Exclusive range: `0..10` = 0 to 9 |
| `..=` | Inclusive range: `0..=10` = 0 to 10 |
| `as` | Type cast: `x as float` |
| `?` | Error propagation (try operator) |

### Compound Assignment

| Operator | Equivalent |
|----------|-----------|
| `+=` | `x = x + rhs` |
| `-=` | `x = x - rhs` |
| `*=` | `x = x * rhs` |
| `/=` | `x = x / rhs` |
| `%=` | `x = x % rhs` |

### Operator Precedence (highest to lowest)

1. `(expr)`, `f()`, `x.field`, `x[i]` — primary
2. `-x`, `!x` — unary
3. `*`, `/`, `%` — multiplicative
4. `+`, `-` — additive
5. `<`, `>`, `<=`, `>=` — relational
6. `==`, `!=` — equality
7. `&&` — logical AND
8. `||` — logical OR

---

## 7. Functions

### Declaration Syntax

```spectra
// Private function (default)
fn name(param1: Type1, param2: Type2) -> ReturnType {
    // body
}

// Public function
pub fn name(param1: Type1) -> ReturnType {
    // body
}

// Function with no return value (unit return)
pub fn greet(name: string) {
    println(f"Hello, {name}!");
}

// Generic function
fn identity<T>(x: T) -> T {
    return x;
}

// Generic function with trait bound
fn process<T: Clone>(item: T) -> int {
    return item.clone();
}
```

### Return

- `return expr;` — explicit return.
- The last expression in a block (without a semicolon) is also a return value.
- `return;` — returns `unit`.

```spectra
fn add(a: int, b: int) -> int {
    return a + b;      // explicit return
}

fn add2(a: int, b: int) -> int {
    a + b              // implicit return (last expression, no semicolon)
}
```

### Entry Point

Every runnable program must have exactly one `pub fn main() -> int { ... }`. The return value of `main` becomes the process exit code (0 = success).

```spectra
pub fn main() -> int {
    // program logic
    return 0;
}
```

### Recursion

Functions can call themselves recursively:

```spectra
pub fn factorial(n: int) -> int {
    if n <= 1 {
        return 1;
    }
    return n * factorial(n - 1);
}
```

---

## 8. Control Flow

### if / elif / else

```spectra
if condition {
    // ...
} elif other_condition {
    // ...
} else {
    // ...
}
```

- `elif` and `elseif` are both valid (identical behavior).
- Conditions must be `bool` — no implicit bool coercion.
- Braces `{ }` are always required.

```spectra
fn classify(score: int) -> int {
    if score >= 90 {
        return 4;
    } elif score >= 70 {
        return 3;
    } elif score >= 50 {
        return 2;
    } else {
        return 1;
    }
}
```

### unless

`unless condition { ... }` is equivalent to `if !condition { ... }`.

```spectra
unless value < 0 {
    println("value is non-negative");
}

// With else:
unless flag {
    // runs when flag is false
} else {
    // runs when flag is true
}
```

### while

```spectra
while condition {
    // body
}
```

```spectra
let i = 0;
while i < 10 {
    i = i + 1;
}
```

### do-while

```spectra
do {
    // body
} while condition;
```

```spectra
let x = 0;
do {
    x = x + 1;
} while x < 5;
// x is 5 after this
```

### for (range and iterator)

```spectra
// Exclusive range: i = 0, 1, 2, ..., 9
for i in 0..10 {
    // ...
}

// Inclusive range: i = 0, 1, 2, ..., 10
for i in 0..=10 {
    // ...
}

// Array iteration
let arr = [10, 20, 30, 40, 50];
for item in arr {
    println(item);
}
```

Note: `for x of iterable` is also valid syntax (identical to `for x in iterable`).

### loop (infinite loop)

```spectra
loop {
    // body
    if condition {
        break;
    }
}
```

### break and continue

```spectra
let i = 0;
while i < 100 {
    if i == 5 { break; }      // exit loop
    if i % 2 == 0 { continue; } // skip even
    i = i + 1;
}
```

### switch

`switch` compares a value against literal cases. Use `_` or omit for default:

```spectra
switch day {
    case 1 => {
        println("Monday");
    }
    case 2 => {
        println("Tuesday");
    }
    case 3 => {
        println("Wednesday");
    }
}
```

With default:
```spectra
switch option {
    case 1 => { result = 10; }
    case 2 => { result = 20; }
    _ => { result = 0; }
}
```

**Note:** `switch` is an experimental feature. Enable with `--enable-experimental switch` or just use `match` instead, which is the preferred construct.

---

## 9. Arrays

### Creation

```spectra
let arr = [1, 2, 3, 4, 5];               // inferred type: [int]
let arr: [int] = [10, 20, 30];           // explicit
let matrix = [[1, 2], [3, 4]];           // nested arrays
```

### Access and Modification

```spectra
let arr = [10, 20, 30, 40, 50];
let first = arr[0];     // 10
let last = arr[4];      // 50

arr[2] = 99;            // set element
```

### Iteration

```spectra
let arr = [1, 2, 3, 4, 5];
let sum = 0;
let i = 0;
while i < 5 {
    sum = sum + arr[i];
    i = i + 1;
}
```

Or with `for`:
```spectra
let sum = 0;
for x in arr {
    sum = sum + x;
}
```

### Rules

- Arrays are zero-indexed.
- Out-of-bounds access at runtime causes a panic.
- Array size is fixed at creation (for stack arrays).
- For dynamic arrays, use `std.collections.list_*` functions.

---

## 10. Tuples

### Creation

```spectra
let t = (1, "hello", true);              // (int, string, bool)
let pair: (int, int) = (10, 20);
```

### Access

Use `.0`, `.1`, `.2`, ... to access tuple elements:

```spectra
let t = (42, "world");
let n = t.0;    // 42
let s = t.1;    // "world"
```

---

## 11. Structs

### Declaration

```spectra
struct Point {
    x: int,
    y: int,
}

// With visibility modifiers on fields
pub struct Person {
    pub name: string,
    pub age: int,
}

// Generic struct
struct Wrapper<T> {
    value: T,
}
```

### Instantiation

```spectra
let p = Point { x: 3, y: 4 };
let person = Person { name: "Alice", age: 30 };
```

### Field Access

```spectra
let x = p.x;    // 3
let y = p.y;    // 4
```

### Field Mutation

```spectra
p.x = 10;       // direct field assignment
```

### Rules

- All fields require explicit type annotations.
- Fields are private by default — only accessible within `impl` blocks unless declared `pub`.
- When instantiating, all fields must be provided (no defaults).
- Field names must match exactly (case-sensitive).

---

## 12. Enums

### Unit Variants

```spectra
pub enum Direction {
    North,
    South,
    East,
    West,
}
```

Usage:
```spectra
let d = Direction::North;
let code = match d {
    Direction::North => 1,
    Direction::South => 2,
    Direction::East  => 3,
    Direction::West  => 4,
};
```

### Tuple Variants (carrying data)

```spectra
pub enum Shape {
    Circle(int),           // radius
    Rect(int, int),        // width, height
    Dot,                   // unit variant
}

pub enum Status {
    Active(int),
    Inactive,
    Error(int),
}
```

Usage:
```spectra
let s = Shape::Circle(5);
let area = match s {
    Shape::Circle(r)  => r * r * 3,
    Shape::Rect(w, h) => w * h,
    Shape::Dot        => 0,
};

let status = Status::Active(200);
let code = match status {
    Status::Active(v) => v,
    Status::Inactive  => 0,
    Status::Error(e)  => e * -1,
};
```

### Struct Variants (named fields)

```spectra
enum Event {
    Click { x: int, y: int },
    KeyPress { key: int },
    Quit,
}
```

Usage:
```spectra
let e = Event::Click { x: 10, y: 20 };
match e {
    Event::Click { x, y } => {
        println(f"Click at {x},{y}");
    }
    Event::KeyPress { key } => {
        println(key);
    }
    Event::Quit => {
        println("quit");
    }
}
```

### Built-in Generic Enums

SpectraLang provides `Option<T>` and `Result<T, E>` as built-in generic enums:

```spectra
// Option<T>
let some_val: Option<int> = Option::Some(42);
let none_val: Option<int> = Option::None;

let result = match some_val {
    Option::Some(v) => v,
    Option::None    => 0,
};

// Result<T, E>
let ok_val: Result<int, string> = Result::Ok(100);
let err_val: Result<int, string> = Result::Err("failed");
```

### Rules

- Enum variants are always accessed with `EnumName::VariantName`.
- When matching a cross-module enum, the bare `VariantName` (without `EnumName::`) can be used in patterns if the enum type is clear from context.
- Generic enums require type arguments in some contexts: `Option<int>`, `Result<int, string>`.

---

## 13. Impl Blocks & Methods

### Inherent Impl (adding methods to a type)

```spectra
struct Counter {
    value: int,
    step: int,
}

impl Counter {
    // Static constructor (no self)
    fn new(step: int) -> Counter {
        Counter { value: 0, step: step }
    }

    // Immutable method (&self — read-only access)
    pub fn get(&self) -> int {
        self.value
    }

    // Returns a new value (pure functional style)
    pub fn increment(&self) -> Counter {
        Counter { value: self.value + self.step, step: self.step }
    }

    // Mutable method (&mut self)
    pub fn reset(&mut self) {
        self.value = 0;
    }
}
```

Usage:
```spectra
let c = Counter::new(5);    // static call: TypeName::method_name(args)
let c2 = c.increment();     // method call: instance.method_name(args)
let v = c2.get();           // 5
```

### Method Receivers

| Receiver | Syntax | Meaning |
|----------|--------|---------|
| None | `fn new(...)` | Static/associated function — called as `Type::new(...)` |
| Value | `fn method(self)` | Takes ownership of self |
| Immutable ref | `fn method(&self)` | Read-only access to self's fields |
| Mutable ref | `fn method(&mut self)` | Mutable access to self's fields |

### Self Inside Methods

Inside an `impl` block, `self` refers to the instance. Field access uses `self.field_name`:

```spectra
impl Point {
    pub fn magnitude_sq(&self) -> int {
        self.x * self.x + self.y * self.y
    }
}
```

### Visibility in Impl Blocks

- Methods inside `impl Type { }` are **private** by default.
- Prefix with `pub` to make them callable from outside the module.
- Methods inside `impl Trait for Type { }` are **always public**.

---

## 14. Traits

### Trait Declaration

```spectra
trait Shape {
    fn area(&self) -> int;                    // abstract method (no body)
    fn perimeter(&self) -> int;               // abstract method
    fn describe(&self) -> int {               // default implementation
        return 0;
    }
}

// Trait with parent trait (inheritance)
trait Scalable: Shape {
    fn scale(&self, factor: int) -> int;
}
```

### Trait Implementation

```spectra
struct Circle {
    radius: int,
}

impl Shape for Circle {
    fn area(&self) -> int {
        self.radius * self.radius * 3
    }

    fn perimeter(&self) -> int {
        self.radius * 6
    }
    // describe() uses default implementation
}
```

### Calling Trait Methods

Once a type implements a trait, call its methods like regular methods:

```spectra
let c = Circle { radius: 5 };
let a = c.area();         // 75
let p = c.perimeter();    // 30
```

### Rules

- All non-default trait methods must be implemented.
- Default methods may be overridden in the impl.
- Trait method signatures in the impl must exactly match the trait declaration.
- `Self` inside a trait refers to the type implementing the trait.

### Self Keyword in Traits

```spectra
trait Clone {
    fn clone(self) -> Self;     // Self = implementing type
}

impl Clone for Point {
    fn clone(self) -> Point {   // Point substituted for Self
        return Point { x: self.x, y: self.y };
    }
}
```

---

## 15. Pattern Matching

### match Expression

```spectra
match scrutinee {
    Pattern1 => expression_or_block,
    Pattern2 => expression_or_block,
    _ => default_expression,
}
```

### Pattern Types

#### Wildcard

```spectra
match value {
    _ => println("catch-all"),
}
```

#### Literal Patterns

```spectra
match score {
    100 => println("perfect"),
    0   => println("zero"),
    _   => println("other"),
}
```

#### Binding Patterns (capture the value)

```spectra
match x {
    n => println(n),    // n is bound to the value of x
}
```

#### Enum Unit Variant Patterns

```spectra
match direction {
    Direction::North => 1,
    Direction::South => 2,
    Direction::East  => 3,
    Direction::West  => 4,
}
```

#### Enum Tuple Variant Patterns (destructuring)

```spectra
match status {
    Status::Active(code) => code,
    Status::Inactive     => 0,
    Status::Error(e)     => e * -1,
}
```

Multiple fields:
```spectra
match shape {
    Shape::Circle(r)    => r * r * 3,
    Shape::Rect(w, h)   => w * h,
    Shape::Dot          => 0,
}
```

#### Enum Struct Variant Patterns

```spectra
match event {
    Event::Click { x, y } => {
        return x + y;
    }
    Event::KeyPress { key } => {
        return key;
    }
    Event::Quit => {
        return 0;
    }
}
```

#### Nested Patterns

```spectra
match wrapped {
    Option::Some(inner) => match inner {
        Option::Some(value) => value,
        Option::None => 0,
    },
    Option::None => 0,
}
```

### Match Arm Body

A match arm body can be:
- A single expression: `Pattern => expr,`
- A block: `Pattern => { stmts; expr }`
- A block with return: `Pattern => { return value; }`

```spectra
let result = match x {
    0 => 0,
    1 => 1,
    n => n * 2,
};
```

### Exhaustiveness

Every `match` must cover all possible cases. Always add `_ =>` if not all cases are listed explicitly.

### Cross-Module Enum Matching

When matching an enum imported from another module, you can use bare variant names in patterns (the enum type is inferred from the scrutinee):

```spectra
import cx_geometry;

let shape = cx_geometry::Shape::Circle(10);
let area = match shape {
    Shape::Circle(r)  => r * r * 3,   // bare variant name is OK in patterns
    Shape::Rect(w, h) => w * h,
    Shape::Dot        => 0,
};
```

### if let

Pattern-match and bind in a conditional:

```spectra
if let Option::Some(v) = maybe_value {
    println(v);
} else {
    println("nothing");
}
```

### while let

Loop while a pattern matches:

```spectra
while let Option::Some(v) = get_next() {
    process(v);
}
```

---

## 16. Generics

### Generic Functions

```spectra
fn identity<T>(x: T) -> T {
    return x;
}

fn first<T>(a: T, b: T) -> T {
    return a;
}
```

### Generic Functions with Trait Bounds

```spectra
fn process<T: Clone>(item: T) -> int {
    return item.clone();
}

// Multiple bounds with +
fn debug_and_clone<T: Debug + Clone>(item: T) -> int {
    return item.debug();
}
```

### Generic Structs

```spectra
struct Wrapper<T> {
    value: T,
}

let w = Wrapper { value: 42 };
let v = w.value;    // 42
```

### Generic Enums

```spectra
enum Option<T> {
    Some(T),
    None,
}

enum Result<T, E> {
    Ok(T),
    Err(E),
}
```

Usage with type annotation when needed:
```spectra
let opt: Option<int> = Option::Some(100);
let res: Result<int, string> = Result::Ok(42);
```

### Rules

- Generic type parameters use `PascalCase` names (`T`, `E`, `Key`, `Value`).
- Generic argument inference is limited — when the compiler cannot infer the type, add explicit type annotations.
- Generic implementations work via monomorphization (the compiler generates specialized versions for each concrete type used).

---

## 17. Closures / Lambdas

### Syntax

```spectra
|param1, param2| expression

|param1: Type, param2: Type| expression

|param| {
    // multi-line body
    expression
}
```

### Examples

```spectra
let double = |x| x * 2;
let add = |a, b| a + b;

let result = double(5);    // 10
let sum = add(3, 4);       // 7
```

With type annotations:
```spectra
let multiply: fn(int, int) -> int = |a: int, b: int| a * b;
```

### Passing Closures to Functions

Closures are used with higher-order stdlib functions:
```spectra
import std.collections;

let lst = list_new();
list_push(lst, 1);
list_push(lst, 2);
list_push(lst, 3);

let doubled = list_map(lst, |x| x * 2);
let evens = list_filter(lst, |x| x % 2 == 0);
let total = list_reduce(lst, 0, |acc, x| acc + x);
```

---

## 18. F-Strings

F-strings allow inline expression interpolation:

```spectra
let name = "Alice";
let age = 30;
let greeting = f"Hello, {name}! You are {age} years old.";
println(greeting);
// Output: Hello, Alice! You are 30 years old.
```

### Rules

- Prefix the string literal with `f`: `f"text {expr} more text"`
- Expressions inside `{}` are evaluated and converted to string automatically.
- F-strings produce a `string` value.
- Any expression can be interpolated: variables, function calls, arithmetic.

```spectra
let x = 10;
let y = 20;
println(f"Sum: {x + y}");              // Sum: 30
println(f"Area: {3.14 * 5.0 * 5.0}"); // Area: 78.5
```

---

## 19. Type Casting

### as Operator

Cast between numeric types:

```spectra
let n: int = 42;
let f: float = n as float;     // int → float

let pi: float = 3.14;
let i: int = pi as int;        // float → int (truncates)
```

### std.convert Functions

For all other conversions:

```spectra
import std.convert;

let n = 42;
let s = int_to_string(n);           // "42"
let f = int_to_float(n);            // 42.0

let text = "123";
let parsed = string_to_int(text);   // 123
let safe = string_to_int_or(text, 0); // 0 if parse fails
```

---

## 20. Constants & Statics

### Constants

Compile-time constant — value is evaluated at compile time:

```spectra
const MAX_SIZE: int = 1000;
const PI: float = 3.14159;
pub const VERSION: string = "1.0.0";
```

### Statics

Module-level mutable variable — initialized once at startup:

```spectra
static counter: int = 0;
pub static global_flag: bool = false;
```

### Rules

- `const` values cannot be mutated.
- `static` variables are mutable globals.
- Both `const` and `static` can have `pub` visibility.
- Type annotation is optional but recommended.

---

## 21. Type Aliases

```spectra
type Score = int;
type Name = string;
pub type IntPair = (int, int);
```

Usage:
```spectra
let s: Score = 95;
let n: Name = "Alice";
```

---

## 22. Visibility

| Modifier | Scope |
|----------|-------|
| `pub` | Public — accessible from any module that imports this one |
| `internal` | Internal — accessible only within the same package |
| *(none)* | Private — accessible only within this module |

### Visibility Rules

- Functions: `pub fn`, `fn` (private)
- Structs: `pub struct`, `struct` (private)
- Struct fields: `pub field: Type`, `field: Type` (private — only accessible from `impl` blocks)
- Enums: `pub enum`, `enum` (private)
- Impl methods: `pub fn`, `fn` (private by default in inherent impls)
- Trait impl methods: always public
- Constants/statics: `pub const`, `const`, `pub static`, `static`

### Public Visibility Requirement for Cross-Module Use

For a function, struct, or enum to be usable from another module, it must be declared `pub`. Fields that need to be accessed outside of the type's own `impl` block must also be `pub`.

```spectra
// sa_student.spectra
pub struct Student {         // pub struct — accessible from other modules
    id: int,                 // private field — only accessible in impl Student
    pub name: string,        // pub field — accessible everywhere
}

pub fn student_average(...) -> int { ... }   // pub fn — accessible from other modules
fn internal_helper() -> int { ... }          // private — only within this module
```

---

## 23. Standard Library

All stdlib functions become available unqualified after importing their module. For example, `import std.io;` makes `println`, `print`, etc. available directly.

### std.io — Input / Output

```spectra
import std.io;
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `println` | `(value: any) -> unit` | Print value + newline to stdout |
| `print` | `(value: any) -> unit` | Print value (no newline) to stdout |
| `eprint` | `(value: any) -> unit` | Print value (no newline) to stderr |
| `eprintln` | `(value: any) -> unit` | Print value + newline to stderr |
| `flush` | `() -> unit` | Flush stdout buffer |
| `read_line` | `() -> string` | Read one line from stdin (strips newline) |
| `input` | `(prompt: string) -> string` | Print prompt, flush, read line |

```spectra
import std.io;

println("Hello!");              // Hello!\n
print("Enter: ");
let line = read_line();
let name = input("Name: ");
```

### std.math — Mathematics

```spectra
import std.math;
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `abs` | `(n: int) -> int` | Absolute value |
| `min` | `(a: int, b: int) -> int` | Minimum of two ints |
| `max` | `(a: int, b: int) -> int` | Maximum of two ints |
| `clamp` | `(v: int, lo: int, hi: int) -> int` | Clamp v to [lo, hi] |
| `sign` | `(n: int) -> int` | Returns -1, 0, or 1 |
| `gcd` | `(a: int, b: int) -> int` | Greatest common divisor |
| `lcm` | `(a: int, b: int) -> int` | Least common multiple |
| `sqrt_f` | `(x: float) -> float` | Square root |
| `pow_f` | `(base: float, exp: float) -> float` | Power |
| `floor_f` | `(x: float) -> float` | Floor |
| `ceil_f` | `(x: float) -> float` | Ceiling |
| `round_f` | `(x: float) -> float` | Round |
| `sin_f` | `(x: float) -> float` | Sine |
| `cos_f` | `(x: float) -> float` | Cosine |
| `tan_f` | `(x: float) -> float` | Tangent |
| `log_f` | `(x: float) -> float` | Natural log |
| `log2_f` | `(x: float) -> float` | Log base 2 |
| `log10_f` | `(x: float) -> float` | Log base 10 |
| `atan2_f` | `(y: float, x: float) -> float` | atan2 |
| `pi` | `() -> float` | π constant |
| `e_const` | `() -> float` | e constant |
| `abs_f` | `(x: float) -> float` | Absolute value (float) |
| `is_nan_f` | `(x: float) -> bool` | Is NaN? |
| `is_infinite_f` | `(x: float) -> bool` | Is infinite? |

```spectra
import std.math;

let m = max(10, 20);                  // 20
let a = abs(-5);                      // 5
let r = sqrt_f(16.0);                 // 4.0
let pi_val = pi();                    // 3.14159...
let clamped = clamp(150, 0, 100);    // 100
```

### std.string — String Operations

```spectra
import std.string;
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `len` | `(s: string) -> int` | Length in bytes |
| `contains` | `(s: string, sub: string) -> bool` | Contains substring? |
| `starts_with` | `(s: string, prefix: string) -> bool` | Starts with prefix? |
| `ends_with` | `(s: string, suffix: string) -> bool` | Ends with suffix? |
| `to_upper` | `(s: string) -> string` | Uppercase |
| `to_lower` | `(s: string) -> string` | Lowercase |
| `trim` | `(s: string) -> string` | Remove leading/trailing whitespace |
| `concat` | `(a: string, b: string) -> string` | Concatenate two strings |
| `substring` | `(s: string, start: int, end: int) -> string` | Slice [start, end) |
| `replace` | `(s: string, from: string, to: string) -> string` | Replace all occurrences |
| `index_of` | `(s: string, sub: string) -> int` | First occurrence index (-1 if not found) |
| `split_first` | `(s: string, sep: string) -> string` | Part before first sep |
| `split_last` | `(s: string, sep: string) -> string` | Part after last sep |
| `split_by` | `(s: string, sep: string) -> int` | Split into list (returns list handle) |
| `is_empty` | `(s: string) -> bool` | Is empty? |
| `count_occurrences` | `(s: string, sub: string) -> int` | Count occurrences |
| `char_at` | `(s: string, i: int) -> int` | Char code at index (-1 if OOB) |
| `repeat_str` | `(s: string, n: int) -> string` | Repeat string n times |
| `pad_left` | `(s: string, width: int, pad_char: int) -> string` | Left-pad with char |
| `pad_right` | `(s: string, width: int, pad_char: int) -> string` | Right-pad with char |
| `reverse_str` | `(s: string) -> string` | Reverse |

```spectra
import std.string;

let s = "  Hello, World!  ";
let trimmed = trim(s);                          // "Hello, World!"
let upper = to_upper("hello");                  // "HELLO"
let n = len("abc");                             // 3
let has = contains("foobar", "oba");            // true
let joined = concat("foo", "bar");              // "foobar"
let sub = substring("hello", 1, 4);            // "ell"
```

### std.convert — Type Conversions

```spectra
import std.convert;
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `int_to_string` | `(n: int) -> string` | Int to string |
| `float_to_string` | `(f: float) -> string` | Float to string |
| `bool_to_string` | `(b: bool) -> string` | Bool to "true"/"false" |
| `string_to_int` | `(s: string) -> int` | Parse int (0 on error) |
| `string_to_float` | `(s: string) -> float` | Parse float (0.0 on error) |
| `string_to_int_or` | `(s: string, default: int) -> int` | Parse int with fallback |
| `string_to_float_or` | `(s: string, default: float) -> float` | Parse float with fallback |
| `string_to_bool` | `(s: string) -> bool` | "true" → true |
| `int_to_float` | `(n: int) -> float` | Int to float |
| `float_to_int` | `(f: float) -> int` | Float to int (truncates) |
| `bool_to_int` | `(b: bool) -> int` | true→1, false→0 |

```spectra
import std.convert;

let s = int_to_string(42);          // "42"
let n = string_to_int("123");       // 123
let f = int_to_float(7);            // 7.0
let i = float_to_int(3.99);         // 3 (truncated)
```

### std.collections — Dynamic Lists

```spectra
import std.collections;
```

Lists store `int` values. Use int handles to pass lists between functions. String values are stored as integer pointers.

| Function | Signature | Description |
|----------|-----------|-------------|
| `list_new` | `() -> int` | Create new list; returns handle |
| `list_push` | `(h: int, v: int) -> unit` | Append value |
| `list_len` | `(h: int) -> int` | Length |
| `list_get` | `(h: int, i: int) -> int` | Get element (-1 if OOB) |
| `list_set` | `(h: int, i: int, v: int) -> unit` | Set element |
| `list_pop` | `(h: int) -> int` | Remove and return last (-1 if empty) |
| `list_pop_front` | `(h: int) -> int` | Remove and return first (-1 if empty) |
| `list_insert_at` | `(h: int, i: int, v: int) -> unit` | Insert at index |
| `list_remove_at` | `(h: int, i: int) -> int` | Remove at index (-1 if OOB) |
| `list_contains` | `(h: int, v: int) -> bool` | Contains value? |
| `list_index_of` | `(h: int, v: int) -> int` | First index (-1 if not found) |
| `list_sort` | `(h: int) -> unit` | Sort ascending in-place |
| `list_sort_by` | `(h: int, cmp: fn(int,int)->int) -> unit` | Sort with comparator |
| `list_map` | `(h: int, f: fn(int)->int) -> int` | Map → new list handle |
| `list_filter` | `(h: int, f: fn(int)->bool) -> int` | Filter → new list handle |
| `list_reduce` | `(h: int, init: int, f: fn(int,int)->int) -> int` | Reduce to single value |
| `list_clear` | `(h: int) -> unit` | Remove all elements |
| `list_free` | `(h: int) -> unit` | Free list memory |
| `list_free_all` | `() -> int` | Free all lists |

```spectra
import std.collections;

let lst = list_new();
list_push(lst, 10);
list_push(lst, 20);
list_push(lst, 30);

let n = list_len(lst);           // 3
let first = list_get(lst, 0);    // 10

let doubled = list_map(lst, |x| x * 2);
let total = list_reduce(lst, 0, |acc, x| acc + x);  // 60

list_free(lst);
```

### std.random — Random Numbers

```spectra
import std.random;
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `random_seed` | `(seed: int) -> unit` | Set RNG seed |
| `random_int` | `(min: int, max: int) -> int` | Random int in [min, max] |
| `random_float` | `() -> float` | Random float in [0.0, 1.0) |
| `random_bool` | `() -> bool` | Random bool |

```spectra
import std.random;

random_seed(42);
let n = random_int(1, 100);    // 1 to 100
let f = random_float();        // 0.0 to 0.999...
let b = random_bool();
```

### std.fs — File System

```spectra
import std.fs;
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `fs_read` | `(path: string) -> string` | Read entire file ("" on error) |
| `fs_write` | `(path: string, content: string) -> bool` | Write file |
| `fs_append` | `(path: string, content: string) -> bool` | Append to file |
| `fs_exists` | `(path: string) -> bool` | File exists? |
| `fs_remove` | `(path: string) -> bool` | Delete file |

```spectra
import std.fs;

let ok = fs_write("output.txt", "Hello\n");
let content = fs_read("output.txt");
let exists = fs_exists("output.txt");
```

### std.env — Environment

```spectra
import std.env;
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `env_get` | `(key: string) -> string` | Get env var ("" if not set) |
| `env_set` | `(key: string, value: string) -> bool` | Set env var |
| `env_args_count` | `() -> int` | Number of CLI arguments |
| `env_arg` | `(index: int) -> string` | Argument at index ("" if OOB) |

```spectra
import std.env;

let home = env_get("HOME");
let argc = env_args_count();
let first_arg = env_arg(0);
```

### std.option — Option Helpers

```spectra
import std.option;
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `is_some` | `(opt: any) -> bool` | Is Some? |
| `is_none` | `(opt: any) -> bool` | Is None? |
| `option_unwrap` | `(opt: any) -> any` | Unwrap (panics on None) |
| `option_unwrap_or` | `(opt: any, default: any) -> any` | Unwrap or default |

```spectra
import std.option;

let v: Option<int> = Option::Some(42);
let has = is_some(v);                      // true
let val = option_unwrap(v);                // 42
let safe = option_unwrap_or(v, 0);        // 42
```

### std.result — Result Helpers

```spectra
import std.result;
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `is_ok` | `(res: any) -> bool` | Is Ok? |
| `is_err` | `(res: any) -> bool` | Is Err? |
| `result_unwrap` | `(res: any) -> any` | Unwrap Ok (panics on Err) |
| `result_unwrap_or` | `(res: any, default: any) -> any` | Unwrap Ok or default |
| `result_unwrap_err` | `(res: any) -> any` | Unwrap Err (panics on Ok) |

```spectra
import std.result;

let r: Result<int, string> = Result::Ok(100);
let ok = is_ok(r);                           // true
let val = result_unwrap(r);                   // 100
```

### std.char — Character Operations

```spectra
import std.char;
```

All functions take a Unicode code point as `int`:

| Function | Signature | Description |
|----------|-----------|-------------|
| `is_alpha` | `(c: int) -> bool` | Is letter? |
| `is_digit_char` | `(c: int) -> bool` | Is digit (0-9)? |
| `is_whitespace_char` | `(c: int) -> bool` | Is whitespace? |
| `is_upper_char` | `(c: int) -> bool` | Is uppercase? |
| `is_lower_char` | `(c: int) -> bool` | Is lowercase? |
| `is_alphanumeric` | `(c: int) -> bool` | Is letter or digit? |
| `to_upper_char` | `(c: int) -> int` | Uppercase code point |
| `to_lower_char` | `(c: int) -> int` | Lowercase code point |

```spectra
import std.char;
import std.string;

let code = char_at("Hello", 0);   // 72 (= 'H')
let is_upper = is_upper_char(code);  // true
let lower_code = to_lower_char(code); // 104 (= 'h')
```

### std.time — Time Functions

```spectra
import std.time;
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `time_now_millis` | `() -> int` | Milliseconds since Unix epoch (-1 on error) |
| `time_now_secs` | `() -> int` | Seconds since Unix epoch (-1 on error) |
| `sleep_ms` | `(ms: int) -> unit` | Sleep for ms milliseconds |

```spectra
import std.time;

let start = time_now_millis();
sleep_ms(100);
let elapsed = time_now_millis() - start;
```

---

## 24. Multi-Module Projects

### Project Structure

A SpectraLang project is a directory containing `.spectra` files. The CLI discovers all `.spectra` files in the directory (and subdirectories), resolves dependencies based on `import` statements, and compiles them in topological order.

```
my_project/
├── spectra.toml     (optional project manifest)
├── main.spectra     (module main_project; — has pub fn main())
├── utils.spectra    (module utils;)
├── models.spectra   (module models;)
└── helpers.spectra  (module helpers;)
```

### spectra.toml (Optional Project Manifest)

```toml
[project]
name = "my_project"
version = "0.1.0"
entry = "main.spectra"    # optional, explicit entry point

# Optional source directories (default: src/)
src_dirs = ["src", "lib"]
```

### Module Dependency Rules

1. Each `.spectra` file declares exactly one module with `module name;`.
2. Module names must be **unique** across the project.
3. Modules that depend on others list them with `import`.
4. Circular dependencies are not allowed.
5. Standard library modules (`std.*`) do not require corresponding `.spectra` files.

### Running a Multi-Module Project

```bash
# Run all .spectra files in a directory
spectralang run my_project/

# Run specific files (for complex_demo with mixed modules)
spectralang run main.spectra module_a.spectra module_b.spectra
```

### Cross-Module Function Calls

After `import module_name;`, all `pub` functions from that module are available unqualified **or** qualified:

```spectra
// sa_report.spectra
import sa_grades;

pub fn score_to_gpa(score: int) -> int {
    let g = score_to_grade(score);   // unqualified call to sa_grades::score_to_grade
    return grade_points(g);          // unqualified call to sa_grades::grade_points
}
```

### Cross-Module Qualified Calls

Use `ModuleName::function_name()` when calling functions from a specific module:

```spectra
import cx_geometry;

pub fn area_of_square(side: int) -> int {
    let sq = cx_geometry::Shape::Rect(side, side);   // qualified enum constructor
    return cx_geometry::shape_area(sq);               // qualified function call
}
```

### Cross-Module Struct/Enum Usage

```spectra
// cx_ledger.spectra
pub struct Account {
    id: int,
    balance: int,
}

pub enum TxKind {
    Credit(int),
    Debit(int),
}

// In cx_main.spectra
import cx_ledger;

let acc = cx_ledger::make_account(1, 500);      // factory function (recommended)
let credit = cx_ledger::TxKind::Credit(100);    // qualified enum constructor
```

**Important:** Direct struct construction across modules is only possible if the struct and its fields are both `pub`. The recommended pattern is to provide a factory function (`pub fn new(...)`) in the owning module.

---

## 25. CLI Reference

### Commands

| Command | Description |
|---------|-------------|
| `spectralang run <files/dir>` | Compile and execute via JIT |
| `spectralang compile <files/dir>` | Compile without executing |
| `spectralang check <files/dir>` | Type-check only (no code generation) |
| `spectralang lint <files/dir>` | Run lint checks |
| `spectralang fmt <files>` | Format source files |
| `spectralang repl` | Start interactive REPL |
| `spectralang new <name>` | Scaffold new project |
| `spectralang help` | Show help |

### Common Flags

| Flag | Description |
|------|-------------|
| `--run` / `-r` | Execute after compilation (JIT) |
| `--emit-object <path>` | Generate native object file (AOT) |
| `--emit-exe <path>` | Generate native executable (AOT) |
| `--no-optimize` / `-O0` | No optimizations |
| `-O1` | Constant folding |
| `-O2` | Constant folding + dead code elimination (default) |
| `-O3` | All optimizations |
| `--dump-ast` | Print AST to stderr (debug) |
| `--dump-ir` | Print IR to stderr (debug) |
| `--verbose` / `-v` | Verbose build output |
| `--summary` | Per-module pipeline summary |
| `--json` | JSON diagnostic output (lint) |
| `--enable-experimental <feature>` | Enable experimental feature |

### Available Experimental Features

| Feature | Description |
|---------|-------------|
| `switch` | `switch/case` statement |
| `unless` | `unless` conditional |
| `do-while` | `do { } while` loop |
| `loop` | Infinite `loop { }` |

> Note: In practice these features are already widely supported without the flag. The flag is only required when strict experimental-feature gating is enabled.

### Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `64` | Usage error (bad arguments) |
| `65` | Compilation error |
| `74` | I/O error |

### Lint Rules

| Rule | Description |
|------|-------------|
| `unused-binding` | Variable declared but never used |
| `unreachable-code` | Code after `return` is unreachable |
| `shadowing` | Variable shadows a variable in a parent scope |

---

## 26. Complete Working Examples

### Example 1: Basic Math Module + Main

**math.spectra**
```spectra
module math;

pub fn gcd(a: int, b: int) -> int {
    if b == 0 { return a; }
    return gcd(b, a % b);
}

pub fn factorial(n: int) -> int {
    if n <= 1 { return 1; }
    return n * factorial(n - 1);
}

pub fn power(base: int, exp: int) -> int {
    let result = 1;
    let i = 0;
    while i < exp {
        result = result * base;
        i = i + 1;
    }
    return result;
}

pub fn is_prime(n: int) -> bool {
    if n < 2 { return false; }
    if n == 2 { return true; }
    if n % 2 == 0 { return false; }
    let i = 3;
    while i * i <= n {
        if n % i == 0 { return false; }
        i = i + 2;
    }
    return true;
}
```

**main.spectra**
```spectra
module main;

import std.io;
import math;

pub fn main() -> int {
    println(gcd(48, 18));        // 6
    println(factorial(6));       // 720
    println(power(2, 10));       // 1024
    println(is_prime(17));       // true
    return 0;
}
```

Run:
```bash
spectralang run main.spectra math.spectra
```

---

### Example 2: Structs, Enums, and Traits

```spectra
module shapes;

import std.io;

trait Area {
    fn area(&self) -> int;
}

trait Perimeter {
    fn perimeter(&self) -> int;
}

struct Rectangle {
    width: int,
    height: int,
}

struct Circle {
    radius: int,
}

impl Rectangle {
    pub fn new(w: int, h: int) -> Rectangle {
        Rectangle { width: w, height: h }
    }
}

impl Area for Rectangle {
    fn area(&self) -> int {
        self.width * self.height
    }
}

impl Perimeter for Rectangle {
    fn perimeter(&self) -> int {
        2 * (self.width + self.height)
    }
}

impl Area for Circle {
    fn area(&self) -> int {
        self.radius * self.radius * 3
    }
}

impl Perimeter for Circle {
    fn perimeter(&self) -> int {
        self.radius * 6
    }
}

pub fn main() -> int {
    let r = Rectangle::new(5, 3);
    let c = Circle { radius: 4 };

    println(r.area());       // 15
    println(r.perimeter());  // 16
    println(c.area());       // 48
    println(c.perimeter());  // 24

    return 0;
}
```

---

### Example 3: Enums with Match and Option

```spectra
module grade_system;

import std.io;

pub enum Grade {
    A,
    B,
    C,
    D,
    F,
}

pub fn score_to_grade(score: int) -> Grade {
    if score >= 90 { return Grade::A; }
    if score >= 80 { return Grade::B; }
    if score >= 70 { return Grade::C; }
    if score >= 60 { return Grade::D; }
    return Grade::F;
}

pub fn grade_to_points(g: Grade) -> int {
    match g {
        Grade::A => 4,
        Grade::B => 3,
        Grade::C => 2,
        Grade::D => 1,
        Grade::F => 0,
    }
}

fn safe_divide(a: int, b: int) -> Option<int> {
    if b == 0 { return Option::None; }
    return Option::Some(a / b);
}

pub fn main() -> int {
    let g1 = score_to_grade(95);
    let g2 = score_to_grade(73);
    let g3 = score_to_grade(55);

    println(grade_to_points(g1));  // 4
    println(grade_to_points(g2));  // 2
    println(grade_to_points(g3));  // 0

    let result = safe_divide(10, 2);
    let val = match result {
        Option::Some(v) => v,
        Option::None    => 0,
    };
    println(val);  // 5

    return 0;
}
```

---

### Example 4: Multi-Module Project with Traits

**scorable.spectra**
```spectra
module scorable;

pub trait Scorable {
    fn total(&self) -> int;
    fn average(&self) -> int;
    fn best(&self) -> int;
}
```

**student.spectra**
```spectra
module student;

import scorable;

pub struct Student {
    pub name: string,
    math: int,
    science: int,
    english: int,
}

impl Student {
    pub fn new(name: string, math: int, sci: int, eng: int) -> Student {
        Student { name: name, math: math, science: sci, english: eng }
    }
}

impl Scorable for Student {
    fn total(&self) -> int {
        self.math + self.science + self.english
    }

    fn average(&self) -> int {
        (self.math + self.science + self.english) / 3
    }

    fn best(&self) -> int {
        let m = self.math;
        let s = self.science;
        let e = self.english;
        if m >= s {
            if m >= e { return m; }
            return e;
        }
        if s >= e { return s; }
        return e;
    }
}
```

**main.spectra**
```spectra
module main;

import std.io;
import student;

pub fn main() -> int {
    let alice = Student::new("Alice", 95, 88, 92);
    let bob = Student::new("Bob", 72, 68, 75);

    println(alice.total());    // 275
    println(alice.average());  // 91
    println(alice.best());     // 95

    println(bob.average());    // 71

    return 0;
}
```

---

### Example 5: Using Standard Library

```spectra
module stdlib_demo;

import std.io;
import std.math;
import std.string;
import std.convert;
import std.collections;
import std.random;

pub fn main() -> int {
    // std.math
    let m = max(10, 20);               // 20
    let a = abs(-42);                  // 42
    let r = sqrt_f(25.0);              // 5.0
    println(m);
    println(a);
    println(r);

    // std.string
    let s = "Hello, World!";
    println(len(s));                    // 13
    println(to_upper(s));               // "HELLO, WORLD!"
    println(contains(s, "World"));     // true
    println(substring(s, 0, 5));       // "Hello"

    // std.convert
    let n_str = int_to_string(42);
    println(n_str);                    // "42"
    let parsed = string_to_int("123");
    println(parsed);                   // 123

    // std.collections
    let lst = list_new();
    list_push(lst, 10);
    list_push(lst, 30);
    list_push(lst, 20);
    list_sort(lst);
    println(list_get(lst, 0));         // 10
    println(list_get(lst, 1));         // 20
    println(list_get(lst, 2));         // 30
    list_free(lst);

    // std.random
    random_seed(42);
    let rn = random_int(1, 10);
    println(rn);                       // deterministic with seed

    return 0;
}
```

---

### Example 6: f-strings and String Operations

```spectra
module greeting;

import std.io;
import std.string;
import std.convert;

fn make_greeting(name: string, score: int) -> string {
    let grade = if score >= 90 { "A" } elif score >= 80 { "B" } else { "C" };
    return f"Hello, {name}! Your grade is {grade} (score: {score}).";
}

pub fn main() -> int {
    let greeting = make_greeting("Alice", 95);
    println(greeting);
    // Hello, Alice! Your grade is A (score: 95).

    let name = "Bob";
    let age = 25;
    println(f"{name} is {age} years old.");
    // Bob is 25 years old.

    let n = 42;
    let doubled = n * 2;
    println(f"{n} doubled is {doubled}.");
    // 42 doubled is 84.

    return 0;
}
```

---

### Example 7: Algorithms with Arrays

```spectra
module algorithms;

import std.io;

pub fn bubble_sort(arr: [int], n: int) {
    let i = 0;
    while i < n - 1 {
        let j = 0;
        while j < n - 1 - i {
            if arr[j] > arr[j + 1] {
                let tmp = arr[j];
                arr[j] = arr[j + 1];
                arr[j + 1] = tmp;
            }
            j = j + 1;
        }
        i = i + 1;
    }
}

pub fn binary_search(arr: [int], n: int, target: int) -> int {
    let lo = 0;
    let hi = n - 1;
    while lo <= hi {
        let mid = (lo + hi) / 2;
        if arr[mid] == target { return mid; }
        if arr[mid] < target { lo = mid + 1; }
        else { hi = mid - 1; }
    }
    return -1;
}

pub fn main() -> int {
    let arr = [64, 34, 25, 12, 22];
    bubble_sort(arr, 5);
    // arr is now [12, 22, 25, 34, 64]

    let i = 0;
    while i < 5 {
        println(arr[i]);
        i = i + 1;
    }

    let sorted = [10, 20, 30, 40, 50];
    let idx = binary_search(sorted, 5, 30);
    println(idx);  // 2

    return 0;
}
```

---

## Appendix A: Reserved Keywords

| Keyword | Status | Purpose |
|---------|--------|---------|
| `module` | ✅ Implemented | Declare module |
| `import` | ✅ Implemented | Import module |
| `pub` | ✅ Implemented | Public visibility |
| `internal` | ✅ Implemented | Package-internal visibility |
| `fn` | ✅ Implemented | Declare function |
| `struct` | ✅ Implemented | Declare struct |
| `enum` | ✅ Implemented | Declare enum |
| `impl` | ✅ Implemented | Implementation block |
| `trait` | ✅ Implemented | Declare trait |
| `let` | ✅ Implemented | Variable declaration |
| `mut` | ✅ Accepted (optional) | Mutability hint |
| `Self` | ✅ Implemented | Implementing type in trait/impl |
| `if` | ✅ Implemented | Conditional |
| `elif` | ✅ Implemented | Else-if |
| `elseif` | ✅ Implemented | Alias for elif |
| `else` | ✅ Implemented | Else branch |
| `unless` | ✅ Implemented | Negated conditional |
| `while` | ✅ Implemented | While loop |
| `do` | ✅ Implemented | Do-while loop |
| `for` | ✅ Implemented | For loop |
| `in` | ✅ Implemented | For x in iterable |
| `of` | ✅ Implemented | Alias for in (for x of iterable) |
| `loop` | ✅ Implemented | Infinite loop |
| `match` | ✅ Implemented | Pattern matching |
| `switch` | ✅ Implemented | Value comparison |
| `case` | ✅ Implemented | Switch/match arm |
| `return` | ✅ Implemented | Return from function |
| `break` | ✅ Implemented | Exit loop |
| `continue` | ✅ Implemented | Next loop iteration |
| `true` | ✅ Implemented | Boolean literal |
| `false` | ✅ Implemented | Boolean literal |
| `const` | ✅ Implemented | Compile-time constant |
| `static` | ✅ Implemented | Module-level mutable |
| `type` | ✅ Implemented | Type alias |
| `as` | ✅ Implemented | Type cast |
| `dyn` | ✅ Accepted | Dynamic dispatch |
| `export` | 🚧 Reserved | Future use |
| `class` | 🚧 Reserved | Future use |
| `foreach` | 🚧 Reserved | Future use |
| `repeat` | 🚧 Reserved | Future use |
| `until` | 🚧 Reserved | Future use |
| `cond` | 🚧 Reserved | Future use |
| `yield` | 🚧 Reserved | Future use |
| `goto` | 🚧 Reserved | Future use |

---

## Appendix B: Common Errors and Solutions

| Error | Cause | Solution |
|-------|-------|----------|
| `module declaration missing` | No `module name;` at top of file | Add `module name;` as the first line |
| `main not found` | No `pub fn main() -> int` | Add entry point function |
| `type mismatch: int and float` | Mixing int/float without conversion | Use `int_to_float(x)` or `float_to_int(x)` |
| `cannot assign to immutable` | Rare compiler edge case | Variables are mutable by default; check the context |
| `non-exhaustive match` | Not all enum variants covered | Add `_ =>` wildcard arm |
| `undefined variable 'x'` | Using variable before `let` or out of scope | Move `let x = ...` to the correct scope |
| `undefined function 'f'` | Calling a function not imported or defined | Import the module or define the function |
| `break/continue outside loop` | Used outside `while`/`for`/`loop` | Move inside a loop body |
| `field not found` | Accessing a struct field that doesn't exist | Check field name spelling |
| `missing field in struct literal` | Not all struct fields provided | Provide all required fields |
| `cyclic dependency` | Module A imports B, B imports A | Restructure to break the cycle |
| `duplicate module name` | Two files declare the same module | Ensure each module name is unique |
| `unresolved import 'mod'` | Importing a module with no matching file | Create a file with `module mod;` |

---

## Appendix C: Naming Conventions

| Construct | Convention | Example |
|-----------|-----------|---------|
| Variables | `snake_case` | `my_var`, `total_count` |
| Functions | `snake_case` | `calculate_area`, `get_name` |
| Parameters | `snake_case` | `fn f(total_score: int)` |
| Structs | `PascalCase` | `Point`, `UserAccount` |
| Enums | `PascalCase` | `Color`, `Status` |
| Enum Variants | `PascalCase` | `Color::Red`, `Status::Active` |
| Traits | `PascalCase` | `Printable`, `Comparable` |
| Type Params | Short `PascalCase` | `T`, `E`, `Key`, `Val` |
| Modules | `snake_case` | `module my_lib;` |
| Files | `snake_case.spectra` | `my_module.spectra` |
| Constants | `UPPER_SNAKE_CASE` | `const MAX_SIZE: int = 100;` |

---

*End of SpectraLang AI Agent Reference*
