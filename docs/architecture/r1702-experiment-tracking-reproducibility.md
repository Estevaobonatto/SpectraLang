# R-1702 Experiment Tracking and Reproducibility

Status: complete for the current production baseline.

## Contract

R-1702 adds runtime experiment tracking through `std.ml`. A Spectra training
program can create a run, record config, metrics, artifacts, seed, lockfile, and
model output, then emit a structured manifest.

Public API:

| API | Purpose |
|---|---|
| `ml.experiment_start(name, out_dir, seed)` | Creates an experiment handle and deterministic manifest path |
| `ml.experiment_set_config(exp, key, value)` | Records string config values |
| `ml.experiment_log_metric(exp, name, value, step)` | Records numeric metrics |
| `ml.experiment_log_artifact(exp, path)` | Records artifact path, size, and FNV-1a 64-bit content hash |
| `ml.experiment_set_lockfile(exp, path)` | Records package lockfile path, size, and hash |
| `ml.experiment_set_model_output(exp, path)` | Records model output path, size, and hash |
| `ml.experiment_finish(exp)` | Writes `experiment-manifest.json` |
| `ml.experiment_manifest_path(exp)` | Returns the manifest path |
| `ml.experiment_repro_command(exp)` | Returns the documented reproduction command |
| `ml.experiment_compare_manifests(a, b)` | Compares configs, metrics, artifacts, lockfile, model output, and seed |

## Manifest Schema

Manifest schema identifier:

```text
spectra.ml.experiment.v1
```

Top-level fields:

- `schema`
- `name`
- `seed`
- `configs`
- `metrics`
- `artifacts`
- `lockfile`
- `model_output`
- `manifest_path`
- `reproduction_command`

Configs are sorted by key/value before emission so manifests are stable across
equivalent runs. Metrics preserve logging order. Artifacts include `path`,
`size`, and `fnv64`.

The current reproduction command is intentionally explicit:

```powershell
spectralang run <training.spectra> --package-lock spectra.lock --experiment-manifest <manifest>
```

The CLI does not yet replay from a manifest automatically. The command is the
documented operator workflow tying the source program, package lockfile, and
experiment manifest together.

## Validation

Required gate:

```powershell
python scripts\validate_r1702_experiment_tracking.py
```

The script runs:

- `cargo test -p spectra-runtime ml_phase17_experiment_tracking_manifests_compare_and_repro_command`
- `cargo run -p spectra-cli -- run tests/validation/93_ml_phase17_experiment_tracking.spectra`
- `cargo run -p spectra-cli -- run examples/ai/experiment_tracking_reproducibility.spectra`

The script also parses `target/ai-examples/experiment-run/experiment-manifest.json`
and checks schema, seed, metrics, artifacts, lockfile, model output, and
reproduction command.
