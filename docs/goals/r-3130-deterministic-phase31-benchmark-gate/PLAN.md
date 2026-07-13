# R-3130 — Deterministic Phase 31 Benchmark Gate

## Objective

Certify the Phase 31 cross-language benchmark gate with one contract for the
21 current scenarios, explicit binary/profile metadata, reproducible sampling,
fail-closed validation, and reviewed baseline changes only.

## Execution order

1. Centralize the 21-scenario and measurement contract.
2. Harden runner diagnostics, independent attempts, confirmations, caching,
   and subprocess timeouts.
3. Harden validator against missing/extra scenarios, metadata drift, noise,
   functional failures, and confirmed regressions.
4. Convert baseline tooling to candidate generation plus explicit reviewed
   application.
5. Keep `run_tests.ps1` timeout forwarding explicit and test it.
6. Run two complete reports on a quiescent reference host and compare their
   semantic fields.
7. Update documentation and roadmap; complete only after all acceptance gates
   pass.

## Required evidence

- `target/phase31/cross-lang-report.json`
- `target/phase31/cross-lang-report.md`
- two independent stable reports
- comparison of scenario IDs, commands, correctness, exit codes, failure
  classes, profile, binary, revision, and measurement policy
- unit tests covering contract, noise, regression, timeout, and baseline
  mutation protection

## Non-goals

- no compiler/runtime optimization without a controlled reproduction;
- no automatic baseline mutation;
- no release certification while Phase 31 is inconclusive or failing.
