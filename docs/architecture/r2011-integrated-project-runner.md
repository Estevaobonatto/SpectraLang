# R-2011 Integrated Project Runner

`scripts/validate_r2011_integrated_project_runner.py` is the Phase 20 runner
for the R-2008 integrated project matrix.

Inputs:

- `docs/architecture/r2008-language-feature-project-matrix.toml`
- `roadmap/roadmap.toml`
- `target/debug/spectralang.exe` through `--binary`

Behavior:

- verifies every matrix-required project file before execution
- runs each exact matrix command with the configured `spectralang` binary
- supports `spectralang run`, `spectralang package check`, and
  `spectralang package test`
- classifies failures as `compile`, `semantic`, `lowering`, `backend`,
  `runtime`, `package`, `missing-file`, `fixture`, `expectation`, or `timeout`
- writes `target/r2011-integrated-project-runner/report.json`

Report schema: `spectralang.r2011_integrated_project_runner.v1`.

Each project result records:

- project name and id
- project path and entrypoint
- command, exact matrix command, and concrete executed command
- category, roadmap item, and owner
- elapsed time, status, failure class, exit code, output tail, and expected
  outcome

Validation command:

```powershell
python scripts\validate_r2011_integrated_project_runner.py --binary target\debug\spectralang.exe
```

`run_tests.ps1` includes the `phase20-integrated-project-runner` gate and runs
`validate_r2011_integrated_project_runner.py`.
