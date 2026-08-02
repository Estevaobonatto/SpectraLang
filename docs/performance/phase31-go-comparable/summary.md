# Phase 31 Implementation Summary

Updated: 2026-08-02
Roadmap items: `R-3101`, `R-3103`, `R-3108`, `R-3130`, `R-3131`, `R-3132`, and
`R-3133`, `R-3104`, and `R-3105` are complete. Remaining item statuses are tracked authoritatively in
`roadmap/roadmap.toml`.

## What Was Built

### R-3101 Cross-Language Benchmark Suite (complete)

- **21 scenarios** × **3 active languages** (Spectra + Go + Rust) under `benchmarks/cross-lang/`.
- **Driver** in `scripts/phase31_run_all.py`: builds the Go/Rust binaries once per scenario, supports full 3-warmup/20-sample performance certification and a fast `--code-validation` mode, and emits `target/phase31/cross-lang-report.{json,md}`. Java fixtures remain historical and are not executed.
- **Gate** in `scripts/validate_phase31_cross_lang.py`: checks the complete 21-scenario contract, correctness, metadata, noise, and ≤ 15% drift vs checked-in baseline.
- **Baseline helpers** in `scripts/phase31_lock_baseline.py` + `scripts/phase31_apply_baseline.py`: create a reviewed candidate; applying requires explicit `--apply`, two stable runs, matching metadata, and no inconclusive scenario.
- **Methodology** in `docs/performance/phase31-go-comparable/methodology.md`.
- **Findings** in `docs/performance/phase31-go-comparable/findings-r3101-initial.md`.
- **Wired into `run_tests.ps1`** as `phase31_run_all` + `validate_phase31_cross_lang`.

### R-3102 Profiling (in_progress)

- O orquestrador reproduzível é `scripts/phase31_profile.py` e o teste de
  contrato é `scripts/test_phase31_profile.py`.
- A captura oficial permanece pendente porque a distribuição WSL2 configurada
  não consegue anexar seu VHDX (`ERROR_PATH_NOT_FOUND`). Nenhum flamegraph
  sintético ou attribution baseada apenas em benchmark foi aceito.
- The first R-3101 pass itself surfaced the most important finding: **string concatenation is 50x slower than Go** in `cpu-string-build`.

### R-3103 Optimization Plan (complete)

- `docs/performance/phase31-go-comparable/optimization-plan.md` ranks 14 optimization items by impact × feasibility / risk, backed by current release evidence.
- The two current-head release reports pass the 21-scenario functional and strict gates with the active Spectra + Go + Rust matrix; Java fixtures remain historical only.
- O0/O3 IR for all 21 scenarios is covered by `target/phase31/r3103-ir/manifest.json`, and the five review snapshots are versioned under `ir/r3103/`.
- The evidence is `benchmark_and_ir_hypothesis` only; `R-3102` remains the separate causal Linux profiling workstream in progress.
- Tier 1: R-3108 (string concat), R-3107 (tensor buffer reuse).
- Tier 2: R-3104 and R-3105 complete; R-3106 remains the next backend/midend item.
- Tier 3: R-3110, R-3111, R-3112 (SIMD + matmul + conv).
- Tier 4: R-3113, R-3114 (async).
- Tier 5: R-3115, R-3116, R-3117 (compiler + cranelift tuning).
- Tier 6: R-3109 (autodiff inference skip).

### R-3104 Codegen hot path (complete)

- `backend/src/codegen.rs` and `backend/src/aot.rs` now use the dense
  `DenseValueMap`, seed parameters by their real IR ids, preserve PHI/
  terminator lookup errors, and pre-intern JIT/AOT host names deterministically.
- JIT and AOT smoke compilation of the same fixture passed; the 21-scenario
  code-validation report passed `21/21` with Spectra + Go + Rust and no Java.
- Scalar `alloca` analysis is a single linear use scan with conservative
  dominance checks for cross-block initialization; unsafe cases retain memory
  lowering. The paired control/candidate codegen guardrail reports no
  individual regression above 5%; geometric gain is informational only.
