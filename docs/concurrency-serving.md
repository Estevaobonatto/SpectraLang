# Concurrency and Serving Runtime

Spectra exposes the Phase 11 runtime baseline through two standard library
modules:

- `std.concurrent`: deterministic local concurrency primitives for data
  pipeline-style workloads.
- `std.serve`: local in-process inference serving primitives for batching,
  warmup, cancellation, and toy benchmarking.

Both modules are stdlib APIs. They do not add new language syntax.

## Imports

```spectra
import std.concurrent as concurrent;
import std.serve as serve;
```

## `std.concurrent`

### API

```spectra
concurrent.reset() -> unit

concurrent.task_spawn(value: int) -> int
concurrent.task_join(task: int) -> int
concurrent.task_is_done(task: int) -> bool

concurrent.channel_new() -> int
concurrent.channel_send(channel: int, value: int) -> bool
concurrent.channel_recv(channel: int) -> int
concurrent.channel_len(channel: int) -> int
concurrent.channel_close(channel: int) -> unit

concurrent.counter_new(value: int) -> int
concurrent.counter_add(counter: int, delta: int) -> int
concurrent.counter_get(counter: int) -> int

concurrent.pipeline_sum(start: int, count: int, workers: int) -> int

concurrent.stats_tasks_spawned() -> int
concurrent.stats_channels() -> int
```

### Contract

- Handles are positive integers managed by the runtime.
- `task_spawn(value)` starts a runtime task and stores the task under a handle.
- `task_join(task)` deterministically waits for the task and returns its value.
- Channels are FIFO and non-blocking.
- `channel_recv(channel)` returns `-1` when the channel has no pending value.
- `channel_send(channel, value)` returns `false` when the channel is closed.
- `pipeline_sum(start, count, workers)` splits the integer range into chunks and
  sums them in worker threads.

### Example

```spectra
module concurrency_example;

import std.concurrent as concurrent;

pub fn main() -> int {
    concurrent.reset();

    let task = concurrent.task_spawn(42);
    if concurrent.task_join(task) != 42 {
        return 1;
    }

    let channel = concurrent.channel_new();
    concurrent.channel_send(channel, 7);
    concurrent.channel_send(channel, 9);
    if concurrent.channel_recv(channel) != 7 {
        return 2;
    }
    if concurrent.channel_recv(channel) != 9 {
        return 3;
    }

    if concurrent.pipeline_sum(1, 100, 4) != 5050 {
        return 4;
    }

    return 0;
}
```

## `std.serve`

### API

```spectra
serve.reset() -> unit

serve.server_new(model: int) -> int
serve.server_warmup(server: int) -> bool
serve.server_is_warm(server: int) -> bool
serve.server_enqueue(server: int, input: int) -> int
serve.server_cancel(server: int, request: int) -> bool
serve.server_process_batch(server: int, max_batch: int) -> int
serve.server_result(server: int, request: int) -> int
serve.server_pending(server: int) -> int
serve.server_set_timeout(server: int, timeout: int) -> bool
serve.server_resident_model(server: int) -> int
serve.server_benchmark(server: int, requests: int, batch: int) -> int
```

### Contract

- The current serving baseline is local and in-process.
- A server handle owns a resident model identifier.
- The toy model computes `input * model`.
- `server_process_batch(server, max_batch)` processes at most `max_batch`
  pending requests and returns the processed count.
- A server must be warmed before it processes requests.
- Cancelled and pending requests return `-1` from `server_result`.
- `server_benchmark(server, requests, batch)` warms the server, enqueues
  deterministic inputs, processes them in batches, and returns the processed
  request count.

### Example

```spectra
module serving_example;

import std.serve as serve;

pub fn main() -> int {
    serve.reset();

    let server = serve.server_new(3);
    let request = serve.server_enqueue(server, 10);

    serve.server_warmup(server);
    if serve.server_process_batch(server, 1) != 1 {
        return 1;
    }
    if serve.server_result(server, request) != 30 {
        return 2;
    }
    if serve.server_benchmark(server, 8, 3) != 8 {
        return 3;
    }

    return 0;
}
```

## Current Limits

- There is no dedicated concurrency syntax yet.
- Channels are non-blocking and integer-only in this baseline.
- Serving does not include HTTP, gRPC, sockets, async I/O, or distributed model
  residency.
- The baseline is intended to support deterministic compiler/runtime tests and
  local ML serving design work.

## Validation

```powershell
cargo test -p spectra-runtime -p spectra-compiler -p spectra-midend -p spectra-cli -p spectra-interop -p spectra-lsp
cargo run -q -p spectra-cli -- run tests\validation\77_concurrency_pipeline.spectra
cargo run -q -p spectra-cli -- run tests\validation\78_serving_foundations.spectra
.\run_tests.ps1
```
