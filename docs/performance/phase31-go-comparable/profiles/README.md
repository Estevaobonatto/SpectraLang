# R-3102 Profiling Evidence

This directory contains real, checked-in profiling evidence for the eight
Phase 31 CPU and tensor scenarios. The official capture backend is Linux
`perf` under WSL2, with `cargo-flamegraph`/FlameGraph scripts, Go, Rust and
Graphviz available in the same environment. Windows `xperf` output is useful
diagnostically but does not satisfy the R-3102 acceptance gate.

## Capture

From a working Linux/WSL2 checkout:

```bash
python3 scripts/phase31_profile.py preflight --backend perf
python3 scripts/phase31_profile.py capture \
  --spectra-binary target/release/spectralang \
  --profile release --backend perf
python3 scripts/phase31_profile.py validate
```

The capture writes deterministic per-scenario directories containing the
Spectra flamegraph, `perf report` summary, optimized and unoptimized IR,
pipeline output, reference summaries, and `metadata.json`. The metadata records
the exact command, binary, profile, revision, host, and tool versions.

The script never reads or writes `docs/performance/phase31-go-comparable/baseline.json`.
Routine correctness remains the fast `--code-validation` path; full benchmark
certification remains the separate five-attempt statistical path.

## Interpretation

Expected symbols include JIT/codegen, hostcall dispatch, runtime registry,
tensor allocation/materialization, and process startup. JIT symbol gaps must be
reported as unresolved rather than replaced with invented attribution.

The evidence is not complete until all eight scenarios validate and
`bottleneck-analysis.md` is backed by the committed reports and IR snapshots.
