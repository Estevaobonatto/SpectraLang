# R-3103 Benchmark + IR Evidence

- Status: `passed`
- Schema: `spectra.phase31.r3103_evidence.v1`
- Revision: `f7ba1dbb3295084342fc002c7816eadf096adafb`
- Profile: `release`
- Classification: `benchmark_and_ir_hypothesis`
- Profiling causal claim: `False`
- Baseline modified: `False`

The report is benchmark/IR evidence only. Linux perf/flamegraph attribution remains R-3102.

## Report hashes

| Report | SHA-256 |
|---|---|
| `target\phase31\r3103-release-run-1.json` | `5881f1e28f3639f211485706937c725f72d4144f102a33ee06cd3abe8e999dfb` |
| `target\phase31\r3103-release-run-2.json` | `c564bda5c1cd6d1f4466c6e3c9fe544417a2a793715751f3245298ceb2d2b6a6` |

## Scenario evidence

| Scenario | Median ns | P95 ns | Stddev ns | Dispersion % | Gap to Go | Correctness | Reference parity | Failure class |
|---|---:|---:|---:|---:|---:|---|---|---|
| cpu-loop-sum | 42930200 | 72037000 | 13396404 | 8.350382091471044 | 1.523 | True | None | none |
| cpu-fibs | 43080850 | 60758100 | 9867219 | 0.6355724179072604 | 1.784 | True | None | none |
| cpu-string-build | 28349900 | 37493600 | 5928256 | 1.2566287711773234 | 1.456 | True | None | none |
| cpu-hashmap | 28461600 | 32855300 | 2278610 | 0.8232168693882355 | 1.485 | True | None | none |
| tensor-create | 54052200 | 83497500 | 18684981 | 8.673930808125908 | 0.869 | True | None | none |
| tensor-elementwise | 31227500 | 79503000 | 18708976 | 7.300363373698084 | 1.04 | True | None | none |
| tensor-reduce | 37196250 | 49851900 | 6444885 | 3.82295622124445 | 0.914 | True | None | none |
| tensor-matmul | 30491200 | 51733900 | 11356418 | 2.9463606731375194 | 1.343 | True | None | none |
| ml-mlp-step | 29105050 | 31474000 | 1284813 | 7.050054513657683 | 1.685 | True | None | none |
| async-echo | 32435200 | 69912100 | 23447784 | 3.5427683504340965 | 1.121851 | True | True | none |
| async-pipeline | 37567550 | 61503900 | 24959906 | 3.0043298052826595 | 1.596 | True | None | none |
| sort-int | 53389650 | 66014700 | 5932708 | 1.9152326340404928 | 1.813 | True | None | none |
| binary-search | 63155900 | 66077400 | 11931570 | 1.151716028668941 | 1.54 | True | None | none |
| sieve | 40445000 | 60259300 | 8517584 | 0.9409998649245179 | 1.778 | True | None | none |
| matrix-transpose | 43657000 | 53322300 | 4824583 | 5.196554043703837 | 1.4 | True | None | none |
| string-reverse | 61435100 | 68447100 | 3341596 | 5.944025483803233 | 1.613 | True | None | none |
| count-primes | 70264950 | 81570100 | 20591456 | 2.4058288461850275 | 1.272 | True | None | none |
| gcd | 294512050 | 338664900 | 18252896 | 0.6738362657826734 | 1.177 | True | None | none |
| pow-fast | 33724600 | 42402800 | 3869468 | 3.868710521924437 | 1.639 | True | None | none |
| word-count | 84368500 | 119086300 | 15015759 | 1.697815631890534 | 2.31 | True | None | none |
| digit-sum | 46586100 | 49155500 | 1032971 | 4.342864717809358 | 1.621 | True | None | none |

## Failures

- none

## IR evidence

