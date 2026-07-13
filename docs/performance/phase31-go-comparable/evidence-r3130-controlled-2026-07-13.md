# R-3130 Controlled Evidence — 2026-07-13

## Environment

- Spectra binary: `target/debug/spectralang.exe`
- Profile: `debug`
- Competing `cline.exe` process terminated after explicit user authorization.
- Baseline: unchanged.
- Policy: 3 warmups, 20 timed samples, 3 independent attempts, and 2
  confirmation attempts for initial drift.

## Reports

- `target/phase31/stable-run-1.json`
- `target/phase31/stable-run-2.json`
- `target/phase31/stable-report.json`
- `target/phase31/focused-r3130.json`

Semantic comparison passed:

```text
PASS: semantic Phase 31 evidence matches
```

## Result

- `async-pipeline`: stable across complete runs; aggregate variation 5.4%.
- `async-echo`: reproduced above baseline in both complete runs and focused
  run, approximately 23–28% slower with low independent variance.
- `cpu-loop-sum`: focused run remained inconclusive at 11.9% noise.
- No correctness failure or Phase 31 runner timeout occurred.

## Decision

`async-echo` is tracked by `R-3131` for profiling. The first controlled
diagnostic identified two measurable components:

- current debug CLI startup plus JIT execution is approximately 32--34 ms;
- the full workload is approximately 40--41 ms after replacing the task slot
  `Arc<OnceLock>` allocation path with `Option<SpectraHostValue>`;
- release diagnostic is approximately 24 ms for the full workload;
- runtime slot optimization passed task reuse, reset invalidation, Fast ABI,
  channels, counters, and pipeline tests;
- focused post-fix Phase 31 remained correct but measured approximately 40.5 ms
  with low independent variance, still above the 33.9 ms baseline.

This classifies current failure as benchmark process/JIT overhead plus a real
runtime allocation cost that was reduced but not fully eliminated. Baseline
remains unchanged. R-3131 and R-3130 remain open until a reference-environment
run separates stale process-inclusive baseline from remaining backend/runtime
cost and the official debug gate passes.

Diagnostic reports:

- `target/phase31/async-echo-diagnostics/debug-after-slot-fix.json`
- `target/phase31/async-echo-diagnostics/release.json`
- `target/phase31/async-echo-after-slot-fix.json`

Subsequent validation added a no-op-safe `ConcurrentRegistry::clear` fast path
when every task slot is already free. The invariant and Fast ABI tests pass,
but the focused post-reset-fast-path run remained approximately 40.9 ms with
low independent variance. This optimization is retained because it removes
proven redundant work; it does not justify changing the baseline or closing
R-3131.

The full `run_tests.ps1` execution after the slot fix had these decisive
results:

- workspace/runtime/backend tests: passed;
- Phase 31 runner: passed without timeout;
- Phase 31 validator: failed closed on `async-echo` at 16.6% above baseline;
- R-1603 backend validator: passed;
- GPU speedup: now explicitly skipped when adapter probe returns unavailable;
- all other listed gates: passed.
