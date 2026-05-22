# ADR 0002: Autodiff Runtime Contract

Status: Accepted

Date: 2026-05-22

## Context

SpectraLang needs gradient-based training support for AI and machine learning workloads. The current tensor system is a runtime-handle API, so autodiff must integrate with `std.tensor` handles before the future static `Tensor<T, Shape>` syntax exists.

## Decision

The accepted Phase 5 contract is eager reverse-mode autodiff inside the tensor runtime:

- Public API lives in `std.tensor`.
- Gradients are supported for float tensors.
- `requires_grad(handle, true)` marks a float tensor as a differentiable leaf.
- Operations create graph nodes only while grad mode is enabled.
- `backward(loss)` accepts a scalar tensor loss and accumulates gradients into reachable float tensors.
- `grad(handle)` returns a float tensor containing the accumulated gradient.
- `zero_grad(handle)` clears accumulated gradient for training-loop reuse.
- Graph nodes are released after `backward` by default.
- `set_grad_enabled(false)` disables graph construction for inference/no-grad sections.

## Supported Gradient Rules

Phase 5 supports gradient rules for:

- elementwise `add`, `sub`, `mul`, `div`
- unary `neg`, `relu`, `exp_f`, `log_f`, `sqrt_f`, `sigmoid_f`, `tanh_f`
- tensor reductions `sum_t`, `mean_t`
- matrix multiplication `matmul`
- `transpose`
- scalar tensor dot product `dot_t`
- reshape/flatten views as identity-gradient graph edges

Existing scalar-returning APIs such as `sum`, `sum_f`, `mean_f`, and `dot` remain non-graph scalar host calls. Differentiable reductions use tensor-returning forms ending in `_t`.

## Memory and Inference Policy

The runtime releases graph nodes after backward to avoid graph retention across training iterations. Inference mode avoids autograd overhead by preventing creator nodes from being attached to operation outputs.

## Consequences

- Autodiff is usable from current Spectra programs through `std.tensor` without new syntax.
- Static tensor typing and compiler-native autodiff remain future work.
- Broadcast-aware gradient reduction is deferred until broadcasted tensor operations exist in the production tensor API.
