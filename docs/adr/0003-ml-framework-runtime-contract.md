# ADR 0003: ML Framework Runtime Contract

Status: Accepted

Date: 2026-05-26

## Context

SpectraLang now has tensor handles and reverse-mode autodiff in `std.tensor`. Phase 6 needs a usable high-level ML layer without waiting for future static tensor syntax or compiler-native graph IR.

## Decision

The accepted Phase 6 contract is a runtime-backed `std.ml` module layered on top of `std.tensor`:

- Module handles provide parameter registration and training/eval mode.
- Layers are host calls over tensor handles.
- Losses return scalar tensor losses compatible with `tensor.backward`.
- Optimizers update tensor parameters in place from accumulated gradients and clear gradients after each step.
- Dataset and dataloader handles provide reproducible minibatch selection from tensor-backed datasets.

## Supported Baseline

Layers:

- `linear(input, weight, bias)`
- `conv2d(input, kernel, bias, batch, in_ch, height, width, out_ch, kernel_h, kernel_w)`
- `dropout(input, p, training)`
- `max_pool2d(input, batch, channels, height, width, pool_h, pool_w)`

Losses:

- `mse_loss`
- `bce_loss`
- `cross_entropy_loss`
- `nll_loss`

Optimizers:

- `sgd_step`
- `sgd_momentum_step`
- `adam_step`
- `adamw_step`
- `exp_lr`

Data:

- `dataset_from_tensors`
- `dataset_len`
- `dataloader_new`
- `dataloader_batch_count`
- `dataloader_batch_features`
- `dataloader_batch_labels`

## Acceptance Evidence

- Runtime tests train a small MLP with `linear` + MSE + SGD to convergence.
- Runtime tests train a small convolutional model with `conv2d` + MSE + AdamW/SGD to convergence.
- Runtime tests cover module parameter registration, dataloader batching, BCE, LR scheduling, dropout, and max pooling.
- Spectra examples `72_ml_phase6_mlp_training.spectra` and `73_ml_phase6_cnn_training.spectra` compile and run through the public API.

## Consequences

- The current ML framework is production-usable for CPU tensor-handle programs.
- Rich model classes, serialization formats, parallel data prefetch, image-folder readers, and compiler-native ML syntax remain future work.
- Numerical convergence gates live in Rust runtime tests; Spectra validation examples verify public API integration and execution.
