# Phase 31 Cross-Language Performance Methodology

Updated: 2026-07-13
Roadmap items: `R-3101`, `R-3102`, `R-3103`, `R-3104..R-3117`, `R-3130`

## Purpose

This document defines how SpectraLang is benchmarked against Go and Rust
on a fixed set of CPU, tensor, ML, and async scenarios. The goal is
**reproducible, machine-readable, non-regression** evidence for the
"Go-comparable performance" target introduced in `phase_31`. The gap between
Spectra and the reference languages is normally reported. `async-echo` is the
explicit exception: its versioned real-concurrency contract uses Go parity as
an acceptance gate.

## Hardware

- Reference machine: developer workstation used to update
  `docs/performance/phase31-go-comparable/baseline.json`.
- Local dev runs are normalized through the same scenario IDs, iteration counts,
  and warm-up policy, but machine variance is reported per run.
- CI gates use the same scenario IDs and rely on `max_drift_pct` rather than
  absolute nanosecond thresholds for cross-language comparisons.

## Software

| Runtime | Version pin | Build flags |
|---|---|---|
| Spectra (JIT) | in-tree CLI | performance certification uses explicit `target/release/spectralang.exe`; repository code validation uses the same release profile |
| Go | `go1.22+` | `go build -ldflags="-s -w"` |
| Rust | stable 1.80+ | `cargo build --release` |

The Rust driver for cross-language execution lives in
`runtime/examples/phase31_cross_lang_bench.rs`. The Python runner
`scripts/phase31_run_all.py` shells out to the Go and Rust binaries for each
scenario. Java benchmark fixtures are retained only as historical reference
material and are excluded from the active matrix.

## Scenarios

21 scenarios across CPU, tensor, ML, async, and additional workload domains.
Each active scenario has 3 implementations under
`benchmarks/cross-lang/<scenario>/{spectra,go,rust}/`.

### CPU

| ID | Operation | Iterations | Why it matters |
|---|---|---|---|
| `cpu-loop-sum` | sum 1..N inside tight loop | 5 outer × 200_000_000 inner | baseline integer arithmetic, loop overhead, register pressure |
| `cpu-fibs` | iterative Fibonacci(40) in a loop | 200_000 | loop + function-call overhead |
| `cpu-string-build` | concatenate 1000 short strings with separator | 200 | string alloc, format, copy |
| `cpu-hashmap` | insert + lookup 100_000 entries in hashmap | 30 | hashmap ops, hashing, alloc |

### Tensor

| ID | Operation | Shape / iters | Why it matters |
|---|---|---|---|
| `tensor-create` | `full_f(1024x1024, 1.0)` | 20 | materialization, alloc |
| `tensor-elementwise` | `relu -> tanh -> sqrt` on 1M elements | 10 | elementwise throughput |
| `tensor-reduce` | `sum_f` on 1M elements | 20 | reduction throughput |
| `tensor-matmul` | 256x256 * 256x256 | 5 | GEMM throughput |

### ML

| ID | Operation | Iters | Why it matters |
|---|---|---|---|
| `ml-mlp-step` | forward + backward + sgd_step on 64x64 MLP | 50 | end-to-end ML training step |

### Async

| ID | Operation | Iters | Why it matters |
|---|---|---|---|
| `async-echo` | 1_000 fan-out/fan-in iterations x 10 executable tasks | 3 | real scheduler/task overhead |
| `async-pipeline` | producer/consumer through channel of size 16 | 5 | context switch + channel cost |

## Reporting Schema

The driver emits JSON in this shape:

```json
{
  "schema": "spectra.phase31.bench.v1",
  "profile": "release",
  "host": "...",
  "runtimes": {"go": "...", "rust": "..."},
  "scenarios": [
    {
      "id": "cpu-loop-sum",
      "category": "cpu",
      "iterations": 1000000000,
      "results": {
        "spectra": {"median_ns": ..., "p95_ns": ..., "stddev_ns": ..., "ns_per_iter": ...},
        "go":      {"median_ns": ..., "p95_ns": ..., "stddev_ns": ..., "ns_per_iter": ...},
        "rust":    {"median_ns": ..., "p95_ns": ..., "stddev_ns": ..., "ns_per_iter": ...}
      },
      "gap_to_go": 1.42,
      "gap_to_rust": 1.07,
      "correctness_passed": true
    }
  ]
}
```

The full report is written to `target/phase31/cross-lang-report.json`. A human
summary is written to `target/phase31/cross-lang-report.md`.

## Statistical Policy

- 3 warmup iterations per scenario; 20 timed iterations follow.
- `median_ns`, `p95_ns`, and `stddev_ns` come from the 20 timed iterations.
- Standalone performance certification performs 5 complete independent
  attempts per scenario and aggregates their medians; scenarios with initial
  drift or instability may receive 2 additional confirmation attempts.
