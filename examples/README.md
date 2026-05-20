# Examples Status

This directory currently mixes three categories of `.spectra` files:

- `valid`: examples that are expected to compile with `spectralang compile`
- `negative`: examples that are intentionally invalid and should fail fast
- `draft`: examples that document work-in-progress language areas and are not part of the stable compile set yet

## Valid examples confirmed in this stabilization round

- `test_beta_imports.spectra`
- `test_beta_closures.spectra`
- `test_if_let_correctness.spectra`
- `test_import_demo.spectra`
- `test_lint_warnings.spectra`
- `test_stdlib_improvements.spectra`
- `test_direct.spectra`
- `test_oop_encapsulation.spectra`

## Negative examples

- `bad_semantic_test.spectra`
- `test_errors_demo.spectra`
- `test_exhaustiveness_fail.spectra`
- `test_literal_non_exhaustive.spectra`
- `test_match_basic.spectra`

## Draft / currently blocked by advanced backend work

- `test_pattern_matching.spectra`
- `test_oop_dyn_trait.spectra`
- `test_oop_casting.spectra`
- `test_oop_drop.spectra`

## Special cases

- `test_multi_a.spectra` and `test_multi_b.spectra` are a multi-file example and should be validated together.
- examples that use experimental syntax must be compiled with explicit feature flags:
  - `switch`
  - `unless`
  - `do-while`
  - `loop`
