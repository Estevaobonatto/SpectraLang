# 8. Benchmarks And Comparisons

Phase 13 adoption benchmarks are reproducibility checks, not performance claims.
They prove that the AI examples execute end-to-end and emit machine-readable
timing data on the local machine.

## Run The AI Example Benchmark

```powershell
python scripts\ai_examples_benchmark.py --out target\ai-examples\benchmark.json
```

The JSON report contains one record per `examples/ai/*.spectra` program:

```json
{
  "example": "linear_regression_train_export.spectra",
  "status": "passed",
  "elapsed_ms": 1234
}
```

`status` must be `passed` for every example before R-1302 can be considered
complete.

## What To Compare

Use the benchmark report for:

- detecting regressions where an example stops running;
- detecting large timing changes during runtime work;
- documenting the exact command used for local adoption validation.

Do not use Phase 13 benchmark numbers as numerical-kernel performance claims.
Kernel performance is tracked by the Phase 4 benchmark harness.

## CI/Gated Validation

`run_tests.ps1` executes all AI examples directly. The benchmark script is a
separate adoption artifact so users can collect timing data without changing the
main regression runner.
