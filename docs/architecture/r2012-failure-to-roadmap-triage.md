# R-2012 Failure-To-Roadmap Triage

`scripts/validate_r2012_failure_triage.py` is the Phase 20 triage gate for
integrated project failures found by the R-2011 runner.

Inputs:

- `target/r2011-integrated-project-runner/report.json`
- `roadmap/roadmap.toml`
- `docs/roadmap-backlog.md`
- `docs/production-ai-implementation-plan.md`
- `run_tests.ps1`

Behavior:

- validates R-2011 report schema and required project result fields
- verifies failure classes are one of `compile`, `semantic`, `lowering`,
  `backend`, `runtime`, `package`, `fixture`, `missing-file`, `expectation`, or
  `timeout`
- passes when the runner report has zero failed projects
- for every failed project, requires a tracking roadmap item outside `R-2008`
  through `R-2013`
- requires each tracking item to include owner, phase, dependencies, risk, and
  acceptance criteria
- requires tracking text to include project id, affected project path, exact
  command, and failure class
- requires matching backlog text for the tracking item

Report:

```powershell
target\r2012-failure-triage\report.json
```

Validation command:

```powershell
python scripts\validate_r2012_failure_triage.py --runner-report target\r2011-integrated-project-runner\report.json
```

`run_tests.ps1` includes the `phase20-failure-triage` gate and runs
`validate_r2012_failure_triage.py` after the R-2011 runner.
