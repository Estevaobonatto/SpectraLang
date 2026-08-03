# 5. Model Authoring

Spectra model authoring uses `std.tensor` for data and parameters and `std.ml`
for layers, losses, optimizers, datasets, and dataloaders. The current
production path is explicit and handle-based: tensors are runtime handles, model
modules are runtime handles, and examples validate behavior through the CLI.

## Linear Model

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

Run the complete checked-in version:

```powershell
.\target\debug\spectralang.exe run examples\ai\linear_regression_train_export.spectra
```

That example trains a toy linear regression model and writes
`target/ai-examples/linear_regression_model.txt`.

## Classification

Use binary cross entropy for logistic-style examples:

```spectra
let probs = tensor.requires_grad(tensor.full_f(4, 0.5), true)
let target = tensor.full_f(4, 1.0)
let loss = ml.bce_loss(probs, target)
tensor.backward(loss)
ml.sgd_step(probs, 0.1)
```

Validated example:

```powershell
.\target\debug\spectralang.exe run examples\ai\logistic_regression_train_export.spectra
```

## Modules And Layers

`std.ml` module handles allow examples to express a model boundary while keeping
the current compiler/runtime contract explicit.

```spectra
let model_handle = ml.module_create()
let dense = ml.linear_layer_create(4, 2)
ml.module_add_layer(model_handle, dense)
```

Validated example:

```powershell
.\target\debug\spectralang.exe run examples\ai\mlp_training_serving.spectra
```

## Datasets And Dataloaders

Use tensor-backed datasets for reproducible AI examples:

```spectra
let features = tensor.reshape(tensor.full_f(4, 1.0), 4, 1)
let labels = tensor.full_f(4, 2.0)
let dataset = ml.dataset_from_tensors(features, labels)
let loader = ml.dataloader_create(dataset, 2, true, 7)
let batch = ml.dataloader_next(loader)
```

The `true, 7` arguments request deterministic shuffling with seed `7`.

## Production Rules For Examples

- Call `tensor.free_all()` before and after long-running examples.
- Set deterministic seeds or use deterministic constants.
- Write export artifacts under `target/ai-examples/`.
- Keep examples executable with `spectralang run`, not pseudo-code.
