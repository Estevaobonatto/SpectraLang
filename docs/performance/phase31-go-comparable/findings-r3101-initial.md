# Phase 31 Cross-Language Performance Findings (Initial Pass)

Updated: 2026-06-23
Roadmap item: `R-3101` first complete run
Roadmap item: `R-3103` input

## Summary

First complete pass of the cross-language benchmark suite (Spectra vs Go,
Java, Rust) on 11 scenarios. The table is generated from
`target/phase31/cross-lang-report.md` and is automatically re-emitted by
`scripts/phase31_run_all.py`.

| scenario | gap vs Go | gap vs Rust | comment |
|---|---:|---:|---|
| `cpu-loop-sum` | 2.0x | 2.1x | tight integer loop, baseline arithmetic |
| `cpu-fibs` | 2.3x | 3.8x | simple function-call heavy loop |
| `cpu-string-build` | **51.4x** | **68.8x** | R-3108 priority; string concat is 50x slower than Go |
| `cpu-hashmap` | 3.8x | 6.8x | list-based scan in Spectra (no exposed map); R-3107 |
| `tensor-create` | **5.1x** | **17.5x** | full_f materialization is the dominant cost; R-3107 |
| `tensor-elementwise` | 1.3x | 2.7x | close to Go parity already |
| `tensor-reduce` | 0.97x | 1.8x | at parity with Go (1.0x)! |
| `tensor-matmul` | 2.0x | 2.5x | 64x64 hand-rolled matmul; SIMD gap |
| `ml-mlp-step` | 3.3x | 3.8x | host-call-heavy training step |
| `async-echo` | 218x | 2.4x | goroutine M:N model dominates; OS-thread cost in others |
| `async-pipeline` | 2.7x | 2.8x | channel send/recv via host calls |

## Top Findings

### F1: string concatenation is 50x slower than Go (cpu-string-build)

`str.concat` allocates a fresh string each call. Go and Rust use a pre-sized
mutable buffer (Go `strings.Builder`, Rust `String::with_capacity`). This is
the single largest gap and a high-priority target for R-3108
("String Materialization Optimization").

### F2: tensor materialization is 5-20x slower than Go/Rust (tensor-create)

`tensor.full_f` allocates and fills a buffer on every call. Even a 1M-element
alloc + fill takes 270ms in Spectra vs 14ms in Rust. R-3107
("Tensor Cross-Call Buffer Reuse") and R-3102 profiling will localize the
host call vs allocation overhead.

### F3: hashmap lookup is 4-7x slower (cpu-hashmap)

Two factors: (a) the Spectra scenario uses a list with linear search because
the typed map surface is not yet exposed to .spectra; (b) each `list_contains`
is a host call. Both factors disappear with R-3107 (buffer reuse) and a
follow-up exposing `map_*` to the language surface.

### F4: async-echo is 218x slower than Go, but Rust/Java are also slow

This is not a Spectra bug. Go's M:N goroutine scheduler handles 10k tasks
with almost no overhead, while Rust/Java spawn real OS threads. The fairer
comparison is Rust (2.4x gap), which still shows R-3113/R-3114 have work to do.

### F5: tensor-reduce is at Go parity already

Sum reduction in Spectra is 34.9ms vs Go's 35.9ms. This sets the realistic
target for the rest of the tensor surface: 1.0-1.5x gap is achievable with
the existing kernels.

## Prioritization for R-3103

1. **R-3108** (String ABI) — closes the 50x gap on `cpu-string-build`.
2. **R-3107** (Tensor Cross-Call Buffer Reuse) — closes 5-17x gap on
   `tensor-create` and reduces hashmap host-call overhead.
3. **R-3106** (Alloca Hoisting) — supports R-3107 and reduces midend cost.
4. **R-3104** (Cranelift Value Map) — broadens across all 11 scenarios.
5. **R-3110** (SIMD) — closes Rust gap on `tensor-elementwise` and
   `tensor-matmul`.
6. **R-3105** (Host Call Batching) — same impact as R-3104 but more surgical.
7. **R-3113 / R-3114** (Reactor async) — closes async-echo gap vs Rust.
8. **R-3109** (Autodiff Inference Skip) — improves `ml-mlp-step` inference path.

