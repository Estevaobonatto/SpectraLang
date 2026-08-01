# R-3103 Benchmark + IR Evidence

- Status: `blocked`
- Schema: `spectra.phase31.r3103_evidence.v1`
- Revision: `bd48a6b9a13631eeac9dd3b906b38e3595a4b19a`
- Profile: `release`
- Classification: `benchmark_and_ir_hypothesis`
- Profiling causal claim: `False`
- Baseline modified: `False`

The report is benchmark/IR evidence only. Linux perf/flamegraph attribution remains R-3102.

## Report hashes

| Report | SHA-256 |
|---|---|
| `target\phase31\r3103-release-run-1.json` | `122317bbe8751bd1f43abe9a6e471f28583c477ebbc087d7bb9f694a1da952ad` |
| `target\phase31\r3103-release-run-2.json` | `21f4d5f7789bd3ea9da133d16ce45ad16e8de0fda6ac4851bf333026a76b8d96` |

## Scenario evidence

| Scenario | Median ns | P95 ns | Stddev ns | Dispersion % | Gap to Go | Correctness | Reference parity | Failure class |
|---|---:|---:|---:|---:|---:|---|---|---|
| cpu-loop-sum | 29973600 | 38995600 | 5209002 | 3.8018723143032536 | 1.604 | True | None | none |
| cpu-fibs | 41150300 | 47083900 | 2901144 | 2.0875477767910238 | 1.741 | True | None | none |
| cpu-string-build | 27363050 | 60475100 | 19724305 | 2.982434341200999 | 1.633 | True | None | none |
| cpu-hashmap | 29409550 | 45016000 | 6828677 | 2.3999443058362218 | 1.445 | True | None | none |
| tensor-create | 39185350 | 45192600 | 3934386 | 14.49842399761613 | 1.004 | True | None | inconclusive |
| tensor-elementwise | 28525350 | 31351500 | 1815248 | 2.0735708488384557 | 0.912 | True | None | none |
| tensor-reduce | 27052300 | 29857800 | 1350187 | 1.6160283555780357 | 0.849 | True | None | none |
| tensor-matmul | 25900200 | 30235000 | 1639079 | 7.459089119002941 | 1.139 | True | None | none |
| ml-mlp-step | 26786050 | 29973800 | 1381727 | 2.3155750679241476 | 1.552 | True | None | none |
| async-echo | 29199450 | 33703700 | 2451371 | 4.0856933949098355 | 1.179 | True | False | reference_parity_failure |
| async-pipeline | 28278250 | 34277200 | 2730763 | 2.3434832100170744 | 1.579 | True | None | none |
| sort-int | 55564150 | 87446500 | 12717778 | 5.6627159778382286 | 1.878 | True | None | none |
| binary-search | 61346900 | 65042800 | 3257313 | 2.087130727062003 | 1.628 | True | None | none |
| sieve | 38671050 | 62904700 | 8887254 | 2.8206060316882824 | 1.823 | True | None | none |
| matrix-transpose | 41792800 | 48866100 | 3248268 | 1.1836231645546473 | 1.558 | True | None | none |
| string-reverse | 57872950 | 60496200 | 2265250 | 0.5187239443595988 | 1.68 | True | None | none |
| count-primes | 84997550 | 126261700 | 22023227 | 1.8566407035598649 | 1.383 | True | None | none |
| gcd | 284222150 | 290076200 | 3894409 | 0.4735881812632881 | 1.205 | True | None | none |
| pow-fast | 40398950 | 55193000 | 8287392 | 6.629846198602084 | 1.613 | True | None | none |
| word-count | 64846750 | 67941700 | 1665186 | 0.6926742646327182 | 2.19 | True | None | none |
| digit-sum | 47633050 | 55836500 | 3387558 | 0.29166456218327524 | 1.885 | True | None | none |

## Failures

- tensor-create: measurement is inconclusive (dispersion > 10%)
- async-echo: Go reference parity is outside the certified window
- async-echo: Go reference parity is outside the certified window

## IR evidence

