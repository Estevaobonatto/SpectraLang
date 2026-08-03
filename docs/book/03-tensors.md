# 3. Tensors

The current production tensor model is a standard-library handle API. Tensor
values are `int` handles owned by the runtime.

```spectra
import std.tensor as tensor

public func main() returns int {
    tensor.free_all()

    let x = tensor.full_f(4, 1.0)
    if tensor.len(x) != 4 {
        return 1
    }

    tensor.free_all()
    return 0
}
```

## Common Constructors

```spectra
let zeros = tensor.zeros(8)
let ones = tensor.ones(8)
let floats = tensor.full_f(4, 1.0)
let range = tensor.arange(1, 5, 1)
let matrix = tensor.reshape(floats, 2, 2)
```

## Shape Queries

```spectra
tensor.len(matrix)
tensor.rank(matrix)
tensor.rows(matrix)
tensor.cols(matrix)
```

## Kernels

```spectra
let a = tensor.full_f(4, 1.0)
let b = tensor.full_f(4, 2.0)
let added = tensor.add(a, b)
let product = tensor.matmul(tensor.reshape(a, 2, 2), tensor.reshape(b, 2, 2))
let activated = tensor.relu(product)
```

## Lifecycle

Long-running examples should call `tensor.free_all()` at the beginning and end
of a program. This keeps examples deterministic and avoids contaminating
subsequent host-call tests with previous handles.

## Validation Example

See:

- `examples/ai/toy_transformer_inference.spectra`
- `tests/validation/68_tensor_phase4_kernels.spectra`
