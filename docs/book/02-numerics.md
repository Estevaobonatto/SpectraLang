# 2. Numerics

Spectra currently exposes production numeric work primarily through scalar
`int`, scalar `float`, and `std.tensor` handles.

## Scalar Numerics

```spectra
let a = 40
let b = 2
let c = a + b
```

Use `std.math` for scalar helpers:

```spectra
import std.math as math

public func main() returns int {
    if math.abs(-42) != 42 {
        return 1
    }
    return 0
}
```

## Conversion

Use `std.convert` when crossing scalar type boundaries:

```spectra
import std.convert as convert

let value = convert.int_to_float(42)
```

## Randomness

Tensor random APIs are the recommended path for AI examples because they are
seeded and tested:

```spectra
import std.tensor as tensor

tensor.seed(2026)
let batch = tensor.uniform(8, 0, 10)
```

## Reproducibility Rule

AI examples should set seeds or use deterministic constants. Production
examples in this repo avoid ambient randomness unless the goal is specifically
to test RNG.