| Scenario | O0 SHA-256 | O0 blocks | O0 allocas | O0 host calls | O3 SHA-256 | O3 blocks | O3 allocas | O3 host calls |
|---|---|---:|---:|---:|---|---:|---:|---:|
| cpu-loop-sum | `10cda48ba57ef3f4e29ff8d842aed039446a26c8e1c8ae0b539954a879f37407` | 0 | 8 | 0 | `3ad5a987b7965175a765fc6b2a49c7d918ad22df2d2a785daf454b366c7bad15` | 0 | 8 | 0 |
| cpu-fibs | `34e97567badd55e45deaec46c5959a35f8272ff9e0eadce875e343a9c8a973c1` | 0 | 10 | 0 | `e7897c0d6b12b8040d38a7c8a80580c457ad3479d8a7fbf035c04517a450c309` | 0 | 10 | 0 |
| cpu-string-build | `844d4c12488043760cbba8bafc90395b39a76c6587f27297e8faf1e3b25664c3` | 0 | 6 | 16 | `200713800a402b7abd79035a9f3cb0199780c2d0d6b07cb842ee0a3be9960cc3` | 0 | 6 | 16 |
| cpu-hashmap | `334c93f2182d435ea49e3fc80d0fa33cb5a17577859319bfd3ecb4724cc85708` | 0 | 10 | 20 | `d165a439c3b09e362bb967e8abbc33e2c4b92cbb6dfc04b2f55988e09bc8d6c0` | 0 | 10 | 20 |
| tensor-create | `807a00b5519066d16c6b1ea051435efe584e95d7056ad10be689c1ae3d4b50c8` | 0 | 4 | 16 | `cc7d48c1901aaed5d07d829e733482ae7ab6c8277349f425477034a33d984170` | 0 | 4 | 16 |
| tensor-elementwise | `16ff0bcc7cf92ca857b62daa0577ff22fc4da3ea68d74be83daff54c4c8871e4` | 0 | 0 | 16 | `ac929660ebf72e5da28f0ea1adc3c17899ba928eb5e0c90b18d220cb821b6414` | 0 | 0 | 16 |
| tensor-reduce | `55865bdc174ab2570016f43ef3df8c1dfe19014b74990f9681d40d1a5ee90b5f` | 0 | 0 | 12 | `c5d8f07acaf31c72f85eee43783f6172200b19a03e5ce5311fe229e8ac5aa400` | 0 | 0 | 12 |
| tensor-matmul | `391567a617f14ae484c838c431bb0808564af87d7423b90a4c6c8f1a0c60db80` | 0 | 0 | 28 | `9a2597f6caca3bbc4fc08ffde70f1d1192ce09eb60fc4013fd29e6804bedb47e` | 0 | 0 | 28 |
| ml-mlp-step | `81e7d3b0b9e507eeeb81f5ce288ef30e1956dd53f7e3935bf359ba85c9301f16` | 0 | 2 | 76 | `06ed89e80eef8de68e610a6f32d56f19e6a1a9037236fc2df581e7f9fa39f8d2` | 0 | 2 | 76 |
| async-echo | `0add86c4a9a2b66a0ed6c03b14ae14a8ca267bce926f5f5846bccbedcb75078d` | 0 | 4 | 12 | `71027456714fd7dec6188cedfbea0e5d96cc4cf9bfccc999d9b9be7e5467a9a6` | 0 | 4 | 12 |
| async-pipeline | `e6f7f1beced26ec4a2b353d48ef2bf9e8138e94ed1f8ad9c233757f91d3750b1` | 0 | 10 | 20 | `8b7784f22ffb4affd0da4bd38e66c0f9a2e00c5ca6cfa01349c0894de8bf634b` | 0 | 10 | 20 |
| sort-int | `b265a7f19f07f2574893ce149c5686188ad30c4607efd01512b86f576a634293` | 0 | 14 | 0 | `aeba79b2e27da9ffa145664a90006e073423bf19cb07d2fb2f3ba540e3c75cb5` | 0 | 14 | 0 |
| binary-search | `1fd102f90c5487b09aa3dfc3371ebe71c06bce02058cb4bc55bbdf2086641733` | 0 | 18 | 0 | `bc88a445c7929fd6960a214642d683145dae8a0b4780fd70b9719491c4409706` | 0 | 18 | 0 |
| sieve | `dba8a00db685886f644b4a63bafba4076c98f8d4ada770021368fd6ecf43b972` | 0 | 16 | 0 | `5af8656c0f5662697b4ab35be09b0713032bd6cc7d0a0584cb4b9e6b182acc90` | 0 | 16 | 0 |
| matrix-transpose | `8cda28fa4baf227c0872901837ace8b5089eaee1b83c36650f73bff0dc713998` | 0 | 12 | 0 | `2576ed681c30b196201214751e6e655a8302889a9af66f38b4a72b667de4f270` | 0 | 12 | 0 |
| string-reverse | `5723dd42303fe7a574a59c427d3a15e9d11ae234e4c0c83e7f4f2b5d4c937aa7` | 0 | 14 | 4 | `20ec933ea3462c958f10dbde758bd9cb67c4f70d7a25c2a6eb479310f2174a3e` | 0 | 14 | 4 |
| count-primes | `69e05f926bf49b41ecb2382fb00f9098c342b50c94272b502f99f9e99de7d63e` | 0 | 12 | 0 | `1ba46f857f90327f575da852c9ab2ccf89bb7d81a63652a96c439aca9206a004` | 0 | 12 | 0 |
| gcd | `f7d94d2538c3d05524c05872a7b782b3f72e5830308637f16ef0efc06ac6cd6e` | 0 | 16 | 0 | `4e2bf9fe0834df23ec3d2595218632c8d47e9fdacd5a3371d76550af492d99ad` | 0 | 16 | 0 |
| pow-fast | `e3228dc9080740e7daca275d52b584f47b7c1e9ad8ee08e5005eda75634bab6d` | 0 | 18 | 0 | `49a7032470fa6ea8b991db0d7f5f60f994887532135ca4011200dce3c9e9932e` | 0 | 18 | 0 |
| word-count | `2cc779360c3a30c51e3590f95b3d1dfd38e06cef879826a2439da0e87df7dbe9` | 0 | 12 | 8 | `0a82d495467afd6f8d7c332dbbce913172f1c33136f6737ac3ebceaec0df8357` | 0 | 12 | 8 |
| digit-sum | `10fb9367abc71a602f507ae603263d80aaac7df49dbbc300d8e4bcaf7078cb7d` | 0 | 12 | 0 | `ed00892acb1a7741b1383a202d09b084253c5bf00853d818a7ac564e482b49e9` | 0 | 12 | 0 |

