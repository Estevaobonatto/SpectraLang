# R-1703 Distributed Training Foundations

Status: complete for the current production baseline.

## Contract

R-1703 adds deterministic single-machine distributed-training simulation through
`std.ml`. The goal is to validate orchestration semantics, worker progress,
checkpoint coordination, interruption recording, and resume without claiming
networked cluster execution.

Supported topology:

- one local process
- N simulated workers
- synchronous global step advancement
- JSON checkpoint files written by the runtime
- resume from checkpoint into a new session handle

Public API:

| API | Purpose |
|---|---|
| `ml.distributed_session_start(name, out_dir, worker_count, seed)` | Creates a deterministic simulated-worker session |
| `ml.distributed_worker_step(session, worker_id, samples, loss)` | Records one local worker step and returns that worker step count |
| `ml.distributed_global_step(session)` | Advances the global step only when all workers have progressed past it |
| `ml.distributed_worker_step_count(session, worker_id)` | Returns a worker's local step count |
| `ml.distributed_checkpoint_save(session, path, interrupted_worker)` | Writes checkpoint JSON and optionally marks one worker interrupted |
| `ml.distributed_resume(path)` | Loads checkpoint JSON, reactivates workers, and returns a new session handle |
| `ml.distributed_summary(session)` | Returns JSON summary of topology, global step, worker steps, samples, and checkpoint |

`interrupted_worker` uses `-1` for no interrupted worker. Any non-negative value
must reference an existing worker.

## Checkpoint Schema

Checkpoint schema identifier:

```text
spectra.ml.distributed_checkpoint.v1
```

Top-level fields:

- `schema`
- `name`
- `topology`
- `seed`
- `worker_count`
- `global_step`
- `interrupted_worker`
- `last_checkpoint_path`
- `workers`

Each worker records:

- `worker_id`
- `step_count`
- `sample_count`
- `accumulator`
- `active`

The checkpoint is the source used by `ml.distributed_resume`; resume does not
depend on the original in-memory session.

## Non-Goals

R-1703 intentionally does not implement:

- multi-process networking
- GPU collectives
- NCCL/MPI/gRPC integration
- distributed tensor sharding
- fault-tolerant object storage
- elastic worker membership

Those belong in later accelerator, serving, or distributed-runtime phases.

## Validation

Required gate:

```powershell
python scripts\validate_r1703_distributed_training.py
```

The script runs:

- `cargo test -p spectra-runtime ml_phase17_distributed_training_checkpoint_resume`
- `cargo run -p spectra-cli -- run tests/validation/94_ml_phase17_distributed_training.spectra`
- `cargo run -p spectra-cli -- run examples/ai/distributed_training_checkpoint.spectra`

The script also parses `target/ai-examples/distributed-run/checkpoint.json` and
checks schema, topology, seed, worker count, global step, interrupted worker,
and worker activity state.
