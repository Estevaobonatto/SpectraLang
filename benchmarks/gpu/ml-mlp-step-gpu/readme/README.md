# ml-mlp-step-gpu

R-1603 / R-3080 / R-3052: production GPU training step benchmark.

## What it measures

A 2-layer MLP (128x128 hidden, 128 batch) running 10 forward+backward+SGD
iterations end-to-end. The Spectra bench uploads the input, weights, and
biases to Wgpu once via `tensor.to_device(_, 6)` and then runs the entire
training loop without further host copies. The Go reference is a
hand-rolled CPU implementation with the same shapes and learning rate.

## Files

- `spectra/bench.spectra` — production GPU training step (uses
  `ml.linear`, `tensor.relu`, `ml.mse_loss`, `tensor.backward`,
  `ml.sgd_step`, plus the new R-3052 residency and R-3080 backward
  kernels).
- `go/bench.go` — CPU reference, same shapes, no external deps.

## Building the CLI for GPU

The default `cargo build -p spectra-cli` does **not** include the GPU
backend. Use:

```
cargo build -p spectra-cli --features gpu
```

For meaningful speedup numbers, use a release build:

```
cargo build -p spectra-cli --features gpu --release
```

## Speedup gate

Run on a host with a WGPU adapter. The Python harness
`scripts/validate_r1603_gpu_speedup.py` measures both, reports
`target/r1603-gpu-speedup/report.json`, and fails if the GPU
timing is not faster than the CPU timing at the same batch size.

```
python scripts/validate_r1603_gpu_speedup.py
```

## Measured results (RTX 2060, debug + release)

| Build  | CPU (Spectra) | GPU (Spectra Wgpu) | Ratio | Go reference |
|--------|---------------|--------------------|-------|--------------|
| debug  | 2.15 s/iter   | 1.70 s/iter        | 1.27x | 0.20 s/iter  |
| release| 3.98 s/iter   | 3.46 s/iter        | 1.15x | 0.75 s/iter  |

The Go reference runs 5-10x faster than Spectra's CPU path because the
Go reference uses raw `float64` arithmetic without the autograd graph
or tensor bookkeeping. The GPU/CPU speedup is real (the GPU path
exercises the R-3080 backward kernels and R-3052 residency) but modest
because the production GPU hot path is still single-pass naive
matmul; tiled/SIMD kernels (R-3110, R-3111) and full residency
(R-3052 full hot path) are pending.

## Acceptance (R-1603)

- `target/r1603-gpu-speedup/report.json` records the CPU/GPU timings
  and the ratio at batch 128.
- ratio > 1.0x at batch 128 (production baseline). Met on RTX 2060.
- ratio > 1.5x at batch 128 (stretch). Requires R-3110/R-3111 (SIMD
  + tiled matmul) and the full R-3052 residency hot path.
- The bench itself is in CI default via
  `scripts/validate_r1603_gpu_backend.py` (correctness only, self-skip
  when no adapter is available).
