# 7. Standard Library, Runtime, And Packages

This chapter lists the production APIs used by the AI book path. The complete
language surface is documented in the reference files under `docs/`; this page
is the adoption-oriented subset needed to run the checked-in AI examples.

## Standard Library Modules Used By AI Examples

| Module | Purpose |
|--------|---------|
| `std.tensor` | tensor handles, kernels, RNG, autodiff, lifecycle |
| `std.ml` | layers, losses, optimizers, datasets, dataloaders |
| `std.fs` | deterministic export files with safe nested artifact writes |
| `std.concurrent` | local pipeline primitives |
| `std.serve` | local in-process serving |

Use aliases:

```spectra
import std.tensor as tensor;
import std.ml as ml;
import std.fs as fs;
```

`std.fs.fs_write` and `std.fs.fs_append` create missing parent directories when
possible and return `false` for controlled filesystem failures. AI examples can
write nested artifact paths such as `target/ai-examples/run/report.txt` without
precreating the directory tree.

## Runtime Lifecycle

Long-running examples should reset tensor state:

```spectra
tensor.free_all();
// work
tensor.free_all();
```

For host-call heavy examples, use the CLI runner instead of invoking internals
directly:

```powershell
.\target\debug\spectralang.exe run examples\ai\data_preprocessing_pipeline.spectra
```

## FFI And Python Interop

The Phase 8 production baseline provides:

- `python/spectra_bridge.py` for Python-to-Spectra CLI/JIT integration.
- `tools/spectra-interop/include/spectra_interop.h` for the C ABI.
- `tools/spectra-interop/examples/rust_ffi_sample.rs` for Rust integration.
- `.npy` one-dimensional f64 exchange through the Python bridge.

Validate interop through the repository test runner:

```powershell
.\run_tests.ps1
```

## Package Manager

The Phase 9 production baseline provides:

```powershell
.\target\debug\spectralang.exe package lock
.\target\debug\spectralang.exe package build
.\target\debug\spectralang.exe package check
.\target\debug\spectralang.exe package test
.\target\debug\spectralang.exe package bench
.\target\debug\spectralang.exe package doc
.\target\debug\spectralang.exe package search math
.\target\debug\spectralang.exe package add gitmath
```

Use exact versions, catalog-backed Git packages, and checked-in `spectra.lock`
for reproducible AI projects. Central hosted registry authentication and
provenance signing remain future hardening work.

## Full Validation

The book, AI examples, package workflow, interop checks, security checks, and
language regression tests are wired through:

```powershell
.\run_tests.ps1
```
