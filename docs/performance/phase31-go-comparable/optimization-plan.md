# Phase 31 Optimization Plan (R-3103)

Updated: 2026-07-13
Roadmap item: `R-3103 Optimization Implementation Plan`
Source data: `docs/performance/phase31-go-comparable/findings-r3101-initial.md`

## Prioritized List

Priority is `(impact × feasibility) / risk`. Each item lists estimated
per-scenario speedup range and the risk of regressing other workloads.

### Tier 1 — Close the largest single-scenario gap

| ID | Title | Target scenarios | Estimated speedup | Status |
|---|---|---|---|---|
| R-3108 | String Materialization Optimization | `cpu-string-build` (was 71.7x) | 5-20x | **complete** (3.85x measured) |
| R-3107 | Tensor Cross-Call Buffer Reuse | `tensor-create` (5.1x) | 2-5x | not_started |

### Tier 2 — Broad backend hot-path

| ID | Title | Estimated speedup | Risk | Notes |
|---|---|---|---|---|
| R-3104 | Cranelift Value Map Dense Indexing | 1.2-1.5x on all scenarios | low | `Vec<Option<Value>>` indexed by `ValueId`. Split JIT/AOT into separate files. |
| R-3105 | Host Call Batching and Name Precompute | 1.1-1.3x on tensor/ML | low | Pre-compute host name records at module load. |
| R-3106 | Alloca Hoisting | 1.05-1.1x where relevant | low | Lifting of loop-invariant allocas. |

### Tier 3 — Numerics

| ID | Title | Target scenarios | Estimated speedup | Risk |
|---|---|---|---|---|
| R-3110 | SIMD Elementwise Kernels | `tensor-elementwise` (2.7x vs Rust) | 2-4x | medium |
| R-3111 | Tiled Register-Blocked Matmul | `tensor-matmul` (2.5x vs Rust) | 2-4x | medium |
| R-3112 | Im2col + GEMM Conv2D | future conv workloads | 1.5-2x | medium |

### Tier 4 — Async

| ID | Title | Target scenarios | Estimated speedup | Risk |
|---|---|---|---|---|
| R-3113 | Work-Stealing Task Pool | `async-echo` vs Rust (2.4x) | 1.5-2x | high |
| R-3114 | Zero-Alloc Async Hot Path | `async-echo`, `async-pipeline` | 1.1-1.3x | medium |

### Tier 5 — Compiler and Cranelift Tuning

| ID | Title | Estimated speedup | Risk |
|---|---|---|---|
| R-3115 | Aggressive Const Propagation | 1.0-1.05x on small programs | low |
| R-3116 | Extended DCE | 1.0-1.05x | low |
| R-3117 | Cranelift Opt-Level Tuning | 1.1-1.3x on hot loops | low |

### Tier 6 — Inference ML

| ID | Title | Estimated speedup | Risk |
|---|---|---|---|
| R-3109 | Autodiff Inference-Mode Graph Skipping | 1.1-1.2x on inference paths | medium |

## Recommended Execution Order (updated)

1. **R-3108** — complete. 3.85x on cpu-string-build.
2. **R-3107** — next; highest leverage after R-3108.
3. **R-3104** — benefits all scenarios; clean refactor.
4. **R-3105** — small surgery on top of R-3104.
5. **R-3106** — supports R-3107.
6. **R-3110** — leverages R-3107.
7. **R-3111** — leverages R-3107.
8. **R-3109** — narrow impact.
9. **R-3131** — complete; real fan-out/fan-in scheduler semantics and Go parity
   certified in two release reports.
10. **R-3132** — fuse proven single-use `task_spawn`/`task_join` pairs and
   measure the reset Fast ABI; keep conservative fallback and baseline
   unchanged. The ≤1% aspiration is deferred after acceptance of the measured
   R-3132 result.
11. **R-3115 / R-3116 / R-3117** — combined compiler pass.
12. **R-3112** — conv2d opt.
13. **R-3113 / R-3114** — follow-on async work, now unblocked by R-3131.

## Acceptance Gate per Item

Each item must:

- Pass `run_tests.ps1` (zero failed expected tests).
- Pass `validate_phase31_cross_lang.py` (no > 5% Spectra drift).
- Pass `validate_r1501_bench.py` (numerical perf baseline).
- Pass `validate_r2006_performance_refresh.py`.
- Pass `validate_r2111_async_bench.py`.
- Add a regression `.spectra` test under `tests/validation/` or `tests/errors/`.
- Update `docs/performance/phase31-go-comparable/findings-r3101-initial.md`
  with the new numbers.
- Update `docs/performance/phase31-go-comparable/baseline.json` only if the
  improvement is intentional and accepted.

## R-3132 measurement contract

The `async-echo` path is process-inclusive and uses the optimized release
binary for official cross-language comparison. R-3132
adds `ConcurrentSpawnJoinFusion` after lowering and before DCE. It accepts only
same-block pairs whose handle has one use and whose gap is pure; all other
handles use the existing spawn/join ABI. The runtime keeps
`Vec<Option<SpectraHostValue>>`, increments task statistics once for a fused
pair, and does not create a visible slot. `concurrent.reset()` is also emitted
through a direct Fast ABI call.

Diagnostic output is written to
`target/phase31/async-echo-diagnostics/r3132-debug*.json` and includes a
dedicated fused variant, p95, standard deviation, exact command, profile,
binary, revision, and expected fused-operation accounting. The current
post-implementation median is about 38.9 ms versus the unchanged 33.865 ms
baseline. This result was accepted for R-3132; no baseline update was made.

## R-3130/R-3131 final result

The former `async-echo` fixture compared immediate Spectra values with real Go
goroutines. The corrected `fanout_fanin_real_concurrency.v2` contract creates
ten executable Spectra task units before joining them. A persistent two-worker
runtime executes the batch without one OS thread per task. Two complete release
reports passed all 21 scenarios with Spectra/Go ratios `1.025752` and
`1.048312`; paired variation was below 10%. The repository suite uses the
separate `--code-validation` mode so correctness validation does not repeat
thousands of benchmark processes. The historical baseline remains unchanged.

## Out of Scope for R-3103

- Goroutine-style M:N scheduler (would be a separate workstream).
- AOT cross-compile perf.
- WebAssembly backend.
