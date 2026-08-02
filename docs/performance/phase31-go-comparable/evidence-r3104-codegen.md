# R-3104 Codegen Hot Path Evidence

- Status: `blocked`
- Revision: `f7ba1dbb3295084342fc002c7816eadf096adafb`
- Profile: `release`
- Classification: `benchmark_and_ir_hypothesis`
- Profiling causal claim: `False`
- Matrix: `Spectra + Go + Rust` (Java excluded)
- Scenarios: `21/21`
- Baseline modified: `False`

The evidence is benchmark/IR based; causal Linux profiling remains R-3102.

## Controlled codegen comparison

- CPU target geometric-mean ratio after/before: `0.9746671590546341`
- CPU target improvement: `2.533284094536592%`

## Runtime steady state

- Measurement: precompiled Spectra AOT executable, excluding JIT/process startup.
- Scenarios: `6/6`

| Scenario | Spectra/Go | Spectra/Rust | Baseline drift |
|---|---:|---:|---:|
| `cpu-loop-sum` | `1.0860718085106382` | `1.2546562286851952` | `-41.035254265187795%` |
| `cpu-fibs` | `1.232159168042369` | `1.874937039752622` | `-41.20735301688311%` |
| `cpu-hashmap` | `1.1304485562097` | `1.2456319737053467` | `-85.01884278120149%` |
| `tensor-create` | `0.7532335087840979` | `2.6353390554082594` | `-76.46787691301844%` |
| `tensor-elementwise` | `0.6702184878611862` | `1.0778194976269253` | `-49.788970832311044%` |
| `tensor-matmul` | `0.8578910019869429` | `0.9366441615938019` | `-62.69380084175214%` |

## Reports

| Artifact | SHA-256 |
|---|---|
| `target\phase31\r3104-release-run-1.json` | `7470e7619bc0090495cc4f010c8a7f477bafa3d31c4a9883ee1eaea6bcb523b4` |
| `target\phase31\r3104-release-run-2.json` | `1b3bfdcf0fd0139fb086d4f1990e75b93d5cead960fef30c0fbc21299f5ea3a2` |

## Failures

- async-echo: Go parity gap must be within 0 < gap_to_go <= 1.202162x
- async-echo: Go reference parity is outside the accepted window
- codegen CPU target geometric mean improved only 2.533% (< 5%)
