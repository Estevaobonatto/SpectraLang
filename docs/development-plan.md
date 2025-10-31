# SpectraLang Development Plan

## Vision and Core Principles
- Build SpectraLang as a performant, multi-paradigm language implemented in Rust to leverage memory safety, concurrency, and rich tooling.
- Maintain a modular compiler architecture (workspace crates: `frontend`, `midend`, `backend`, `runtime`, `tools`) to avoid oversized files and enable parallel development.
- Prioritise clean, intuitive syntax with minimal English keywords while preserving expressiveness and metaprogramming hooks.
- Provide an incremental JIT compilation pipeline with precise span-based diagnostics and error aggregation per phase.
- Deliver production-grade documentation and supporting tooling from the earliest iterations.

## Language Feature Blueprint

### Paradigms and Semantics
- **Object-Oriented**: classes, interfaces, traits, single inheritance with mixin-style traits, encapsulation modifiers (`pub`, `protected`, `private`), polymorphic dispatch.
- **Procedural**: free functions, modules, namespace support, pointer/reference interop for systems-level control.
- **Functional**: first-class functions, closures, immutable data emphasis, pattern matching, higher-order utilities.

### Type System
- Strong static typing by default with optional scoped `@weak` directive blocks enabling dynamic checks for interop.
- Parametric polymorphism (generics) in Phase 2, with monomorphisation in the backend for performance.
- Type inference for locals and closures, explicit annotations for public APIs.
- Nullability handled via `Option<T>` style enums; no implicit nulls.

### Memory Model
- Hybrid GC with tri-colour incremental collector in runtime crate.
- Opt-in manual control via `@manual` regions exposing `retain`/`release` semantics and stack allocation primitives.
- Deterministic destructors (`drop` trait) for resource management when leaving scope.

### Control Flow Coverage (>=80% of requested tags)
- **Conditionals**: `if/elif/else`, `if/else if/else`, `switch/case`, `match/case`, `cond`, `unless` (syntactic sugar over negated if).
- **Loops**: `for`, `while`, `do while`, `foreach`, `for in`, `for of`, `loop`, `repeat until`.
- **Flow Control**: `break`, `continue`, `return`, `goto` (restricted to labelled blocks), `yield` for generators.

### Core Data Structures
- **Primitives**: `int`, `float`, `double`, `char`, `string`, `bool`, `byte`.
- **Composite Collections**: `array`, `list`, `vector`, `slice`, `ArrayList`, linked lists, `dict`, `hash`, `map`, `HashMap`, `TreeMap`, `object` literals, `associative array`, `set`, `HashSet`, `TreeSet`, `unordered_set`, `tuple`.
- **Structured Types**: `struct`, `record`, `dataclass`, `class`, `enum`, algebraic data types with pattern matching.
- **Specialised Structures** (Phase 2/3): `queue`, `deque`, `stack`, `heap`, priority queue, tree and graph modules in standard library.

### Metaprogramming and Modules
- Hygienic macro system with staged evaluation; macros defined in separate module scope with explicit imports.
- Module system with namespace hierarchy (`module core.math;`) and visibility controls; integrated package manager resolves modules.
- Reflection API limited to type descriptors and attribute metadata to prevent runtime abuse.

## Syntax and Language Design Strategy
- Define concise keyword set: `fn`, `class`, `trait`, `module`, `let`, `mut`, `match`, `async`, `await`, etc.
- Statement termination via newline sensitivity with optional semicolons for clarity.
- Block syntax uses braces `{ ... }`, indentation-agnostic to accommodate tooling.
- Attribute-style directives (`@weak`, `@manual`, `@test`) to toggle compilation behaviours.
- Metaprogramming via `macro` blocks producing AST fragments with typed parameters.

## Compiler Architecture

### Workspace Layout
- `frontend/` crate: lexer, AST, parser, semantic analyzer (mirrors existing guide but split into submodules per phase to keep files <500 lines).
- `midend/` crate: SSA-based IR, optimisation passes (constant folding, inlining, dead code elimination, loop optimisations).
- `backend/` crate: target-specific code generators (x86-64, AArch64, WASM) plus shared lowering pipeline to Cranelift.
- `runtime/` crate: GC, scheduler, runtime services, FFI bridges.
- `tools/spectra-cli/`: CLI driver, REPL, formatting tool entry point.

### Frontend Strategy
- **Lexer**: deterministic DFA, zero-copy slicing, interned identifiers, keyword trie.
- **Parser**: Pratt parser for expressions, LR-style for declarations, generates AST with spans.
- **Semantic**: multi-pass (registration, type checking, flow analysis), collects diagnostics, builds symbol tables, manages generics instantiation queue.

