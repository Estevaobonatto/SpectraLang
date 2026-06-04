# SpectraLang Testing And Regression Policy

This policy is the R-104 compiler test pyramid contract. It defines which tests
must exist, where new regressions belong, and how fuzz findings become stable
tests.

## Test Pyramid

| Layer | Location | Purpose |
|-------|----------|---------|
| Lexer/parser unit tests | `compiler/src/**` | Local syntax and token behavior |
| Frontend integration tests | `compiler/tests/` | Parser, semantic, diagnostic, and pipeline behavior |
| AST snapshots | `compiler/tests/snapshots/` | Stable parser output summaries for representative syntax |
| Diagnostic snapshots | `compiler/tests/snapshots/` | Stable error-code/message/hint output |
| Midend unit/integration tests | `midend/src/**`, `midend/tests/` | Lowering, verification, and optimization behavior |
| IR snapshots | `midend/tests/snapshots/` | Stable lowered IR summaries |
| Backend tests | `backend/src/**` | Cranelift codegen and verifier-sensitive cases |
| CLI tests | `tools/spectra-cli/tests/`, `run_tests.ps1` | User-facing command behavior |
| Language validation tests | `tests/validation/` | Positive `.spectra` programs that must pass |
| Negative tests | `tests/errors/` | Invalid `.spectra` programs that must fail |
| Fuzz targets | `fuzz/fuzz_targets/` | Panic, recovery, and malformed-input discovery |

## Regression Placement

- Parser bugs get a parser unit test and, when syntax shape matters, an AST
  snapshot.
- Semantic bugs get a focused `compiler/tests/` test and, when user output
  matters, a diagnostic snapshot.
- Lowering or verifier bugs get a `midend/tests/` case and an IR snapshot when
  the emitted shape is part of the contract.
- Backend verifier or `Value not found` bugs get a backend test or a minimal
  `.spectra` validation file that reaches Cranelift.
- CLI behavior bugs get a CLI integration test and a `run_tests.ps1` case when
  they affect the shipped command surface.
- Full-language examples belong in `tests/validation/` only when they are
  expected to pass indefinitely.
- Deliberately invalid programs belong in `tests/errors/`, not validation.

## Snapshot Rules

- Snapshots are canonical summaries, not raw debug dumps.
- Snapshot changes must be reviewed as behavior changes.
- Do not update snapshots to hide a regression.
- If syntax or IR evolves intentionally, update the snapshot and mention the
  reason in the commit/PR text.

## Fuzz Workflow

Fuzz targets are under `fuzz/` and use `cargo-fuzz`.

1. Run the relevant target:

   ```powershell
   cargo fuzz run parser
   ```

2. Minimize crashes:

   ```powershell
   cargo fuzz tmin parser artifacts\parser\crash-...
   ```

3. Convert every valid crash into a checked-in regression test.
4. Keep fuzz targets small: no JIT execution, no network, no unbounded input.

## Completion Gate

R-104 is complete only when:

- compiler, midend, backend, and CLI have stage-local tests;
- AST, IR, and diagnostic snapshots exist and are run by `cargo test`;
- fuzz targets exist for parser, semantic, pipeline, and lowering;
- this policy is checked in;
- `run_tests.ps1` validates the test pyramid structure.
