# R-3102 Bottleneck Analysis

Status: `in_progress` — real Linux profiling is currently blocked because the
configured WSL2 distributions cannot attach their VHDX (`ERROR_PATH_NOT_FOUND`).
No synthetic flamegraphs, benchmark-only attribution, or invented top-five
ranking is accepted as evidence.

The following are the required analysis slots. They remain hypotheses until
the corresponding `perf report`, flamegraph, and IR artifacts exist:

| Rank | Candidate bottleneck | Scenarios | Evidence required | Follow-up |
|---:|---|---|---|---|
| 1 | String materialization and copying | `cpu-string-build` | flamegraph plus allocation/copy symbols | `R-3104`/`R-3105` |
| 2 | Tensor allocation and fill | `tensor-create` | allocator, registry and fill samples | `R-3106`/`R-3107` |
| 3 | Elementwise hostcall dispatch | `tensor-elementwise` | hostcall and kernel attribution | `R-3105`/`R-3110` |
| 4 | Matrix multiplication kernel | `tensor-matmul` | kernel call tree and IR comparison | `R-3111` |
| 5 | Loop/code-generation overhead | CPU scenarios | Cranelift/JIT and IR evidence | `R-3104`/`R-3115`/`R-3116` |

These candidates must be replaced or reordered after capture. `R-3102` may only
be promoted to `complete` once the final ranking names the exact top five
functions per affected scenario, estimates impact and risk, and maps every
claim to a checked-in artifact.
