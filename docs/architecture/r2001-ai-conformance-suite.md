# R-2001 AI Conformance Suite

Status: complete for the current production baseline.

## Purpose

`R-2001` provides a release-candidate certification gate for SpectraLang AI/ML
users. The suite is intentionally executable: a candidate is certified only when
the required conformance gates pass and the versioned JSON report validates.

## Certification Command

```powershell
python scripts\validate_r2001_ai_conformance.py --keep-going --out target\r2001-conformance\conformance-report.json
```

The script exits with code `0` only when certification passes. Any failed,
timed-out, or missing gate rejects the release candidate.

## Report Contract

The emitted report uses:

- schema: `spectralang.ai_conformance_report.v1`
- conformance version: `R-2001/v1`
- default path: `target/r2001-conformance/conformance-report.json`

Required top-level fields:

- `schema`
- `conformance_version`
- `release_candidate`
- `candidate_status`
- `certified`
- `generated_at`
- `git_revision`
- `required_categories`
- `missing_categories`
- `categories`
- `gates`

`candidate_status` is `certified` only when every gate passes and every required
category is present.

## Required Categories

The suite covers the production AI surface required by the roadmap:

- compiler
- runtime
- tensors
- autodiff
- graph
- interop
- package
- serving
- tooling
- docs_examples

## Runner Integration

`run_tests.ps1` includes the `phase20-conformance` gate and invokes:

```powershell
python scripts\validate_r2001_ai_conformance.py --keep-going
```

This makes the conformance suite part of the normal repository validation path.

## Release Candidate Rule

Release candidates must not be certified from partial evidence. A candidate is
rejected when:

- any command exits non-zero
- any command times out
- any required category has zero gates
- the report schema or conformance version does not match the documented
  contract
- the report validation finds inconsistent status fields