| Scenario | O0 SHA-256 | O0 blocks | O0 allocas | O0 host calls | O3 SHA-256 | O3 blocks | O3 allocas | O3 host calls |
|---|---|---:|---:|---:|---|---:|---:|---:|
| cpu-loop-sum | `1ceb339dda4e51a4b41a66da7ad328f45d994c9e9b8e1e3d3866eb13b46c3fd3` | 0 | 8 | 0 | `4ffe05aa3f0a7fd080f9ef95b38e885a14aaf37398d3603ceb208b261ead423d` | 0 | 8 | 0 |
| cpu-fibs | `800863d3040a1af5769afc1177f6dc899bc79ebb18c95ff2d0df60ea75e9d2c5` | 0 | 10 | 0 | `0ccb845e62794c49ca564da5f3c88f3b6888fff8007292a9f75bbc9030925927` | 0 | 10 | 0 |
| cpu-string-build | `0df769f82b8d92f06f94868e6ae269e886e6a0ec2321cc3ceea9ad6dd7b1dc84` | 0 | 6 | 16 | `d37910f45efcd05bddd60b42522fa26a07b44383205e22d91cc7ed501183ce26` | 0 | 6 | 16 |
| cpu-hashmap | `64903f15efd8869ac46cc6bef2d419e4c7ce9b0b06869b2011ce23978f046a14` | 0 | 10 | 20 | `0aebaddf645f3c3bf48d6e33c49c1ddf16ea4caec990b637af4272b821ca847b` | 0 | 10 | 20 |
| tensor-create | `fffcdf2ab419dfbc4c2d8b2b42093cb96723562fd22e00bc8f54417e16317a00` | 0 | 4 | 16 | `a7d357eaccd730f0bb44200f6f72a88b3f363e5d8c1ba1f07c7de6dac112f024` | 0 | 4 | 16 |
| tensor-elementwise | `fc9fe7e3fc7063d499a655b265e71ee1408c064c3ef30db4db8958f3a0b8ce79` | 0 | 0 | 16 | `97d90a0a8e52cf70007f70e312aa493a9f08f0a150825d0eec24139c985b8811` | 0 | 0 | 16 |
| tensor-reduce | `29795e3552227bdb50b852779e22e8013efb69039178c6d86c2d36a8c052881c` | 0 | 0 | 12 | `ce47171a2597fcf66111c1b4755f2262f0be58a403c316547a659f2e7932fa83` | 0 | 0 | 12 |
| tensor-matmul | `bc3df603648ad230bdce8368ba100682499938e7b83143d2b8cb38f920329ee5` | 0 | 0 | 28 | `cc502d862c00ef6cf787ff89867e346cb49f0f9f617b8498bb44f4c7cffec8ef` | 0 | 0 | 28 |
| ml-mlp-step | `7e68553c377667035fc67f84320cec32a1b3467f39e190caee5f19bac766bd97` | 0 | 2 | 76 | `d495c83672404d1899d5bde48bbbcd1987d0b28c8ea43c335cf83aeb9a4f2c86` | 0 | 2 | 76 |
| async-echo | `aef98a9361119ab3413d780f171b3b7098544e65b2ff4adac940b0f6a8c4c805` | 0 | 4 | 12 | `dddb25ccc87bc7dd56a7da7bf245493f90d5c47913e56ad6bcd1f3d969fabebf` | 0 | 4 | 12 |
| async-pipeline | `dcc7fabfa377a287b64a674c1fa842df9b52f738e08a6fd34d6d969bfb018db2` | 0 | 10 | 20 | `a9f18af843a41b5f33058543622349c70b76386d51a0bf62fb2fb0089858458f` | 0 | 10 | 20 |
| sort-int | `153102faffe0e4cf4b12643dc65fa63550efb31e2802ee778a0156d702a0a7bb` | 0 | 14 | 0 | `45854ee08f373ec9f580ff1b54b940bcb36d94f8df15b564a87d30538c8b86af` | 0 | 14 | 0 |
| binary-search | `6532823e6c144512e7ad814d480ff9acb4817b2fcfae7012b0dffbf0483e3feb` | 0 | 18 | 0 | `bdb620fc447327da91024bd955740ad64b14dc78c2bf6981af7d45e8c1826242` | 0 | 18 | 0 |
| sieve | `aeb57b46457cd1f798d30d8cfb3174e08d428541f828e44a7330a104ecb0879b` | 0 | 16 | 0 | `296b0ae05dfd34e2a7a4428b068b3e63fe86c14a8f042d880f6ec11db47978b3` | 0 | 16 | 0 |
| matrix-transpose | `fc5b42ed8d03f1ed8557be34685b0efde9c819e103ff4e1f40c4d7a966fcb2f9` | 0 | 12 | 0 | `936fdf94785e0af1e18791d4a8e4f9c606cb837531014140639501131892c948` | 0 | 12 | 0 |
| string-reverse | `0237cf44a973f3c063f229175222c5348a7aec728dd7149cd2b665b00d0b1cb7` | 0 | 14 | 4 | `17af41e75d9feb8c3400dee55350bb65385d5c19475c5ed61ba6d1cadd2c7071` | 0 | 14 | 4 |
| count-primes | `6d8791785f12be6b4ddc928751e65e87e6dc4c63f56aca5ace4ec7d0ba1c0631` | 0 | 12 | 0 | `8022bf83f1081a7df3d66dd9300c2b8440afd82779b23434295a56602369a53a` | 0 | 12 | 0 |
| gcd | `c31c69e4c7d49c7116f3f032de66642c088bfbabe131e6541178681a03a2d93d` | 0 | 16 | 0 | `723f81a4d996fcd913dc6cc80f5465c3c49eaa880a0cc8a663bdf221a359ce55` | 0 | 16 | 0 |
| pow-fast | `56a5f0d6cb4b5042fb4f147f564ba0591729a5f174204fbf56ed123ab68bd76b` | 0 | 18 | 0 | `c006e7d2b468690c34b521a0b8503fe059404a74986209ef31087584199c60d9` | 0 | 18 | 0 |
| word-count | `7626cb5721df8c90674f83fbb8507bc3563695c73f340d3dedf9afa63edfe494` | 0 | 12 | 8 | `328e3ca4dc470e7818621dcb8c668c04e1d89e0f5445489b06e38beea67edb27` | 0 | 12 | 8 |
| digit-sum | `3619239862997bdd68fe314dec10892a81ebeed994c9152343116d64185952e3` | 0 | 12 | 0 | `312bddbfc01f2c3d3fe3f0c9b78a653fb465015f224130294d2f77538c734bc8` | 0 | 12 | 0 |