- The runtime-primary AOT gate passes all six controlled scenarios: the maximum
  Spectra/Go ratio is `1.175579x`, all candidate/control runtime regressions are
  below 5%, and the Spectra baseline has no regression above 5%.
- Two current-head release reports pass strict validation and semantic
  compatibility for 21/21 Spectra + Go + Rust scenarios; Java is excluded and
  async-echo remains within `1.202162x` with ≤10% paired dispersion. Evidence is
   published as `passed` in `evidence-r3104-codegen.{json,md}`; R-3104 is
   complete and the immutable baseline is unchanged.

### R-3105 Hostcall batching (complete)

- `runtime/src/ffi.rs` now borrows host names and exposes the internal bounded
  `spectra_rt_host_invoke_batch` dispatcher, preserving order and stopping at
  the first failure. Runtime tests cover result propagation, invalid
  descriptors, and failure ordering.
- JIT/AOT share conservative lowering: at most eight independent generic
  hostcalls per basic block, stack-owned descriptor/argument/result arenas, and
  individual fallback for Fast ABI or uncertain cases. The contract fixture
  passes through both JIT and AOT.
- The dedicated clean-control AOT benchmark uses 5 groups × 3 warmups × 20
  samples. Candidate/control is `0.474774x` (`52.5%` faster), with one batched
  site, three grouped calls, zero fallback calls in the hot fixture, and
  `40`/`24` bytes of argument/result arenas.
- The current 21-scenario Spectra + Go + Rust reports pass strict validation,
  the six R-3104 AOT scenarios remain within their gates, Java is excluded,
  and `baseline.json` remains byte-for-byte unchanged. Evidence is published
  in `evidence-r3105-hostcall-batching.{json,md}`.

## Historical Measured Baseline

Values below are checked-in historical reference values, not a claim that the
current working tree passes the gate. Current controlled evidence is recorded
in the versioned R-3133 artifacts `evidence-r3133-async-echo.{json,md}` once
its focused gate passes. Diagnostic JSON is under
`target/phase31/async-echo-diagnostics/`. The earlier R-3131 and R-3132
reports remain historical evidence and are not silently rewritten.

| scenario | gap vs Go | gap vs Rust |
|---|---:|---:|
| `cpu-loop-sum` | 2.34x | 3.11x |
| `cpu-fibs` | 2.24x | 3.43x |
| `cpu-string-build` | **71.7x** | 66.9x |
| `cpu-hashmap` | 6.0x | 6.9x |
| `tensor-create` | 4.5x | 20.8x |
| `tensor-elementwise` | 1.2x | 2.0x |
| `tensor-reduce` | 1.05x | 1.6x |
| `tensor-matmul` | 1.94x | 2.5x |
| `ml-mlp-step` | 2.75x | 3.9x |
| `async-echo` | 68.2x | 2.4x |
| `async-pipeline` | 2.29x | 3.1x |

`tensor-reduce` is already at Go parity. `tensor-elementwise` is at 1.2x Go.
The largest absolute gaps are `cpu-string-build` (R-3108), `tensor-create`
(R-3107), and `cpu-hashmap` (R-3107 + language surface for maps).

## Implementation Order (per optimization-plan.md)

1. R-3108 (string builder API, host function ready, language surface needs the typed `str.builder_*` API)
2. R-3107 (tensor buffer pool)
3. R-3104 (dense value map, complete)
4. R-3105 (hostcall batching, complete)
5. R-3106 (alloca hoisting; next open P0)
6. R-3110 (SIMD elementwise)
7. R-3111 (tiled matmul)
8. R-3109 (autodiff inference skip)
9. R-3133 (current async-echo batch reconciliation; R-3131 historical evidence)
10. R-3115 / R-3116 / R-3117 (compiler + cranelift)
11. R-3112 (im2col + GEMM)
12. R-3113 / R-3114 (async)

## Acceptance Evidence

