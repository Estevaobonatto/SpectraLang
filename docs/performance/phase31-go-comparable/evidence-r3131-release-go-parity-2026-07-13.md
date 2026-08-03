# R-3131 release/Go parity evidence — 2026-07-13

## Finding

R-3131 is complete. The blocking problem was semantic, not a compiler
correctness defect: Go created ten real goroutines before joining, while the old
Spectra fixture passed already-materialized values to `task_spawn`.

## Correction

The versioned `fanout_fanin_real_concurrency.v2` contract now requires ten
executable tasks to be pending before fan-in. Spectra uses the batch scheduler
path, a persistent two-worker executor, and explicit concurrency diagnostics.
The compatibility API `task_spawn(value)` remains available and unchanged at
the language surface.

## Final evidence

| report | Spectra/Go ratio | paired variation | max pending | failures |
|---|---:|---:|---:|---:|
| `target/phase31/r3130-final-run-1.json` | 1.025752 | 3.4373% | 10 | 0 |
| `target/phase31/r3130-final-run-2.json` | 1.048312 | 2.5242% | 10 | 0 |

Both release reports satisfy the bilateral ±5% Go parity window and the
accepted variation limit of 10%. Each reports 10,002 executed task units and a
maximum of ten simultaneously pending tasks. All 21 scenarios passed, and the
two reports are semantically equivalent.

The historical Spectra baseline remains unchanged. The evidence changes the
benchmark contract only because the former cross-language workloads were not
semantically equivalent; it does not rewrite prior measurements to hide a
regression.
