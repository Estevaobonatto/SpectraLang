# R-1502 Memory Planner and Tensor Lifetime Analysis

Updated: 2026-06-06

Roadmap item: `R-1502 Memory Planner and Tensor Lifetime Analysis`

## Purpose

R-1502 makes tensor memory behavior visible and testable. The current production implementation records runtime tensor lifetime metadata in the tensor registry and exposes a JSON memory report through `std.tensor.memory_report()`.

## Public Metrics

- `stats_lifetime_records()` returns the number of tensor lifetime records captured since the last `reset_stats()`.
- `stats_released_lifetimes()` returns the number of records with a release step.
- `stats_allocation_sites()` returns the number of unique allocation sites observed.
- `stats_reuse_rate_per_mille()` returns pool reuse as `pool_hits * 1000 / (pool_hits + pool_misses)`.
- `memory_report()` returns schema `spectra.tensor.memory_report.v1` JSON.

## Report Fields

The JSON report includes:

- allocation counters: `allocations`, `active_tensors`, `active_bytes`, `peak_bytes`
- reuse counters: `reused_buffers`, `pool_hits`, `pool_misses`, `reuse_rate_per_mille`, `scratch_reuses`
- visibility counters: `allocation_sites`, `lifetime_records`, `released_lifetimes`
- per-tensor records: `handle`, `dtype`, `shape`, `bytes`, `allocation_step`, `release_step`, `active`, `allocation_site`

## Validation

Language-level validation:

```powershell
target\debug\spectralang.exe run tests\validation\83_tensor_memory_planner.spectra
```

Runtime-level validation:

```powershell
cargo test -p spectra-runtime tensor_runtime_phase15_memory_report_tracks_lifetimes_sites_and_reuse
```

Integrated validation:

```powershell
.\run_tests.ps1
```

## Current Scope

The planner is runtime-backed rather than graph-IR-backed. This satisfies the R-1502 completion gate because tensor temporaries have visible lifetime metadata in runtime plans. Future graph compilation phases can consume or replace this report with IR-level lifetime plans once `R-1601 Tensor Graph IR` is implemented.