Tracked textual snapshots:

- `cpu-string-build` O0 `0df769f82b8d92f06f94868e6ae269e886e6a0ec2321cc3ceea9ad6dd7b1dc84`, O3 `643df65659fa87ea033491cc6ad0c513ca4189f0350fd62eb2a6aef1a410ba4c`
- `tensor-create` O0 `fffcdf2ab419dfbc4c2d8b2b42093cb96723562fd22e00bc8f54417e16317a00`, O3 `a7d357eaccd730f0bb44200f6f72a88b3f363e5d8c1ba1f07c7de6dac112f024`
- `cpu-hashmap` O0 `c4d31573b6d124043684e86c0779425d22e064bb5cc9fabcb16c42c09759478d`, O3 `364955b5a798fbac9876a59f3c6cd516479265d8e5a3acc0cf9c84ec3dfef2c9`
- `tensor-matmul` O0 `bc3df603648ad230bdce8368ba100682499938e7b83143d2b8cb38f920329ee5`, O3 `cc502d862c00ef6cf787ff89867e346cb49f0f9f617b8498bb44f4c7cffec8ef`
- `ml-mlp-step` O0 `7e68553c377667035fc67f84320cec32a1b3467f39e190caee5f19bac766bd97`, O3 `d495c83672404d1899d5bde48bbbcd1987d0b28c8ea43c335cf83aeb9a4f2c86`

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

