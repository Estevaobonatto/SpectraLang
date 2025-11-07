# SpectraLang Runtime Memory Strategy

SpectraLang's alpha runtime uses a **hybrid memory model** that combines a tracing heap for managed objects with an explicit manual allocator for deterministic lifetimes. This approach keeps deterministic performance available where it matters (tight loops, host interop) while still offering ergonomic GC semantics for the majority of language-level values.

## Strategy Overview

- The runtime exposes a single entry point, `HybridMemory`, which owns both allocators.
- Traced allocations (`HybridMemory::allocate_traced`) are subject to mark-and-sweep collection.
- Manual allocations (`HybridMemory::allocate_manual`) return `ManualBox<T>` handles that release statistics and memory on drop.
- The compiler-generated code is expected to place user-visible Spectra values on the traced heap and rely on the manual allocator for short-lived or host-managed buffers.
- Runtime statistics (`MemoryStats`) and configuration (`MemoryConfig`) are centrally tracked so the CLI can surface diagnostics and regression baselines.

## Traced Heap

The traced heap is a classic **mark-and-sweep collector**:

1. Code requests traced storage through `HybridMemory::allocate_traced(value)` and receives a `Gc<T>` handle.
2. `Gc<T>` handles do **not** keep objects alive by themselves. They are lightweight references that may dangle after a collection.
3. To keep an allocation reachable, the frontend or runtime must create a `GcRoot<T>` (`Gc::into_root`) or embed the `Gc<T>` in another traced object that implements `Trace`.
4. The collector walks the graph by calling the `Trace` implementations supplied by user data structures. A blanket set of implementations already covers common containers (`Option`, `Vec`, fixed-size arrays, etc.).
5. When the retained byte count crosses `MemoryConfig::traced_collection_threshold_bytes`, the runtime automatically triggers a collection. Manual collections are always available through `HybridMemory::collect_garbage()`.
6. Survivors are unmarked for the next cycle, while unreachable slots are returned to a freelist for reuse.

This design keeps the tracing interface explicit: Spectra values that own other GC handles must implement `Trace` and call `trace` on their fields. The compiler already derives this behaviour for generated AST/runtime structures.

### Example

```rust
use spectra_runtime::memory::{HybridMemory, Trace, TraceContext, Gc};

#[derive(Default)]
struct Node {
    value: i32,
    next: Option<Gc<Node>>,
}

impl Trace for Node {
    fn trace(&self, ctx: &mut dyn TraceContext) {
        self.next.trace(ctx);
    }
}

let memory = HybridMemory::default();
let parent = memory.allocate_traced(Node::default());
let child = memory.allocate_traced(Node::default());
parent.borrow_mut().next = Some(child.clone());
let _root = parent.into_root(); // keeps parent and child alive
```

## Manual Heap

Manual allocations are targeted at host interop (FFI buffers, pinned memory) and deterministic lifetimes. `HybridMemory::allocate_manual` returns a `ManualBox<T>` that owns the object and updates runtime statistics automatically.

- Manual allocations respect a soft limit (`MemoryConfig::manual_soft_limit_bytes`). Exceeding the limit returns an `AllocationError` so hosts can decide whether to fail fast or spill to an external allocator.
- Dropping `ManualBox<T>` or extracting the value with `into_inner()` decrements the live allocation counters, keeping telemetry accurate.
- Manual allocations are transparent to the GC and therefore never participate in tracing.

## Configuration

`MemoryConfig` currently exposes two high-impact knobs:

| Field | Purpose | Default |
| --- | --- | --- |
| `traced_collection_threshold_bytes` | Soft threshold that triggers an automatic GC cycle when the traced heap reaches this size. Set to `0` to disable automatic runs. | `4 MiB` |
| `manual_soft_limit_bytes` | Budget for manually tracked allocations. Set to `0` for "no limit". | `32 MiB` |

The runtime initialisation API allows custom configurations:

```rust
use spectra_runtime::{initialize_with_config, MemoryConfig};

let state = initialize_with_config(MemoryConfig {
    traced_collection_threshold_bytes: 8 * 1024 * 1024,
    manual_soft_limit_bytes: 128 * 1024 * 1024,
});

println!("GC stats: {:?}", state.memory_stats());
```

This configuration is stored in `RuntimeState` and can be surfaced by the CLI to match project-level tuning.

## Compiler & Codegen Implications

- Codegen must emit `Trace` implementations for every GC-managed struct so the collector can walk nested references.
- Borrowed handles (`Gc<T>`) should be short-lived; long-lived references must be promoted to roots during lifetimes that span collections.
- Lowering must map Spectra ownership semantics to either the traced or manual allocator, but nothing in the runtime enforces a particular policy—those rules live in the compiler's semantic analysis.
- Diagnostics and optimisation passes can rely on `MemoryStats` to assert invariants or surface warnings (e.g., manual heap pressure). These metrics are accessible without mutating the runtime.

With this foundation in place, subsequent alpha milestones (runtime allocation APIs and standard library scaffolding) can build on a concrete, documented memory substrate.
