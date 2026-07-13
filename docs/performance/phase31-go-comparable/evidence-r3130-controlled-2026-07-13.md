# R-3130 Controlled Evidence — 2026-07-13

## Environment

- Spectra binary for the official Phase 31 gate: `target/release/spectralang.exe`
- Profile: `release`
- Competing `cline.exe` process terminated after explicit user authorization.
- Baseline: unchanged.
- Policy: 3 warmups, 20 timed samples, 3 independent attempts, and 2
  confirmation attempts for initial drift.

## Reports

The latest official report is `target/phase31/cross-lang-report.json`.

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
- `async-echo`: functionally correct, but the Go reference ratio was `0.908`
  (9.2% faster than Go), outside the bilateral +/-5% contract.
- The runner completed within the 1800-second timeout; the validator failed
  closed on the reference-parity rule.

## Decision

`async-echo` is tracked by `R-3131` for reference-parity diagnosis. The
controlled diagnostic identified two measurable components:

- current debug CLI startup plus JIT execution is approximately 32--34 ms;
- the full workload is approximately 40--41 ms after replacing the task slot
  `Arc<OnceLock>` allocation path with `Option<SpectraHostValue>`;
- release diagnostic is approximately 24 ms for the full workload;
- runtime slot optimization passed task reuse, reset invalidation, Fast ABI,
  channels, counters, and pipeline tests;
- focused post-fix Phase 31 remained correct but measured approximately 40.5 ms
  with low independent variance, still above the 33.9 ms baseline.

The profile mismatch between debug Spectra and optimized Go is corrected in
the runner. The remaining failure is a reference-parity/variance issue:
focused release runs measured Spectra at 0.900--0.910 of Go with 6.7--7.9%
variance. No language correctness or runtime regression is proven. Baseline
remains unchanged. R-3131 and R-3130 remain open until two release reports
meet the declared Go and stability gates.

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
