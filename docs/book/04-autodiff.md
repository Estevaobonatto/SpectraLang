# 4. Autodiff

Autodiff is exposed through `std.tensor`. The current production contract is
eager reverse-mode autodiff over float tensor handles.

## Minimal Training Step

```spectra
import std.tensor as tensor
import std.ml as ml

func train_step(x: int, target: int, weight: int, bias: int) returns int {
    let prediction = ml.linear(x, weight, bias)
    let loss = ml.mse_loss(prediction, target)
    tensor.backward(loss)
    ml.sgd_step(weight, 0.1)
    ml.sgd_step(bias, 0.1)
    return 0
}
```

## Make Parameters Trainable

```spectra
let weight0 = tensor.requires_grad(tensor.full_f(1, 0.0), true)
let weight = tensor.reshape(weight0, 1, 1)
let bias = tensor.requires_grad(tensor.full_f(1, 0.0), true)
```

## Inference Mode

Disable gradient tracking for inference-only examples:

```spectra
tensor.set_grad_enabled(false)
// inference work
tensor.set_grad_enabled(true)
```

## Gradient Lifecycle

Use `tensor.zero_grad(handle)` when reusing a parameter and you need to clear
accumulated gradient explicitly. The high-level examples use optimizer steps in
short loops and release graph state with `tensor.free_all()` at the end.

## Validation Example

See:

- `examples/ai/linear_regression_train_export.spectra`
- `examples/ai/logistic_regression_train_export.spectra`
- `tests/validation/71_tensor_phase5_autodiff.spectra`
