# R-3133 Async Echo Reconciliation

- Status: `passed`
- Revision: `b358153d18820e3e70ca9f1098d3fb7fb7fadab7`
- Classification: `external_noise` (high)
- Baseline modified: `False`

This report covers the current batch benchmark only; historical R-3131/R-3132 evidence remains unchanged.

## Async-echo reports

| Report | Gap to Go | Paired dispersion | Parity |
|---|---:|---:|---|
| `target\phase31\r3133-async-echo-only.json` | 1.154469 | 3.0061623710320267% | True |

## Batch variants

| Variant | Median ns | Stddev % |
|---|---:|---:|
| `batch-reset-only` | 45353000 | 21.395 |
| `batch-spawn-only` | 52508250 | 35.703 |
| `batch-join-only` | 32836500 | 28.084 |
| `batch-full` | 34176950 | 28.432 |
| `batch-full-no-reset` | 35524000 | 28.913 |

## Failures

- none
