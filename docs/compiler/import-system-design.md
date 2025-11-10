# Spectra Import System Design

## Goals

- Resolve `import` statements across multiple files so that identifiers can be referenced without fully qualified prefixes.
- Support aliasing and reexports, enabling explicit control over symbol names that enter a module.
- Provide a standard prelude that brings curated stdlib symbols into scope automatically, letting users call math helpers without writing `std.`.
- Maintain fast incremental builds by reusing the existing parser cache and avoiding redundant disk scans.

## Current Status (2025-11-09)

- `ModuleResolver` builds a canonical module graph (topologically ordered) and surfaces detailed diagnostics for missing files, duplicate declarations, header mismatches, and cycles.
- Each `ResolvedImport` now carries the resolved module index and the list of public symbols exposed by the target module, priming the semantic analyser for cross-file symbol lookup and future visibility checks.
- Imports created by the compiler (`std.prelude`, future feature gates, etc.) are flagged as `synthetic`, allowing diagnostics and tooling to suppress messages that should only apply to user-authored imports.
- Public `import` reexports are resolved into `ModuleAlias` bindings; the resolver threads those aliases through the semantic workspace so downstream modules can traverse namespaces like `mylib.math.add()` without reaching into the original stdlib module.
- The semantic layer materialises a `SemanticWorkspace`, populating module-level symbol tables that power import alias lookups during analysis.
- Resolver diagnostics now carry precise origin spans, so the CLI surfaces missing-module errors with file and line information for each offending import.
- The `spectra` CLI executes shared semantic analysis across every resolved module graph before handing source files to the backend, preventing duplicate cross-module failures from surfacing as repeated per-file errors.

## Module Discovery and Path Mapping

- Module identifiers use dot-separated segments; `foo.bar.baz` maps to the relative file path `foo/bar/baz.spectra`.
- The resolver maintains an ordered list of search roots:
  1. Directory of the entry module passed to the CLI.
  2. Parent directories of any user-provided source files (covers `src`, `modules`, etc.).
  3. Paths listed in `Spectra.toml` via `libs = ["path/to/lib", ...]` (resolved relative to the manifest).
  4. Additional roots supplied via repeated `--lib <path>` / `-L<path>` CLI flags.
  5. The bundled stdlib root shipped with the runtime distribution.
- The first matching source file wins. All matches are normalised to canonical paths to avoid duplicate modules that differ only by path casing.
- Each parsed module validates that its declared `module` header matches the canonical module path derived from its file location; mismatches produce a diagnostic that suggests renaming the file or updating the header.

## Dependency Graph and Caching

- The `ModuleResolver` constructs a module graph by starting from the entry module and following parsed imports breadth-first.
- Cycles are detected using a depth-first traversal; when a back-edge is found, the resolver reports the full cycle chain so authors can break it deliberately.
- The existing `ModuleLoader` cache remains the compilation pipeline entry point. Graph traversal reuses cached parses by module ID (the fully qualified module name) to avoid re-tokenising unchanged files.
- Hashing incorporates feature flags, so enabling an experimental feature triggers the expected reparse.

## Import Syntax

- Supported forms after the parser update:
  - `import foo.bar;` (private import, default alias equals the last segment: `bar`).
  - `import foo.bar as math;` (explicit alias).
  - `pub import foo.bar;` (reexports the module for downstream consumers; resolver will make the alias visible to dependants).
- Future enhancements (documented for follow-up implementation):
  - Selective imports: `import foo.bar.{add, sub};` bring named items into scope without referencing the alias.
  - Glob imports: `import foo.bar.*;` for transitional use when migrating to selective imports.
  - Relative imports (e.g., `import .helpers.logging;`) resolved against the current module path.

## Symbol Binding and Prelude Strategy

- Every module exposes the public subset of its declarations. Private imports stay scoped to the defining module, while `pub import` reexports the alias and any members pulled via selective or glob forms.
- The stdlib exposes a `std.prelude` module that reexports curated symbols (`std.math`, `std.text`, etc.). The parser injects a synthetic private import of `std.prelude` into every module unless the source file disables the prelude with `#![no_prelude]`.
- The prelude module itself reexports members using `pub import` alongside selective imports such as `pub import std.math.{add, sub, mul, div};`, ensuring downstream modules receive the bare function names automatically.
- Prelude injection is represented internally by synthetic `Import` items flagged during parsing so diagnostics can differentiate between explicit and implicit imports.

## CLI Integration Plan

- Extend the project planner (`tools/spectra-cli/src/project.rs`) to compute the module search roots and build the dependency graph prior to invoking the compiler pipeline. _(Implemented via `ModuleResolver` integration.)_
- Provide `--lib <path>` CLI switch and `Spectra.toml` `libs = [...]` entries so automation can register extra roots. (`--no-prelude` CLI toggle remains future work.)
- Emit diagnostics when a requested module is missing, duplicated across roots, or has incompatible visibility (for example, attempting to import a private module).
- Cache graph metadata on disk (JSON sidecar) to accelerate repeated CLI runs; cache invalidation occurs when source timestamps or configuration inputs change.

## Implementation Roadmap

1. **Parser groundwork (done):** accept aliases and `pub import` so source files can express intent before resolver support is shipped.
2. **Resolver core (in progress):** build the module graph, detect cycles, and feed resolved modules into the semantic analyser. (Graph construction and import-to-module association complete; semantic wiring pending.)
3. **Name binding:** extend the semantic layer to populate symbol tables using import aliases and prelude-provided symbols.
4. **Prelude rollout (done):** ship `std.prelude`, inject it automatically during parsing (respecting `#![no_prelude]`), and update stdlib documentation/examples to use bare names.
5. **Selective/glob imports:** syntax and binding are now implemented in the parser, resolver, and semantic layers, so tooling and diagnostics operate on precise per-symbol metadata.
6. **Tooling updates:** refresh formatter, language server contracts, and documentation once resolver semantics are live.

## Outstanding Questions

- (Resolved in November 2025) Ambiguous aliases now emit semantic diagnostics that list both imports and recommend explicit aliasing.
- Should the resolver support mixed-case file names on case-insensitive filesystems? (Current plan: canonicalise to lower case when comparing module IDs on Windows.)
- How do we package third-party modules? (To be answered alongside the package manager design.)