Tracked textual snapshots:

- `cpu-string-build` O0 `844d4c12488043760cbba8bafc90395b39a76c6587f27297e8faf1e3b25664c3`, O3 `200713800a402b7abd79035a9f3cb0199780c2d0d6b07cb842ee0a3be9960cc3`
- `tensor-create` O0 `807a00b5519066d16c6b1ea051435efe584e95d7056ad10be689c1ae3d4b50c8`, O3 `cc7d48c1901aaed5d07d829e733482ae7ab6c8277349f425477034a33d984170`
- `cpu-hashmap` O0 `334c93f2182d435ea49e3fc80d0fa33cb5a17577859319bfd3ecb4724cc85708`, O3 `d165a439c3b09e362bb967e8abbc33e2c4b92cbb6dfc04b2f55988e09bc8d6c0`
- `tensor-matmul` O0 `391567a617f14ae484c838c431bb0808564af87d7423b90a4c6c8f1a0c60db80`, O3 `9a2597f6caca3bbc4fc08ffde70f1d1192ce09eb60fc4013fd29e6804bedb47e`
- `ml-mlp-step` O0 `81e7d3b0b9e507eeeb81f5ce288ef30e1956dd53f7e3935bf359ba85c9301f16`, O3 `06ed89e80eef8de68e610a6f32d56f19e6a1a9037236fc2df581e7f9fa39f8d2`

## R-3104–R-3117 coverage

| ID | Roadmap status | Matrix row | Metric | Rejection risk | Rollback |
|---|---|---|---|---|---|
| R-3104 | not_started | True | ns/iter e tempo de lowering; lookup count | Mudança de ordem/ABI, picos em módulos grandes | Reverter se qualquer cenário correto exceder +5% ou se o IR mudar sem ganho |
| R-3105 | not_started | True | host calls/iter, alocações e ns/iter | Reordenação de efeitos ou lifetime de handles | Reverter se hostcall count não cair ou surgir divergência numérica/async |
| R-3106 | not_started | True | allocas/função e bytes alocados | Alias/lifetime incorreto e regressão numérica | Reverter se allocas não reduzirem ou sanitizer/fixtures falharem |
| R-3107 | complete | True | allocations, bytes e mediana `tensor-create` | Reuso de shape/dtype/layout incompatível | Reverter qualquer mudança que altere contagem ativa, bytes ou tolerância R-1503 |
| R-3108 | complete | True | ns/iter e bytes/cópias por string | Quebra de string cross-module ou ownership | Reverter se R-109 ou o cenário de string regredir >5% |
| R-3109 | not_started | True | ns/step e graph nodes/step | Misturar inference e training ou perder gradientes | Reverter se training output mudar ou graph nodes não caírem |
| R-3110 | not_started | True | elementos/s e erro numérico | CPU dispatch não determinístico, NaN/rounding | Reverter se erro exceder R-1503 ou fallback não for funcional |
| R-3111 | not_started | True | GFLOP/s, mediana e erro relativo | Tiling ruim para shapes pequenos, overflow/tolerância | Reverter se qualquer shape perder >5% ou erro exceder R-1503 |
| R-3112 | not_started | True | images/s, mediana e erro máximo | Im2col aumenta memória e piora batch pequeno | Reverter se memória ou latência exceder baseline dedicado |
| R-3113 | not_started | True | task creation ns e throughput | Ordem, fairness ou starvation | Reverter se task accounting, resultado ou parity mudar |
| R-3114 | not_started | True | allocs/task e ns/task | Use-after-free ou cancelamento incorreto | Reverter se allocs não caírem ou cancel/error tests falharem |
| R-3115 | not_started | True | tamanho IR e ns/iter | Code size explosion ou overflow de avaliação | Reverter se IR crescer sem ganho ou diagnostics mudarem |
| R-3116 | not_started | True | instruções/blocks e ns/iter | Remover efeitos observáveis/host calls | Reverter se qualquer fixture mudar saída ou host-call count esperado |
| R-3117 | complete | True | ns/iter, code size e compile time | Compile time/code size ou regressão de cold path | Reverter se ganho não superar ruído ou compile time exceder limite |

Baseline and IR hashes are generated from the current working tree; no profiler causal claim is made.

