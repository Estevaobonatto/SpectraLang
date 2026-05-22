# Tensor Phase 4 Benchmark

Updated: 2026-05-22

Roadmap items:

- `R-401` CPU Kernel Library
- `R-402` Tensor Allocator and Buffer Pool
- `R-403` RNG and Statistical Primitives

## Purpose

This file records the current reproducible benchmark harness for Phase 4 tensor work. The release benchmark treats kernel time within 10% of the naive scalar reference as same-speed to absorb normal timing noise.

## Command

```powershell
cargo run --release -p spectra-runtime --example tensor_phase4_bench
```

## Latest Local Result

```json
{
  "dot_len": 65536,
  "dot_iterations": 500,
  "dot_naive_ns": 18802300,
  "dot_kernel_ns": 16224100,
  "dot_host_ns": 393606300,
  "dot_pass": true,
  "mat_size": 32,
  "mat_iterations": 120,
  "matmul_naive_ns": 5158400,
  "matmul_kernel_ns": 2792600,
  "matmul_host_ns": 4285800,
  "matmul_pass": true,
  "pool_hits": 1,
  "pool_misses": 123,
  "scratch_reuses": 120,
  "allocation_pass": true,
  "passed": true
}
```

## Interpretation

- The release benchmark gate now passes for the checked-in dot and matmul kernel cases.
- The gate compares pure kernel time against naive scalar references with a 10% same-speed tolerance; host-call timing is reported separately as integration overhead.
- Allocation metrics are included in the gate: pool hits, pool misses, and scratch reuse must all be observed.
- Native BLAS/LAPACK is not required for the default Windows build. The `blas` Cargo feature is reserved as an opt-in integration point, but the production default remains the portable kernel path until a native BLAS provider is added and validated in CI.
- AVX-512 is explicitly rejected for the current production baseline because stable portable coverage is not available across target machines. AVX2/NEON remain dispatch strategy targets, but the current accepted production path is the portable kernel with release benchmark evidence.
