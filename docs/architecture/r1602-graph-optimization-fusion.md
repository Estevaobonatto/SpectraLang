# R-1602 Graph Optimization and Fusion

Updated: 2026-06-06

Roadmap item: `R-1602 Graph Optimization and Fusion`

## Purpose

R-1602 adds a deterministic optimization layer over the Phase 16 Tensor Graph IR. The optimizer is graph-level only: it does not replace backend execution yet, but it produces a validated optimized graph that later phases can lower to CPU/GPU kernels, exports, or fusion runtimes.

## Public Midend Contract

- Optimization entry point: `TensorGraph::optimize()`.
- Comparison entry point: `TensorGraph::compare_optimized(&optimized)`.
- Optimization output: `TensorGraphOptimizationResult`.
- Report output: `TensorGraphOptimizationReport`.
- Numerical tolerance policy: `1e-9` absolute and `1e-9` relative, matching R-1503.

## Implemented Optimizations

- Elementwise chain fusion:
  - `relu -> sqrt_f` becomes one `fused_elementwise.relu+sqrt_f` graph node when the chain has a single consumer and observable output metadata is preserved.
- Reduction-adjacent fusion:
  - `relu -> tanh_f -> sum_t` becomes one `fused_reduction.relu+tanh_f->sum_t` graph node when the elementwise chain feeds the reduction through single-consumer edges.
- Memory-aware scheduling metadata:
  - The optimization report records `reusable_edges`, which identifies fused input edges that can be scheduled without materializing intermediate tensors.

## Correctness Contract

The current optimizer is semantics-preserving at graph level:

- it validates the input graph before optimizing;
- it preserves observable output metadata by value ID;
- it emits stable reports with node counts, fused groups, fused elementwise op count, fused reduction count, reusable edges, and tolerance policy;
- `TensorGraph::compare_optimized` checks optimized graph outputs against the original graph.

Full optimized runtime execution is intentionally deferred to later graph/backend phases. R-1602 provides the correctness-preserving graph transformation foundation required by those phases.

## Test Gate

Run:

```powershell
cargo test -p spectra-midend --test tensor_graph_tests
```

The gate includes:

- elementwise chain fusion;
- reduction-adjacent fusion;
- optimized vs unoptimized graph comparison;
- stable optimized graph snapshot;
- existing R-1601 graph validation regressions.

## Spectra Examples

- `examples/ai/tensor_graph_elementwise_fusion.spectra` demonstrates a `relu -> sqrt_f -> tanh_f` elementwise chain.
- `examples/ai/tensor_graph_reduction_fusion.spectra` demonstrates a `relu -> tanh_f -> sum_t` reduction-adjacent pattern.

Both examples are executable through the Phase 13 AI examples block in `run_tests.ps1`.