- Each attempt uses symmetric robust trimming before aggregation while keeping
  every raw sample in the report.
- `run_tests.ps1` uses `--code-validation`: one execution of each runtime for
  every scenario, with no performance certification or baseline decision.
- `independent_stddev_ns` measures variation between complete attempts and is
  the stability statistic when present.
- `ns_per_iter = median_ns / iterations`.
- A scenario with `independent_stddev_ns > median_ns * 0.10` (or, for a
  single-run diagnostic, `stddev_ns > median_ns * 0.10`) is `inconclusive`,
  not a confirmed performance regression. It must be rerun on a quiescent or
  pinned reference machine.
- A stable scenario fails only when correctness fails or its Spectra median
  exceeds the checked-in baseline drift limit after confirmation attempts.
- Baseline updates require two consecutive stable runs and review evidence;
  benchmark scripts never update `baseline.json` automatically.

## Non-Regression Gate

`scripts/validate_phase31_cross_lang.py` reads:

- `docs/performance/phase31-go-comparable/baseline.json` (Spectra median per
  scenario, last accepted version)
- The current run's `target/phase31/cross-lang-report.json`

The gate fails when:

1. Any scenario is missing.
2. Any scenario's correctness check fails.
3. Any scenario's `ns_per_iter` regresses by more than `max_drift_pct` (default
   5%) versus the baseline.
4. Numerical results deviate from the recorded reference value beyond the
   per-scenario tolerance (defaults to 1e-9 for float sums, 1e-6 for elementwise
   chains, 1e-4 for matmul).

The gate keeps Rust gaps diagnostic. `async-echo` is the exception:
`gap_to_go` is an acceptance metric and must remain within +/-5% because Go is
the declared reference runtime for R-3131.

## Local Development

```powershell
python scripts\phase31_run_all.py --spectra-binary target\release\spectralang.exe --spectra-profile release --independent-runs 5 --confirm-regressions 2 --baseline docs\performance\phase31-go-comparable\baseline.json --out target\phase31\cross-lang-report.json
python scripts\validate_phase31_cross_lang.py --baseline docs/performance/phase31-go-comparable/baseline.json --report target\phase31/cross-lang-report.json --profile release --spectra-binary target\release\spectralang.exe
python scripts\compare_phase31_reports.py target\phase31\run-1.json target\phase31\run-2.json
python scripts\diagnose_async_echo.py --binary target\release\spectralang.exe --profile release --out target\phase31\async-echo-diagnostics\release.json
```

The full `run_tests.ps1` invokes the runner and validator once under the
`phase31-cross-lang` group with `--code-validation`. Runtime implementations
within one scenario execute concurrently, while scenarios remain ordered. On
the reference workstation this reduced the Phase 31 code gate from roughly
14 minutes to about 40 seconds. Full statistical certification remains the
standalone command above.

The optional GPU speedup benchmark is not part of the default suite. Run it
with `run_tests.ps1 -Phase phase31_gpu`; when the adapter probe reports no WGPU
adapter, the report is `status = "skipped"`, not a failed language/runtime
benchmark.

`diagnose_async_echo.py` is diagnostic only. It generates temporary fixtures
under `target/phase31/async-echo-diagnostics/` for process startup,
`reset`-only, `task_spawn`-only, `task_join`-only, `spawn+join`, the fused
expression, and the full workload. It records exact commands, binary/profile,
revision, host, warmups, samples, median, p95, standard deviation, and cost per
task pair. It never edits `baseline.json`. The fused variant is diagnostic and
does not replace the official process-inclusive baseline contract.

The official Phase 31 gate uses the repository-built release binary because
Go and Rust benchmark binaries are optimized. Debug remains useful for
compiler/runtime diagnosis, but debug-vs-Go process timings are not a valid
release performance comparison.

## Updating the Baseline

The baseline is updated only when a Spectra change is intentionally raising
performance (i.e. closing a gap) and the change passes the full `run_tests.ps1`.
Updating the baseline requires:

1. Open PR referencing the corresponding `R-31xx` item.
2. Validation evidence: the new run, the comparison vs Go, and the functional
   suite result.
3. Reviewer sign-off in the PR thread.

`scripts/phase31_lock_baseline.py` creates a candidate only. It never edits
`baseline.json`; `scripts/phase31_apply_baseline.py` refuses to edit it unless
called with `--apply` after the two-run and metadata checks pass.

## Out of Scope

- GPU, distributed training, `spectra.api` HTTP server perf (other items).
- WebAssembly, AOT cross-compile.
- Macros / syntax experiments.
