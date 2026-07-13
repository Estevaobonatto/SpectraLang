# R-2013 Release Candidate Integrated Project Gate

## Objective

Implement and validate the fail-closed release-candidate aggregator for
R-2001, R-2011, and R-2012, preserving predecessor reports and producing the
versioned R-2013 certification report.

## Execution scope

1. Validate the R-2008 matrix and the explicit compiler binary.
2. Re-run R-2001 conformance, R-2011 integrated projects, and R-2012 triage
   in that order.
3. Aggregate project evidence, tracked follow-ups, and certification status.
4. Integrate one R-2013 invocation into `run_tests.ps1`.
5. Add unit, directed, reproduction, and documentation evidence.
6. Keep R-2013 `in_progress` until the complete acceptance gate is clean.

## Completion gate

R-2013 may become `complete` only when the generated report is passed with all
eight projects passed and zero untracked failures, the full `run_tests.ps1`
execution is clean, and the roadmap/backlog/strategic plan agree with the
checked-in evidence.
