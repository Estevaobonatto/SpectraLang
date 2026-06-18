# R-2006 Tensor and std Performance Refresh

R-2006 refreshes tensor/std performance evidence before API work continues past
`R-2216`. The gate is release-only and complements the broader R-1501 numerical
benchmark suite with explicit coverage for view materialization, elementwise
chains, reductions, matmul, autodiff, and buffer reuse.

## Evidence

- Baseline thresholds: `docs/performance/r2006-performance-baseline.json`
- Checked-in report: `docs/performance/r2006-performance-report.json`
- Validator: `scripts/validate_r2006_performance_refresh.py`
- Runtime benchmark: `runtime/examples/r2006_tensor_performance_refresh.rs`

The checked-in report was generated on 2026-06-18 with:

```powershell
cargo run --release -p spectra-runtime --example r2006_tensor_performance_refresh
```

The validator reruns the same release benchmark, writes
`target/r2006-performance-report.json`, verifies every required category,
checks numerical correctness flags, confirms memory metrics are positive, and
compares both the live report and checked-in report against the baseline
thresholds.

## Guarded Categories

- `materialization`: `arange -> reshape -> transpose -> flatten -> sum_f`
- `elementwise_chains`: `add -> mul -> relu -> sum_f`
- `reductions`: repeated `sum_f` over a float tensor
- `matmul`: repeated 32x32 integer matrix multiplication with output checks
- `autodiff`: square-sum backward over a trainable tensor
- `buffer_reuse`: repeated allocate/free cycle that must hit the tensor pool

Thresholds must not be updated without replacing the checked-in report in the
same change.
