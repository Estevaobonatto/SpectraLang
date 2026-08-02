# R-3105 Hostcall Batching Evidence

- Status: **BLOCKED**
- Revision: `171458d4faab00db9380fd80746e0634a109c413`
- Dedicated candidate/control: `0.605769996486708` (gate `<= 0.90x`)
- Functional matrix: `False` (Spectra + Go + Rust, 21 scenarios)
- JIT/AOT fixture: `True`
- Baseline unchanged: `True`

The benchmark is classified as `benchmark_and_ir_hypothesis`; no causal profiling claim is made.
Java is excluded from the official matrix.

## Gate failures

- release-1: cpu-loop-sum: measurement is inconclusive (dispersion > 10%)
- release-1: cpu-fibs: measurement is inconclusive (dispersion > 10%)
- release-1: tensor-create: measurement is inconclusive (dispersion > 10%)
- release-1: tensor-elementwise: measurement is inconclusive (dispersion > 10%)
- release-1: async-echo: Go parity gap must be within 0 < gap_to_go <= 1.202162x
- release-1: async-echo: Go reference parity is outside the accepted window
- release-1/cpu-loop-sum: baseline regression exceeds 5%
- release-1/async-echo: baseline regression exceeds 5%
- release-2: cpu-loop-sum: measurement is inconclusive (dispersion > 10%)
- release-2: cpu-fibs: measurement is inconclusive (dispersion > 10%)
- release-2: cpu-hashmap: measurement is inconclusive (dispersion > 10%)
- release-2: tensor-matmul: measurement is inconclusive (dispersion > 10%)
- release-2: ml-mlp-step: measurement is inconclusive (dispersion > 10%)
- release-2: async-echo: reference measurement is inconclusive (paired dispersion > 10%)
- release-2: async-echo: Go parity gap must be within 0 < gap_to_go <= 1.202162x
- release-2: async-echo: Go reference parity is outside the accepted window
- release-2/async-echo: baseline regression exceeds 5%
- steady-state: steady-state tensor-create: candidate/control runtime regression is 5.217% (> 5%)
