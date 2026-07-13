# R-3132 controlled evidence — 2026-07-13

## Scope

This evidence covers the conservative `task_spawn`/`task_join` fusion and the
direct `concurrent.reset()` Fast ABI. The official baseline remains
`docs/performance/phase31-go-comparable/baseline.json` at 33,865,050 ns for
`async-echo`.

## Implementation evidence

- `midend/src/passes/concurrent_spawn_join_fusion.rs` fuses only a single-use
  handle in one basic block and allows only pure instructions in the gap.
- `backend/src/codegen.rs` and `backend/src/aot.rs` emit direct imports for the
  fused spawn/join and reset paths.
- `runtime/src/stdlib/mod.rs` preserves generic dispatch, increments task
  statistics exactly once, and does not allocate a visible slot for a fused
  pair.
- `tests/validation/182_concurrent_spawn_join_fusion.spectra` proves the
  fused result and accounting; 183 proves observed-handle fallback; 184 proves
  reset semantics.

## Reproduction

```powershell
cargo build -p spectra-cli --bin spectralang
python scripts\diagnose_async_echo.py --binary target\debug\spectralang.exe --profile debug --out target\phase31\async-echo-diagnostics\r3132-debug-v3.json
```

The generated diagnostic report records the exact binary, profile, revision,
commands, medians, p95, standard deviation, and operation accounting.

## Result

The focused debug diagnostic produced:

| Variant | Median |
|---|---:|
| startup | 36.06 ms |
| reset-only | 37.54 ms |
| spawn+join | 39.30 ms |
| fused | 38.93 ms |
| full | 38.94 ms |

The fused/full result is approximately 15% above the historical
process-inclusive baseline. This is a real implementation improvement over
the approximately 40.7 ms pre-fusion focused result. The user accepted the
38.9 ms result as the R-3132 criterion; the ≤1% aspiration is deferred and the
baseline remains unchanged.

## Status

R-3132 is `complete` with the accepted measured result and passing regression
coverage. Further reduction toward ≤1% is future optimization work. R-3131,
R-3130, and R-2013 remain open.
