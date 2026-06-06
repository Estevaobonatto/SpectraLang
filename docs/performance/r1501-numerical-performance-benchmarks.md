# R-1501 Numerical Performance Benchmark Suite

Updated: 2026-06-06

Roadmap item: `R-1501 Numerical Performance Benchmark Suite`

## Purpose

This benchmark suite is the production gate for Phase 15 numerical performance visibility. It runs runtime-level host calls in `--release` mode, emits machine-readable JSON, and compares the observed results against checked-in thresholds.

## Covered Paths

- tensor creation: `std.tensor.full_f`
- unary ops: `std.tensor.relu`
- reductions: `std.tensor.sum_f`
- matmul: `std.tensor.matmul`
- convolution: `std.ml.conv2d`
- autodiff: `requires_grad`, `mul`, `sum_t`, `backward`, `grad`
- optimizer steps: `std.ml.linear`, `mse_loss`, `backward`, `sgd_step`
- data loading: `dataset_from_tensors`, `dataloader_new`, batch feature/label extraction

## Commands

Run only the raw benchmark:

```powershell
cargo run --release -p spectra-runtime --example numerical_performance_bench
```

Run the CI-style gate:

```powershell
python scripts/validate_r1501_bench.py
```

The validator writes the observed JSON report to `target/r1501-benchmark-report.json`.

## Baseline Policy

The checked-in baseline lives at `docs/performance/r1501-benchmark-baseline.json`.

Thresholds are intentionally generous for shared CI and developer machines, but the gate still fails when:

- the benchmark is accidentally run in debug mode
- any required benchmark category disappears
- any correctness check fails
- runtime cost exceeds the configured `max_ns_per_iter`

Changing thresholds requires updating the baseline file and explaining the reason in the roadmap/backlog entry.
