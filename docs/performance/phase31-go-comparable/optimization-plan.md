# Phase 31 Optimization Plan (R-3103)

Updated: 2026-08-01
Roadmap item: `R-3103 Optimization Implementation Plan`
Truth owner: `backend`
Evidence class: `benchmark_and_ir_hypothesis`

## Contract and evidence boundary

This document is the executable plan for R-3104–R-3117. It is based on the
current release binary, the 21-scenario Phase 31 contract, two independent
release reports, and O0/O3 IR snapshots. The machine-readable evidence is
`evidence-r3103-benchmark-ir.json`; its report and IR hashes are the audit
anchors.

The measurements identify where an intervention is worth testing. They are not
causal profiler attribution: no row below claims a `perf`, FlameGraph, or
callgrind result. Causal attribution remains R-3102, which is intentionally
`in_progress` while the Linux/WSL2 environment is unavailable. A row can only
be promoted after its implementation supplies a fresh benchmark, correctness
regression coverage, and an explicit rollback comparison.

The checked-in baseline is immutable. The official standalone contract uses a
release build, five independent attempts, three warmups, twenty timed samples,
and at most two confirmation attempts. Repository code validation is a separate
single-run functional gate. The plan is not complete while either release
report is semantically incompatible, a scenario is inconclusive, or strict
cross-language drift exceeds 5%.

## Current gate decision (2026-08-01)

R-3103 is **complete** at revision
`f7ba1dbb3295084342fc002c7816eadf096adafb`. The two release reports are
semantically compatible, all 21 scenarios pass correctness and strict
cross-language validation, and the active matrix is Spectra + Go + Rust (Java
fixtures remain historical and are not executed). The reports use five
independent attempts, three warmups, and twenty timed samples. `async-echo`
measures `1.121851x` and `1.152715x` against Go, both within the accepted
`1.202162x` limit, with maximum paired dispersion `7.3441%`. `tensor-create`
and `tensor-reduce` are conclusive in both reports.

The deterministic IR manifest covers O0/O3 for all 21 scenarios and matches the
current release binary and Git revision. The five tracked snapshots are
refreshed. Baseline SHA-256 remains
`452a2e0e25db99d1175f5cbd1a50ac969512055e70c6ebf1c8c5ef959ca8b30b` before and
after validation. The evidence remains classified
`benchmark_and_ir_hypothesis`; R-3102 is still `in_progress`, and R-3104 is
`in_progress` while its implementation is measured against the rejection gate.

## Current evidence inputs

- Binary: `target/release/spectralang.exe`, built with
  `cargo build --release -p spectra-cli` at the current Git revision.
- Functional report: `target/phase31/r3103-code-validation.json`.
- Statistical reports: `target/phase31/r3103-release-run-1.json` and
  `target/phase31/r3103-release-run-2.json`.
- Baseline: `baseline.json`; the validator records its SHA-256 before and after
  the gate and requires `baseline_modified: false`.
- IR root: `target/phase31/r3103-ir/<scenario>/{o0,o3}.txt`, validated by
  `target/phase31/r3103-ir/manifest.json`; the five highest-priority snapshots
  are copied to `ir/r3103/` for review.
- R-3104 inputs: `target/phase31/r3104-codegen-{before,after}.json`,
  `target/phase31/r3104-ir/manifest.json`, and the blocked evidence in
  `evidence-r3104-codegen.{json,md}`. The implementation passed JIT/AOT smoke
  compilation, but did not satisfy the codegen and strict gates.

The evidence generator records the current Git revision and fails closed on a
revision mismatch, missing scenario, duplicate scenario, inconclusive sample,
or a report failure. Therefore a blocked evidence file is still useful: it
describes the exact remaining gate without silently upgrading a hypothesis to
fact.

## R-3133 async-echo reconciliation

R-3133 renews the async evidence independently of this optimization matrix.
The accepted focused current-head report uses the real
`fanout_fanin_real_concurrency.v2` batch path and is preserved in
`evidence-r3133-async-echo.{json,md}`. It records `1.154469x` against Go with
`3.0062%` paired dispersion. The v2 diagnostic confirms 1,000 batch spawns and
joins, 10,002 executed tasks, `max_pending_tasks=10`, zero task failures, and
balanced batch accounting. The deterministic classification is
`runtime_batch_path` (hypothesis only; no causal profiler claim). The user
accepted this focused criterion; the immutable baseline and historical R-3131
and R-3132 evidence remain preserved. R-3104 is now in progress; this does not
authorize R-3105 or later work.

## Prioritized implementation matrix