- `python scripts/phase31_run_all.py` — full 21-scenario run completes.
- R-3103 release evidence uses revision `f7ba1dbb3295084342fc002c7816eadf096adafb`, 5 independent attempts, 3 warmups, 20 timed samples, and two semantically compatible reports.
- `async-echo` is `1.121851x` / `1.152715x` vs Go with maximum paired dispersion `7.3441%`, within the accepted `1.202162x` limit; the baseline SHA-256 is unchanged.
- `python scripts/generate_r3103_ir.py` produces a current-binary manifest and O0/O3 dumps for all 21 scenarios; the five tracked textual snapshots are synchronized.
- `run_tests.ps1 -Phase phase31_r3103_plan` — R-3103 validator, IR manifest, roadmap, matrix coverage, and diff checks pass.
- Historical release certifications are retained as context only. The accepted
  current R-3133 release evidence is the focused
  `r3133-async-echo-only.json` report; it records `async-echo = 1.154469x`
  against Go with `3.0062%` paired dispersion. The fast 21-scenario
  code-validation report is `r3133-code-validation.json`.
- R-3104 closure evidence is `evidence-r3104-codegen.{json,md}` at revision
  `699db7945243343ed962ffc78c3037fd2eb69adc`; it records the clean-control
  codegen comparison, paired AOT steady-state, current IR manifest, and
  unchanged baseline.
- R-3105 closure evidence is `evidence-r3105-hostcall-batching.{json,md}` at
  revision `2830c5fefa8b25d62c837d6e6b2fa77fd36aa8bf`; it records the dedicated
  clean-control speedup, batch counters, two release reports, six-scenario AOT
  guardrail, JIT/AOT fixture, and unchanged baseline.
- `python scripts/phase31_run_all.py --code-validation ...` plus the matching
  validator is the repository correctness gate and completes in about 40 s.
- `python scripts/validate_phase31_cross_lang.py --strict --max-drift 5` —
  both current R-3103 release reports pass strict mode without changing the
  baseline.
- Gate wired into `run_tests.ps1` (line ~1325).
- `run_tests.ps1` continues to run all other gates (R-1501, R-2006, R-2111,
  etc.) without regression.

## Files Added (this session)

```
docs/performance/phase31-go-comparable/methodology.md
docs/performance/phase31-go-comparable/baseline.json
docs/performance/phase31-go-comparable/findings-r3101-initial.md
docs/performance/phase31-go-comparable/optimization-plan.md
docs/performance/phase31-go-comparable/summary.md
benchmarks/cross-lang/<scenario>/{spectra,go,rust}/...
scripts/phase31_run_all.py
scripts/validate_phase31_cross_lang.py
scripts/phase31_lock_baseline.py
scripts/phase31_apply_baseline.py
.kilo/plans/1782221884148-go-comparable-performance-plan.md
```

## Files Updated

- `roadmap/roadmap.toml` (added phase_31 + R-3101..R-3117)
- `docs/roadmap-backlog.md` (added phase_31 section)
- `run_tests.ps1` (wired phase31 gate)
- `docs/performance/phase31-go-comparable/methodology.md` (workload
  adjustments for async-echo)

## What's Not Done (and why)

- **R-3102 (profiling)** is initial-only: `cargo flamegraph`, `perf`, and
  `pprof` for Go were not invoked because the session was scoped to
  planning + suite + gate + R-3108. The findings doc is the input R-3103
  consumed; R-3102 will produce a deeper profile-driven supplement when
  invoked.
- **R-3104** and **R-3105** are complete under their focused gates. R-3106 and
  R-3109..R-3116 still require their own focused gates;
  existing complete items such as R-3107, R-3108, R-3117, and R-3118 are
  preserved.

## Next Session Suggestions

1. Keep the historical baseline unchanged unless a separately reviewed
   candidate has two stable release runs.
2. R-3102: run `cargo flamegraph`, `perf`, `pprof` on each scenario;
   commit artifacts under `docs/performance/phase31-go-comparable/profiles/`.
3. Implement the next open optimization, R-3106, only after its own focused
   baseline-preserving gate is prepared.
