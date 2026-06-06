# R-1503 Numerical Correctness and Determinism Certification

Updated: 2026-06-06

Roadmap item: `R-1503 Numerical Correctness and Determinism Certification`

## Purpose

R-1503 defines the production correctness gate for SpectraLang numerical runtime behavior. The gate is a portable JSON artifact that can be generated on Windows, Linux, or macOS and compared against the checked-in baseline.

## Public Runtime Contract

- `std.tensor.set_deterministic_mode(1)` enables deterministic tensor mode and resets the tensor RNG to a stable seed; it returns `0` on success.
- `std.tensor.deterministic_mode()` returns the current mode as `0` or `1`.
- `std.tensor.tolerance_abs()` returns the absolute tolerance policy.
- `std.tensor.tolerance_rel()` returns the relative tolerance policy.
- The current tolerance policy is `1e-9` absolute and `1e-9` relative.

## Covered Checks

- RNG: seeded `std.tensor.uniform` self-consistency.
- Reductions: `std.tensor.sum_f`.
- Matmul: deterministic 2x2 matrix multiplication.
- Convolution: deterministic `std.ml.conv2d`.
- Optimizer: deterministic `std.ml.linear` + `mse_loss` + `backward` + `sgd_step`.

## Commands

Run only the raw certifier:

```powershell
cargo run --release -p spectra-runtime --example numerical_correctness_cert
```

Run the CI-style gate:

```powershell
python scripts/validate_r1503_correctness.py
```

The validator writes the observed portable artifact to `target/r1503-correctness-report.json`.
