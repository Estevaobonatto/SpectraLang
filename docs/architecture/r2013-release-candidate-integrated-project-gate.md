# R-2013 Release Candidate Integrated Project Gate

## Purpose

R-2013 is the final Phase 20 certification gate for the checked-in language
and AI Support projects. It combines the R-2001 conformance report, the R-2011
integrated project runner, and the R-2012 failure triage report in one fresh
release-candidate validation.

R-2013 does not replace those validators. It executes them in order, consumes
their reports, and rejects certification unless every required project passes.

## Dependencies and truth owner

The gate depends on:

- R-2001 AI conformance;
- R-2011 integrated project runner;
- R-2012 failure-to-roadmap triage;
- R-2014 multi-module aggregate and trait codegen recovery;
- R-2015 `std.time` production surface.

The R-2008 matrix remains the source of truth for the required project set,
commands, entrypoints, required files, expected outcomes, and matrix version.
R-2013 consumes the matrix; it does not redefine or mutate the project set.

## Execution

Run the complete gate with the repository-built CLI:

```powershell
python scripts\validate_r2013_release_candidate.py `
  --binary target\debug\spectralang.exe `
  --release-candidate local-working-tree
```

The default report is:

```text
target/r2013-release-candidate/report.json
```

Execution order:

```text
R-2001 conformance
    -> R-2011 matrix project execution
    -> R-2012 failure triage
    -> R-2013 aggregate certification report
```

The aggregate validator regenerates all predecessor reports. It never uses an
old report as a substitute for the current run.

## Fail-closed policy

Certification fails when any of these conditions occurs:

- matrix schema, version, command, project directory, entrypoint, or required
  file is invalid;
- a command is not one of `spectralang run`, `spectralang package check`, or
  `spectralang package test`;
- R-2001 is not certified;
- an R-2011 project is missing, fails, times out, or has invalid report data;
- R-2012 reports an untracked failure;
- the report set does not refer to the current matrix version;
- the final report cannot preserve project command and failure evidence.

Tracked failures are still release blockers. R-2012 tracking prevents loss of
ownership and reproduction context; it does not make a failing project pass.

## Report contract

Schema:

```text
spectralang.r2013_release_candidate_gate.v1
```

The report records:

- release-candidate name and Git revision;
- matrix path, schema, and version;
- predecessor report paths;
- project count, passed count, failed count, and untracked failures;
- each project's ID, path, category, exact command, entrypoint, status, exit
  code, failure class, and output tail;
- follow-up roadmap items returned by R-2012;
- validation errors when certification is rejected.

Successful report summary:

```json
{
  "status": "passed",
  "summary": {
    "conformance_certified": true,
    "project_count": 8,
    "projects_passed": 8,
    "projects_failed": 0,
    "untracked_failures": 0
  },
  "follow_up_roadmap_items": []
}
```

Rejected report summary:

```json
{
  "status": "failed",
  "summary": {
    "conformance_certified": true,
    "project_count": 8,
    "projects_passed": 7,
    "projects_failed": 1,
    "untracked_failures": 0
  },
  "validation_errors": [
    "release candidate requires zero failed projects and zero untracked failures"
  ]
}
```

## Failure handling

When a project fails:

1. R-2011 preserves command, path, output tail, exit code, and failure class.
2. R-2012 verifies that the failure is tracked with owner, phase,
   dependencies, risk, reproduction command, affected project, and acceptance.
3. R-2013 rejects certification regardless of whether the failure is tracked.
4. The implementation must fix the failure with regression coverage or keep the
   new roadmap item open; R-2013 cannot be marked complete while the project
   fails.

## `run_tests.ps1` integration

The official full-suite path invokes `validate_r2013_release_candidate.py` once.
The individual R-2001, R-2011, and R-2012 scripts remain available for focused
diagnosis but are not executed a second time by the release-candidate section.

The final gate records `phase20-release-candidate` and
`validate_r2013_release_candidate` in the aggregate test summary.

## Completion gate

R-2013 is complete only when the full validator and `run_tests.ps1` pass, all
matrix projects pass through their declared normal CLI/package commands, the
final report exists at `target/r2013-release-candidate/report.json`, and the
roadmap/backlog/strategic plan describe the same evidence.
