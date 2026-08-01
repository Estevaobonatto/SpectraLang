# R-3133 Async Echo Reconciliation

- Status: `blocked`
- Revision: `95b04bdead6e60207c0fdf9688ef6de774dc87a1`
- Classification: `runtime_batch_path` (medium)
- Baseline modified: `False`

This report covers the current batch benchmark only; historical R-3131/R-3132 evidence remains unchanged.

## Async-echo reports

| Report | Gap to Go | Paired dispersion | Parity |
|---|---:|---:|---|
| `target\phase31\r3133-release-run-1.json` | 1.149 | 4.087352429842222% | False |
| `target\phase31\r3133-release-run-2.json` | 1.137 | 2.3583162460074556% | False |

## Batch variants

| Variant | Median ns | Stddev % |
|---|---:|---:|
| `batch-reset-only` | 30299350 | 9.47 |
| `batch-spawn-only` | 33340300 | 12.586 |
| `batch-join-only` | 29195250 | 8.779 |
| `batch-full` | 29138600 | 6.192 |
| `batch-full-no-reset` | 26621400 | 6.008 |

## Failures

- async-echo: gap to Go is outside 0.95..1.05
- async-echo: reference_performance_passed is not true
- async-echo: gap to Go is outside 0.95..1.05
- async-echo: reference_performance_passed is not true