| ID | Cenário(s) afetado(s) | Evidência atual | Hipótese de gargalo (confiança) | Intervenção planejada | Métrica primária | Ganho esperado | Risco de rejeição | Critério de rollback | Dependências | Comando de validação |
|---|---|---|---|---|---|---|---|---|---|---|
| R-3104 | `cpu-loop-sum`, `cpu-fibs`, `cpu-hashmap`, workloads tensor | Medianas/dispersion dos dois reports + contagens O0/O3; lookup/codegen aparece no IR, sem atribuição causal | Mapa esparso e lookup de host podem dominar lowering (média) | Trocar mapa por `Vec<Option<Value>>` denso, pré-computar `HostNameRecord`, separar JIT/AOT | ns/iter e tempo de lowering; lookup count | 1.2–1.5x nos cenários CPU, sem piora tensor | Mudança de ordem/ABI, picos em módulos grandes | Reverter se qualquer cenário correto exceder +5% ou se o IR mudar sem ganho | R-3103 | `cargo test -p spectra-backend`; `python scripts/phase31_run_all.py ... --independent-runs 5`; strict cross-lang |
| R-3105 | `ml-mlp-step`, `tensor-elementwise`, `async-pipeline` | Host-call count e materialização de nomes nos snapshots; mediana atual registrada no JSON | Alocações e `to_string()` na fronteira hostcall são custo repetido (média) | Cache de nomes, batching de hostcalls consecutivos quando semântico, remover conversões por chamada | host calls/iter, alocações e ns/iter | 1.1–1.3x em ML/tensor/async | Reordenação de efeitos ou lifetime de handles | Reverter se hostcall count não cair ou surgir divergência numérica/async | R-3103, R-3104 (se o cache compartilhar o path) | `cargo test -p spectra-midend -p spectra-runtime`; Phase 31 strict |
| R-3106 | Loops CPU e criação de tensores | Contagem de `alloca` O0/O3 por cenário e lifetime visível no IR | Slots temporários não são reutilizados entre iterações (média-baixa) | Hoist de allocas invariantes, fusão de slots e reuse por lifetime não sobreposto | allocas/função e bytes alocados | 1.05–1.10x onde allocas dominam | Alias/lifetime incorreto e regressão numérica | Reverter se allocas não reduzirem ou sanitizer/fixtures falharem | R-3103, R-1502 | `cargo test -p spectra-midend`; snapshots O0/O3; strict cross-lang |
| R-3107 | `tensor-create` e passos de materialização | Evidência concluída em `181_phase31_buffer_pool.spectra`, pool hit/miss e gate tensor; apontar para `roadmap.toml` | Pool tipado elimina zero-fill/intermediários (alta; já validada) | Manter buffer reuse type/lifetime-safe; só ampliar cobertura se novo benchmark exigir | allocations, bytes e mediana `tensor-create` | Resultado já aceito: redução de alocação e gap release invertido | Reuso de shape/dtype/layout incompatível | Reverter qualquer mudança que altere contagem ativa, bytes ou tolerância R-1503 | R-1502 | `cargo test -p spectra-runtime`; `spectralang run tests/validation/181_phase31_buffer_pool.spectra`; Phase 31 gate |
| R-3108 | `cpu-string-build` | Regressão `180_phase31_string_builder.spectra` e medição aceita de string builder; snapshot textual versionado | Materialização repetida de strings cria cópias no ABI (alta; já validada) | Preservar builder/ABI otimizado e medir novas mudanças contra o artefato aceito | ns/iter e bytes/cópias por string | Resultado já aceito: melhoria medida sem regressão R-109 | Quebra de string cross-module ou ownership | Reverter se R-109 ou o cenário de string regredir >5% | R-109 | `cargo test -p spectra-runtime`; `spectralang run tests/validation/180_phase31_string_builder.spectra`; strict |
| R-3109 | `ml-mlp-step` | Host-call/IR count e mediana do caminho de inference; training fixtures permanecem controle | Construção/retenção de grafo em inference é desnecessária (média) | Pular graph build/free somente em inference mode, mantendo training path | ns/step e graph nodes/step | 1.1–1.2x em serving | Misturar inference e training ou perder gradientes | Reverter se training output mudar ou graph nodes não caírem | R-503, R-3103 | ML regression suite + Phase 31 strict |
| R-3110 | `tensor-elementwise`, `tensor-matmul` | Throughput e tolerância R-1503; snapshots identificam operações relevantes | Kernels escalares deixam unidades SIMD ociosas (média) | SSE2/AVX2/NEON com CPUID dispatch e fallback escalar | elementos/s e erro numérico | 2–4x em elementwise | CPU dispatch não determinístico, NaN/rounding | Reverter se erro exceder R-1503 ou fallback não for funcional | R-3103, R-1503 | kernel conformance + dedicated benchmark + strict |
| R-3111 | `tensor-matmul` | IR/mediana atual; shapes 256–2048 devem ser medidos no item de implementação | Tiles register-blocked reduzem cache miss e chamadas auxiliares (média) | Matmul tiled/register-blocked, com benchmark dedicado para 256, 512, 1024, 2048 | GFLOP/s, mediana e erro relativo | 2–4x contra o caminho atual | Tiling ruim para shapes pequenos, overflow/tolerância | Reverter se qualquer shape perder >5% ou erro exceder R-1503 | R-3103, R-1503 | benchmark matmul dedicado + numerical suite + strict |
| R-3112 | Conv2D dedicado (a criar pelo próprio item) | Nenhum cenário canônico ainda; o plano exige fixture/benchmark e tolerância desde o início | Im2col+GEMM pode superar loops diretos (baixa, a confirmar) | Criar benchmark Conv2D com shapes, dtype e tolerância documentados; só então implementar | images/s, mediana e erro máximo | 1.5–2x após baseline dedicado | Im2col aumenta memória e piora batch pequeno | Reverter se memória ou latência exceder baseline dedicado | R-3103, R-3110 | novo Conv2D fixture/benchmark + numerical gate + strict |
| R-3113 | `async-echo` | Task counts, mediana e parity; causalidade ainda não alegada | Scheduler/fila pode pagar custo por task (média-baixa) | Work-stealing/pool somente com semântica fanout/fanin preservada | task creation ns e throughput | 1.5–2x no async fanout | Ordem, fairness ou starvation | Reverter se task accounting, resultado ou parity mudar | R-3103, R-3131 | async regression + dedicated benchmark + strict |
| R-3114 | `async-echo`, `async-pipeline` | Host calls/allocas e mediana no caminho assíncrono | Handles/futures alocam em cada await (média) | Remover alocações no hot path sem alterar ownership/observabilidade | allocs/task e ns/task | 1.1–1.3x | Use-after-free ou cancelamento incorreto | Reverter se allocs não caírem ou cancel/error tests falharem | R-3103, R-3131 | async cancellation suite + benchmark + strict |
| R-3115 | Cenários CPU pequenos | O3 snapshots e tamanho de IR; impacto esperado pequeno (média-baixa) | Constantes não são reduzidas antes do codegen (média-baixa) | Propagação agressiva com limites de custo e overflow explícitos | tamanho IR e ns/iter | 1.0–1.05x | Code size explosion ou overflow de avaliação | Reverter se IR crescer sem ganho ou diagnostics mudarem | R-3103 | midend tests + IR diff + Phase 31 strict |
| R-3116 | Cenários CPU/ML | O0/O3 mostram instruções/blocks não consumidos; sem profiler causal (baixa) | DCE deixa instruções mortas em paths com efeitos modelados conservadoramente | Estender DCE apenas para operações comprovadamente puras | instruções/blocks e ns/iter | 1.0–1.05x | Remover efeitos observáveis/host calls | Reverter se qualquer fixture mudar saída ou host-call count esperado | R-3103 | midend regression + IR diff + strict |
| R-3117 | Loops quentes | Comparação controlada O0/O3 e mediana por cenário; Cranelift é hipótese (média) | Nível de otimização atual pode deixar passes quentes desativados | Comparar níveis Cranelift em experimento controlado, mantendo ABI e baseline | ns/iter, code size e compile time | 1.1–1.3x em loops quentes | Compile time/code size ou regressão de cold path | Reverter se ganho não superar ruído ou compile time exceder limite | R-3103 | controlled O0/O3/Cranelift benchmark + strict |

## Gate and rollback protocol

For every implementation row, preserve the baseline and run a focused
correctness test before measuring. Record the command, revision, binary,
profile, sample policy, median, p95, dispersion, numerical tolerance, and
failure class. Reject the change when correctness fails, dispersion is
inconclusive, any accepted scenario drifts beyond 5%, or the row's primary
metric does not improve within its stated noise envelope. Roll back to the
previous implementation and keep the row `in_progress`; do not rewrite the
baseline to make a regression disappear.

The R-3103 focused gate is:

```powershell
.\run_tests.ps1 -Phase phase31_r3103_plan
```

It runs validator unit tests, TOML/dependency validation, report and IR hash
validation, matrix coverage checks, and `git diff --check`. Linux profiling is
deliberately absent from this command and remains the R-3102 follow-up.

## Out of scope

- Implementing R-3105 or any later optimization listed in the matrix.
- Fixing WSL2, installing `perf`, FlameGraph, or Valgrind.
- Repairing the independent `data_file`/`folded_file` bug in
  `scripts/phase31_profile.py`.
- Synthetic flamegraphs or causal claims without official profiler artifacts.
- Mutating the Phase 31 baseline or closing R-2505 before the remote
  PostgreSQL 16 report arrives.
