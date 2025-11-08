# SpectraLang Alpha Checklist

## Language Surface

- [x] Freeze the alpha language reference (grammar, keywords, standard library surface) aligned with the target feature matrix _(see `docs/language-reference-alpha.md`)_
- [x] Identify and document unsupported constructs slated for post-alpha milestones _(captured under Deferred Features in `docs/language-reference-alpha.md`)_
- [x] Define the module/package system semantics and file layout conventions _(see “Module and Package Semantics” in `docs/language-reference-alpha.md`)_
- [x] Ensure all planned control-flow forms (foreach, for-of, repeat-until, yield, goto) have syntax and parsing stubs or confirmed deferral
- [x] Document the weak typing mode directives and expected behavior _(see “Weak Typing Mode Status” in `docs/language-reference-alpha.md`)_

## Type System & Semantics

- [x] Implement generic type parameter resolution and substitution in the semantic analyzer _(generic scopes now tracked in `SemanticAnalyzer` and type parameters resolve to `Type::TypeParameter`)_
- [x] Enforce trait bounds and method resolution for generic/parametric types _(trait-bound lookup now validates method receivers and arguments for type parameters and concrete types)_
- [x] Complete exhaustiveness checks for enums, tuples, and pattern combinations _(semantic analyzer now reasons about enum payload guards, tuple-of-bool combinations, and requires catch-alls when coverage cannot be proven)_
- [x] Specify and validate coercion, conversion, and visibility rules _(numeric expressions support int→float promotion with diagnostics for invalid conversions, and public APIs now fail compilation if they expose private user-defined types in signatures)_
- [x] Add type inference coverage tests for complex expressions and method calls _(new semantic fixtures cover numeric promotion, trait-bound dispatch, and diagnostic failures in `tests/semantic/type_inference_complex_expressions.spectra` and `tests/semantic/type_inference_method_errors.spectra`)_

## Frontend Robustness

- [x] Audit lexer and parser feature coverage versus the planned syntax (traits with inheritance, impl blocks, pattern ergonomics) _(see `docs/frontend/parser-coverage-audit.md`)_
- [x] Improve error recovery to continue after common syntax mistakes _(Parser now skips malformed call/method argument lists via `recover_in_delimited_list` and synthetic symbol insertion)_
- [x] Attach detailed span information and contextual hints to diagnostics _(CLI diagnostics now highlight source spans with contextual notes and actionable hints)_
- [x] Support incremental or module-aware parsing to prepare for multi-file projects _(parser workspace cache now reuses module ASTs and CLI keeps a persistent pipeline for multi-file runs; see `compiler/src/parser/workspace.rs`)_
- [x] Verify CLI and compiler flags gate experimental syntax behind feature toggles when needed _(feature-gated constructs enforced via `require_feature` with CLI flag plumbing and parser unit tests)_

## Midend & Backend


- [x] Finish lowering for all AST constructs (struct/enum literals, pattern bindings, method dispatch) _(lowering now materializes nested generic enums/structs, preserves pattern scopes, and passes `tests/validation/57_generic_enum_inference.spectra`; see `midend/src/lowering.rs`)_
- [x] Lower host-call instructions end-to-end, interning names for the JIT and trapping on runtime failures _(backend code now materializes argument/result buffers, dispatches `spectra_rt_host_invoke`, and raises `TrapCode::User(0)` on non-zero statuses; see `backend/src/codegen.rs`)_

## Runtime & Memory Model


- [x] Define the SpectraLang memory strategy (hybrid GC/manual) and initial collector interface _(documented in `docs/runtime/memory-strategy.md`; runtime exposes `HybridMemory`, `Gc`, and `ManualBox` APIs)_
- [x] Wire runtime allocation APIs used by generated code across platforms _(JIT imports `spectra_rt_manual_alloc` and clears allocations via `spectra_rt_manual_clear`; see `backend/src/codegen.rs` and `runtime/src/ffi.rs`)_
- [x] Deliver a minimal standard library (math, collections, I/O) backed by runtime support _(runtime host implementations in `runtime/src/stdlib/mod.rs`; documented in `docs/runtime/standard-library.md`)_
- [x] Establish FFI or host-call conventions for JITed functions interacting with the runtime _(runtime host registry available in `runtime/src/ffi.rs`; conventions documented in `docs/runtime/host-call-conventions.md`)_
- [x] Create conformance tests ensuring runtime initialization and teardown semantics _(runtime FFI tests cover frame cleanup and manual clear behavior; see tests in `runtime/src/ffi.rs`)_

## CLI & Tooling

- [x] Extend `spectra` CLI with module resolution, multi-file project handling, and dependency scanning _(CLI planner now discovers modules, validates imports, and compiles in dependency order via `tools/spectra-cli/src/project.rs`)_
- [x] Implement subcommands for `check`, `run`, `repl`, and project scaffolding where applicable _(CLI now offers `compile`/`check`/`run`, interactive `repl`, and `new` project scaffolding)_
- [x] Surface pipeline summaries (frontend, lowering, passes, codegen timings) behind flags _(`--summary` option now prints per-module pipeline metrics with aggregated timings in `tools/spectra-cli`)_
- [ ] Plan formatting, linting, and VS Code syntax/highlighting tooling integration
- [ ] Define exit codes and logging conventions for automation compatibility

## Quality Gates


- [ ] Write unit tests per compiler stage (lexer, parser, semantic, lowering, codegen)
- [ ] Build integration suites covering examples, diagnostics, and JIT execution paths
- [ ] Introduce regression tests for previously fixed bugs and edge cases
- [ ] Integrate fuzzing or property testing for parser and semantic phases
- [ ] Establish CI pipelines across Windows, macOS, and Linux with performance benchmarks

## Documentation & Ecosystem


- [ ] Draft the SpectraLang book covering language concepts, tutorials, and patterns
- [ ] Produce an API/reference manual for standard library and runtime facilities
- [ ] Document contribution guidelines, coding standards, and release process
- [ ] Outline the package registry vision and semantic versioning policy
- [ ] Set up community channels (forum, chat, issue templates) to support early adopters
