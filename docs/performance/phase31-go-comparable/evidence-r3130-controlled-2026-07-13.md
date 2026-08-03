# R-3130 Controlled Evidence — 2026-07-13

## Result

R-3130 is complete. Two independent full release certifications passed all 21
scenarios without missing commands, timeout, functional error, or
`INCONCLUSIVE` result. The historical baseline was not changed.

## Performance evidence

Artifacts:

- `target/phase31/r3130-final-run-1.json`
- `target/phase31/r3130-final-run-2.json`

Both used `target/release/spectralang.exe`, five independent attempts, three
warmups, twenty timed samples, and up to two confirmations. `async-echo` used
the `fanout_fanin_real_concurrency.v2` contract and reported:

| run | Spectra/Go ratio | paired variation | max pending | executed tasks |
|---|---:|---:|---:|---:|
| 1 | 1.025752 | 3.4373% | 10 | 10002 |
| 2 | 1.048312 | 2.5242% | 10 | 10002 |

Both ratios are inside the bilateral 0.95–1.05 window and both variations are
below 10%. Semantic comparison returned:

```text
PASS: semantic Phase 31 evidence matches
```

## Root causes and corrections

The former fixture compared an immediate Spectra value with real Go goroutines.
The corrected fixture registers ten executable task units before fan-in. The
runtime uses a persistent two-worker executor and reports pending, executed,
joined, failed, cancelled, lock, and slot metrics.

The apparent `phase31_run_all` hang had a separate harness cause:
`run_tests.ps1` waited for the child process before draining redirected output.
The pipe filled and blocked both processes. Output is now drained asynchronously
and timeout termination kills the complete process tree.

The performance workload itself starts roughly 9,660 processes in the worst
case: 21 scenarios, five attempts, four runtimes, three warmups, twenty timed
samples, plus confirmations. It therefore remains a standalone certification.
`run_tests.ps1` uses `--code-validation`, which executes each runtime once per
scenario and runs the four runtime implementations concurrently. The measured
Phase 31 code gate fell from about 14 minutes to 39.82 seconds.

## Final repository gate

- Python Phase 31 gate tests: 22 passed.
- R-1603 GPU backend validator: passed.
- R-2013 aggregate validator: passed; 8/8 projects and zero untracked failures.
- `run_tests.ps1`: exit code 0 in approximately 370 seconds.
