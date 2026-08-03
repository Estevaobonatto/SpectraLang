# R-3104 Codegen Hot Path Evidence

- Status: `passed`
- Revision: `699db7945243343ed962ffc78c3037fd2eb69adc`
- Profile: `release`
- Classification: `benchmark_and_ir_hypothesis`
- Profiling causal claim: `False`
- Matrix: `Spectra + Go + Rust` (Java excluded)
- Scenarios: `21/21`
- Baseline modified: `False`

The evidence is benchmark/IR based; causal Linux profiling remains R-3102.

## Controlled codegen comparison

- CPU target geometric-mean ratio after/before: `1.0189812220164665`
- CPU target improvement: `-1.898122201646646%`

## Runtime steady state

- Measurement: precompiled Spectra AOT executable, excluding JIT/process startup.
- Scenarios: `6/6`

| Scenario | Spectra/Go | Spectra/Rust | Baseline drift |
|---|---:|---:|---:|
| `cpu-loop-sum` | `1.1755788700706207` | `1.3956336454145393` | `-56.5424814942362%` |
| `cpu-fibs` | `1.1549361760956054` | `2.332299340829779` | `-54.90456862077704%` |
| `cpu-hashmap` | `1.0397316152943163` | `1.0953485871219641` | `-90.1608634130382%` |
| `tensor-create` | `0.6888887551405919` | `3.7320955277830827` | `-78.56659614133991%` |
| `tensor-elementwise` | `0.5841363209814379` | `1.072452095472882` | `-64.63992841576585%` |
| `tensor-matmul` | `0.748810857720362` | `0.8718938364377649` | `-75.96000231351353%` |

## Reports

| Artifact | SHA-256 |
|---|---|
| `target\phase31\r3104-release-run-1.json` | `30f7dd411686c1e6d21b04f2df77c5b41c6a01fdda690c0e79cde909f448ce58` |
| `target\phase31\r3104-release-run-2.json` | `af13f5d27e5f5c243f2549e0009a7d994500fdb142001a9c48249136426432e8` |

## Failures

- none