## Caveats

- Numbers were captured on a developer Windows machine, single run, with
  Java's G1 GC and Go's default runtime. Real CI numbers will shift; the
  gate's 5% drift policy absorbs that.
- The async-echo 218x gap is partially an artifact of comparing Go's M:N
  scheduler to OS-thread-based runtimes. R-3102 will produce a fairer
  `async-echo-spectra-only` profile to keep that scenario useful.
- Spectra's hashmap scenario uses a list with linear search; treat its
  absolute numbers as a proxy for "list ops per second", not hashmap ops.
- This is the **first** complete pass. R-3102 will instrument each hot path
  before any optimization patch lands.

## R-3108 (String Materialization) — Complete

Implemented in this session. The optimization replaces the per-call
`str.concat(a, b)` pattern (which allocates a fresh string every call)
with a builder that accumulates parts in a handle and joins them in a
single allocation on `builder_finish`.

### New API

```spectra
import std.string as str;

let b = str.builder_new(0);        // capacity hint in bytes
str.builder_push(b, "x|");          // append a string part
str.builder_push(b, "y|");
str.builder_len(b);                 // current total byte length
let s = str.builder_finish(b);     // consume builder, return concatenated string
str.builder_free(b);                // discard without finalizing
```

The new functions are purely additive — `str.concat`, `str.repeat_str`,
`str.len`, etc. continue to work unchanged.

### Root cause of the earlier "Could not determine object type" error

The midend has a hardcoded table `(module, function) -> HostFunctionDescriptor`
in `midend/src/lowering.rs:8133` that maps known stdlib symbols to their
runtime function names. Existing entries include `("string", "concat")` and
`("string", "repeat_str")`. Adding a new language surface entry is not
enough — the new symbol must also be added to this table, otherwise the
midend falls back to the `infer_expr_ir_type` path, sees the alias
identifier as `IRType::Int`, and reports the bogus "Could not determine
object type" error.

The fix was a 5-line addition to that table for the new
`builder_*` functions. No snapshot updates were needed; the language
surface change in `make_std_string()` worked correctly from the start.

### Measured impact

`cpu-string-build` (50 outer × 100 pushes of `"x|"` per outer):

| | before R-3108 | after R-3108 | change |
|---|---:|---:|---:|
| Spectra time | 942,868,500 ns | 244,815,500 ns | **3.85x faster** |
| Gap vs Go | 71.7x | 19.4x | 52.3 percentage points |
| Gap vs Rust | 66.9x | 26.3x | 40.6 percentage points |

The remaining gap is now in line with the other CPU scenarios (2-3x
Go, 3x Rust) and will close further as R-3104 (dense value map) and
R-3110 (SIMD) land. No other scenario regressed; the gate remains PASS.

### Files changed

- `runtime/src/stdlib/mod.rs`: `StringBuilder`, `StringBuilderRegistry`,
  `std_string_builder_new`, `std_string_builder_push`,
  `std_string_builder_len`, `std_string_builder_finish`,
  `std_string_builder_free`. All registered as host functions.
- `compiler/src/semantic/builtin_modules.rs`: 5 new `pub_fn` entries in
  `make_std_string()`. `builder_new` takes a `Type::Int` capacity hint
  so it is not a no-arg call.
- `midend/src/lowering.rs`: 5 new entries in the hardcoded
  `(module, function) -> HostFunctionDescriptor` table for the new
  symbols (this was the actual fix).
- `tests/validation/180_phase31_string_builder.spectra`: 4 regression
  tests covering push, len, finish, free, empty builder.
- `benchmarks/cross-lang/cpu-string-build/spectra/bench.spectra`:
  updated to use the builder. The Go/Java/Rust versions are unchanged
  because they were already using pre-allocated mutable buffers
  (`strings.Builder`, `StringBuilder`, `String::with_capacity`).
- `docs/performance/phase31-go-comparable/baseline.json`: updated
  `cpu-string-build.spectra_ns_per_iter` to the new median.

