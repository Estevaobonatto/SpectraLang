# 1. Language Basics

Spectra source files use the `.spectra` extension. Every executable example
starts with a module declaration and a public `main` function returning `int`.

```spectra
module hello

public func main() returns int {
    return 0
}
```

Run a file:

```powershell
.\target\debug\spectralang.exe run examples\ai\linear_regression_train_export.spectra
```

Check a file without execution:

```powershell
.\target\debug\spectralang.exe check examples\ai\linear_regression_train_export.spectra
```

Compile a file:

```powershell
.\target\debug\spectralang.exe compile examples\ai\linear_regression_train_export.spectra
```

## Functions

```spectra
func add(a: int, b: int) returns int {
    return a + b
}

public func main() returns int {
    return add(20, 22) - 42
}
```

## Variables

Variables are declared with `let`.

```spectra
let x = 10
let y: int = 32
```

## Control Flow

```spectra
if x > 10 {
    return 1
} else if x == 10 {
    return 0
} else {
    return 2
}
```

Loops:

```spectra
let i = 0
while i < 10 {
    i = i + 1
}
```

## Imports

Use aliases for standard library modules:

```spectra
import std.tensor as tensor
import std.ml as ml
import std.fs as fs
```

The AI examples use aliases consistently because they map cleanly to host calls
and make code easier to scan.

## Project Validation

Run the regression suite before changing examples or docs:

```powershell
.\run_tests.ps1
```