### Middle-End Strategy
- Build Spectra Intermediate Representation (SIR) with SSA form, explicit control-flow graph, type annotations.
- Optimisation pipeline configured via pass manager; each pass isolated in dedicated module (`midend/src/passes/<pass>.rs`).
- Generic specialisation translated to monomorphic SIR prior to backend lowering.

### Backend Strategy
- Use Cranelift for JIT code emission; plug-in architecture for additional targets.
- Target adapters: `backend/src/targets/x86.rs`, `arm.rs`, `wasm.rs` implement trait `TargetBackend`.
- Embed runtime intrinsics (GC safepoints, allocation) via ABI descriptors generated during lowering.

### Runtime and Standard Library
- Minimal but complete standard library: `core` (primitives, traits), `collections`, `io`, `concurrency` package.
- GC runtime with incremental collector, write barriers, object headers defined in `runtime/src/gc/`.
- Manual memory API exported via `spectra::mem` module.

## Tooling Roadmap
- **IDE Support**: Language Server Protocol implementation in Phase 2 providing syntax highlighting, completion, diagnostics.
- **Debugger**: Extend CLI to expose LLDB/GDB integration with SIR-to-native mapping.
- **Package Manager**: `spectra-pm` tool for dependency resolution (semantic versioning, lockfile, registry sync).
- **Formatter**: `spectra fmt` using AST rewrite rules; ensures canonical style.

## Development Phases and Milestones

### Phase 1 (Months 0-3): Compiler Prototype
- Bootstrap workspace, scaffolding crates, continuous integration.
- Implement lexer, parser for core syntax, AST definitions, initial semantic checks (name resolution, basic types).
- Basic REPL and CLI driver supporting single-module compilation.
- Deliver minimal runtime stubs and GC interface.

### Phase 2 (Months 3-7): Advanced Features
- Expand language constructs (pattern matching, macros, majority of control flow tags, collections).
- Implement generics, trait system, type inference completion, module imports/exports.
- Introduce SIR representation, baseline optimisation passes, Cranelift JIT for x86-64 and WASM.
- Begin standard library build-out (collections, IO, async primitives).
- Release initial LSP with syntax highlighting and completion.

### Phase 3 (Months 7-9): Optimisation and Polishing
- Optimise JIT (inlining, escape analysis, GC tuning), add ARM backend.
- Integrate benchmarking harness, ensure performance target within 15% of reference (Rust/C++ baselines).
- Tighten diagnostics, implement deterministic destructors, manual memory regions.
- Harden package manager, formatter, and debugger hooks.

### Phase 4 (Months 9-10): Documentation and Samples
- Produce full specification, API reference, tutorials, best-practice guides.
- Publish sample projects showcasing paradigms and tooling.
- Prepare community onboarding materials, contribution guidelines, and governance docs.

## Quality Gates and Metrics
- Enforce `cargo fmt`, `cargo clippy -D warnings`, full `cargo test` on every merge.
- Track compilation latency benchmarks; regressions >5% block release.
- POSIX compliance checklist (filesystem, process, signals) validated on Linux/macOS/WSL.
- Developer experience metric: onboarding surveys, maintain <2-week learning curve.

## Testing Strategy
- Unit tests per module (lexer, parser, semantic, IR passes, runtime GC components).
- Integration suites compiling curated SpectraLang programs with golden outputs.
- Cross-platform CI matrix: Windows, Linux, macOS for all toolchains.
- Continuous benchmarking harness using Criterion to monitor runtime performance.
- Property-based tests (proptest/quickcheck) for parser and optimiser correctness.

## Documentation Deliverables
- **Language Specification**: formal grammar, type rules, memory model.
- **Programmer Guide**: tutorials, idioms, paradigm coverage, interoperability tips.
- **API Reference**: standard library modules with examples.
- **Tooling Manuals**: CLI, package manager, formatter, LSP usage.
- Documentation versioned alongside releases, published to static site and PDF.

## Ecosystem and Community Strategy
- Establish open governance model, code of conduct, contributor guide.
- Central package registry (`registry.spectra.dev`) with semantic versioning enforced.
- Public issue tracker and community forum (Discourse/Discord) for support.
- Monthly release cadence post-MVP, with long-term support branches.
- Encourage third-party tooling via SDKs and plugin interfaces.

## Risk Management and Dependencies
- Dependencies: Cranelift, LLVM (optional), Wasmtime for WASM runtime, Rust stable toolchain.
- Identify risks: GC complexity, JIT security (sandboxing), cross-platform ABI differences.
- Mitigation: phased prototypes, fuzzing, security review prior to public beta.

## Next Actions
1. Finalise language reference outline and keyword list.
2. Scaffold Rust workspace with crate layout and CI pipeline.
3. Implement lexer spec and token inventory aligned with feature set.
4. Draft documentation structure and publish contributor guidelines.
