# Spectra Interop

This document describes the Phase 8 interoperability surface currently implemented for SpectraLang.

## Status

- Python can invoke Spectra programs through the CLI/JIT boundary.
- Python tensor exchange supports NumPy `.npy` files for one-dimensional little-endian `f64` arrays.
- Rust code can use the `spectra-interop` crate for safe `.npy` round-trips.
- A stable C ABI header and C sample are provided. Local compilation of the C sample requires an installed C compiler such as MSVC `cl`, `clang`, or `gcc`.

The current implementation is intentionally minimal and production-oriented: it provides a stable data interchange baseline without requiring Python extensions, native BLAS, or platform-specific compiler setup for the default build.

## Python Bridge

File:

- `python/spectra_bridge.py`

Run the Phase 8 demo:

```powershell
python python\demo_phase8.py
```

The demo:

- runs a Spectra validation program through `spectra-cli`
- writes a NumPy `.npy` file
- reads the file back with NumPy
- validates numeric contents

Python dependencies:

- `numpy`
- `cargo` available in `PATH`

## Rust Interop

Crate:

- `tools/spectra-interop`

Run tests:

```powershell
cargo test -p spectra-interop
```

Run the Rust FFI sample:

```powershell
cargo run -p spectra-interop --example rust_ffi_sample
```

The Rust API currently exposes:

- `TensorF64::new`
- `TensorF64::as_slice`
- `TensorF64::sum`
- `TensorF64::write_npy`
- `TensorF64::read_npy`

## C ABI

Header:

- `tools/spectra-interop/include/spectra_interop.h`

C sample:

- `tools/spectra-interop/examples/c_ffi_sample.c`

The ABI currently exposes:

- `spectra_interop_abi_version`
- `spectra_interop_add_i64`
- `spectra_interop_tensor_f64_sum`
- `spectra_interop_npy_write_f64`
- `spectra_interop_npy_read_f64`
- `spectra_interop_f64_array_free`

ABI rules:

- `SpectraF64Array` memory returned by Spectra must be released with `spectra_interop_f64_array_free`.
- Paths are passed as UTF-8 byte pointers plus explicit lengths.
- Invalid arguments return `SPECTRA_INTEROP_INVALID_ARGUMENT` or a null `SpectraF64Array`.
- `.npy` support is limited to little-endian `f64`, one-dimensional, C-order arrays.

## Data Format Contract

The supported interchange format is NumPy `.npy` v1.0:

- dtype: little-endian `f64` (`<f8`)
- shape: one-dimensional
- order: C-order only
- pickle: not used

This is the Phase 8 baseline for deterministic tensor exchange. Broader format support such as `.npz`, safetensors, checkpoints, and ONNX remains future work unless tracked by a later roadmap item.

## Validation

Recommended validation commands:

```powershell
cargo test -p spectra-interop
cargo run -p spectra-interop --example rust_ffi_sample
python python\demo_phase8.py
.\run_tests.ps1
```

If no C compiler is available, `run_tests.ps1` reports the C sample as skipped by environment. That skip must not be treated as completion evidence for roadmap items that require the C sample to compile and run.
