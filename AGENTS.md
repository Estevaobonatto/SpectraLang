# AGENTS.md

SEMPRE USAR SKILL CAVEMAN E SUAS VARIANTES

## Purpose

This file defines repository-specific instructions for coding agents working in SpectraLang.

Its main focus is the correct use and maintenance of the implementation planning documents created for the project:

- [docs/production-ai-implementation-plan.md](/D:/Lang/SpectraLang/docs/production-ai-implementation-plan.md)
- [docs/roadmap-backlog.md](/D:/Lang/SpectraLang/docs/roadmap-backlog.md)
- [roadmap/roadmap.toml](/D:/Lang/SpectraLang/roadmap/roadmap.toml)

Agents must treat these files as operational project artifacts, not passive documentation.

---

## Repository Context

SpectraLang is a language and toolchain project with these major areas:

- `compiler/`: lexer, parser, AST, semantic analysis, linting, pipeline
- `midend/`: IR lowering, validation, optimization
- `backend/`: Cranelift codegen, JIT/AOT
- `runtime/`: runtime services, memory, host calls, stdlib plumbing, async reactor, API crate host calls
- `runtime/src/api/`: HTTP parser, server, client, JSON, TLS, routing (sibling to the existing `runtime/src/stdlib/`)
- `runtime/src/reactor/`: platform-specific event loop (`epoll` / `IOCP` / `kqueue`)
- `tools/spectra-cli/`: CLI
- `tools/spectra-lsp/`: language tooling / LSP
- `tools/spectra-interop/`: language interop
- `tests/`: language, semantic, CLI, and project tests
- `docs/`: language docs, project docs, implementation planning docs
- `docs/api/`: API library reference (added in Phase 22)
- `roadmap/`: machine-readable roadmap tracking
- `packages/spectra-api/`: the published Spectra package that delivers the API platform surface (Phase 22+)
- `examples/api/`: runnable API examples for REST, WebSocket, GraphQL, gRPC, SSE, and database integration

The platform targets two complementary workstreams:

- **AI/ML core**: tensors, autodiff, ONNX, RAG, ML serving, model evaluation, drift detection.
- **API platform**: async/await first-class syntax, `spectra.api` package (HTTP/1.1 → HTTP/2 → HTTP/3, TLS, JSON, middleware, auth, OpenAPI, drivers, observability).

---

## Source of Truth Rules

When working with implementation planning, use the following precedence:

1. Actual code and tests
2. `roadmap/roadmap.toml`
3. `docs/roadmap-backlog.md`
4. `docs/production-ai-implementation-plan.md`
5. Older planning notes such as `docs/project-manager.md`

Interpretation rules:

- Code and passing tests define current reality.
- `roadmap/roadmap.toml` is the canonical structured execution tracker.
- `docs/roadmap-backlog.md` is the canonical human-readable execution backlog.
- `docs/production-ai-implementation-plan.md` is the canonical long-form strategic implementation plan.
- If older docs conflict with the three files above, update the older docs or explicitly note the conflict.

---

## Required Planning Files

Agents must preserve and maintain the following files:

### 1. Strategic Plan

File:
- [docs/production-ai-implementation-plan.md](/D:/Lang/SpectraLang/docs/production-ai-implementation-plan.md)

Purpose:
- long-term implementation vision
- workstream decomposition
- production-gap coverage
- acceptance criteria by phase

Update this file when:
- a major subsystem direction changes
- a new workstream is added
- a major architectural dependency changes
- the project focus for AI/ML capabilities changes

Do not update this file for:
- minor task status changes
- small bug fixes
- one-off tactical implementation notes

### 2. Human Backlog

File:
- [docs/roadmap-backlog.md](/D:/Lang/SpectraLang/docs/roadmap-backlog.md)

Purpose:
- issue-ready work breakdown
- human planning and prioritization
- readable status and acceptance list

Update this file when:
- a roadmap item is added, removed, split, merged, or reprioritized
- acceptance criteria change
- recommended execution ordering changes

### 3. Structured Roadmap

File:
- [roadmap/roadmap.toml](/D:/Lang/SpectraLang/roadmap/roadmap.toml)

Purpose:
- machine-readable project planning
- automation, reporting, dependency tracking

Update this file when:
- status changes
- dependencies change
- ownership changes
- risk changes
- an item is created or closed

Whenever an item is changed in the backlog, update `roadmap.toml` in the same change unless there is a clear reason not to.

---

## Agent Responsibilities

When an agent works on implementation tasks, it must decide whether the work changes:

- code only
- code plus roadmap status
- roadmap only
- roadmap plus strategic plan

### Minimum Required Behavior

For every substantial implementation task, the agent should:

1. Identify the relevant roadmap item IDs if they exist.
2. Check whether the task changes status, dependencies, or acceptance criteria.
3. Update `roadmap/roadmap.toml` if the task materially changes execution state.
4. Update `docs/roadmap-backlog.md` if the task changes human planning context.
5. Update `docs/production-ai-implementation-plan.md` only if strategy or architecture changed.

### Examples

If a bug fix completes a tracked item:
- update code
- update `roadmap.toml` status
- update backlog status/notes if relevant

If a task only fixes a local parser bug not tied to roadmap structure:
- update code
- only update roadmap files if the bug was part of a tracked item

If a new AI workstream is introduced, such as ONNX export:
- update strategic plan
- update backlog
- update roadmap.toml

---

## Status Update Rules

Use only these status values in `roadmap/roadmap.toml` unless the file schema is intentionally changed:

- `not_started`
- `in_progress`
- `blocked`
- `complete`

Use these rules consistently:

- `not_started`: no implementation has begun
- `in_progress`: active implementation or active design work is happening
- `blocked`: work cannot continue due to dependency, design uncertainty, or missing prerequisite
- `complete`: acceptance criteria are satisfied and the implementation is validated

Do not mark an item `complete` merely because partial code exists.

An item may be marked `complete` only when:

- implementation is merged in the working tree
- relevant tests or validation exist
- stated acceptance criteria are satisfied or explicitly revised

### Production Completion Rule

Agents must implement roadmap items to the production meaning of the planning
documents, not to a reduced "alpha" interpretation. If an item mentions
vectorization, benchmark evidence, interop, memory safety, diagnostics, or
production hardening, those requirements are part of completion unless the
planning files are explicitly and narrowly revised first.

Partial implementations are allowed only when they are reported and tracked as
partial. In that case:

- keep `roadmap/roadmap.toml` status as `in_progress`, not `complete`
- keep the original production acceptance criteria visible
- add "completed so far" and "remaining before completion" notes in the backlog
- never rename partial work as complete by adding labels such as alpha, MVP, or
  prototype unless the roadmap item itself explicitly defines that as the target
- never claim benchmarks, performance wins, SIMD, BLAS, memory-safety, or
  production readiness without checked-in evidence and validation commands

Before reporting an implementation as complete, agents must compare the final
diff against all acceptance criteria in:

1. `roadmap/roadmap.toml`
2. `docs/roadmap-backlog.md`
3. `docs/production-ai-implementation-plan.md`

If any criterion is not implemented and validated, the item remains
`in_progress`.

### Core Language Correction Rule

When analyzing or correcting core language constructs such as variables,
assignments, `if`, `unless`, `while`, `while let`, `do-while`, `for`, `loop`,
`switch`, `match`, methods, structs/classes, traits, closures, and return paths,
agents must validate the full pipeline, not only parser acceptance.

Required behavior:

- add or update at least one `.spectra` regression in `tests/validation/` or
  `tests/errors/` for the affected construct
- run a frontend/semantic test when the change affects parsing, binding,
  type checking, diagnostics, or return-path analysis
- run a midend/backend or normal `spectralang run` validation when the change
  affects lowering, branches, loops, method calls, aggregate values, or JIT/AOT
  execution
- do not promote a feature from experimental/beta to stable unless it parses
  without flags, compiles, and executes through the normal CLI path when it has
  runtime behavior
- if a construct has compile-only coverage but no reliable execution coverage,
  keep it tracked as incomplete or add a roadmap item with production
  acceptance criteria before calling it complete

Current production baseline:

- `switch`, `unless`, `do-while`, and `loop` are stable core syntax, not active
  experimental features.
- `--enable-experimental <feature>` is accepted only as a compatibility no-op
  until a future gated feature is added and documented.
- `spectralang --list-experimental` must report no active syntax gates unless
  the maturity policy and roadmap are updated in the same change.

### Integrated Project Failure Triage Rule

When executing the Phase 20 integrated language and AI Support validation track
(`R-2008` through `R-2013`), agents must not ignore real implementation
failures found in complete projects.

Required behavior:

- if the failure is fixed in the same change, add regression coverage and record
  validation evidence
- if the failure is not fixed in the same change, add a new roadmap/backlog item
  beyond `R-2008` through `R-2013`
- the new item must include owner, phase, dependencies, risk, acceptance
  criteria, and the reproducing project or command
- do not mark the integrated project gate complete while untracked failures
  remain

---

## Acceptance Criteria Rules

Acceptance criteria are completion gates, not aspirations.

Agents must:

- keep them specific
- keep them testable
- avoid vague language like "improved", "better", or "more robust" without measurement

Good acceptance criteria:

- "`cargo test -p spectra-cli` passes"
- "JSON diagnostics include stable codes for syntax and semantic errors"
- "MLP example trains end-to-end on the reference dataset"

Bad acceptance criteria:

- "tooling is much better"
- "GPU support is mostly done"
- "docs look complete"

If implementation reveals that an acceptance criterion is wrong or unrealistic:

- update it explicitly
- keep the change narrow
- preserve intent

---

## Dependency Update Rules

When changing an item in `roadmap.toml`, review:

- `dependencies`
- `phase`
- `owner`
- `priority`
- `risk`

Agents must update dependencies when:

- a prerequisite becomes necessary
- a previous prerequisite is no longer needed
- an item is split into multiple items

Do not leave stale dependencies behind.

---

## Adding New Roadmap Items

When adding a new roadmap item:

1. Add it to `docs/roadmap-backlog.md`
2. Add it to `roadmap/roadmap.toml`
3. Place it in the correct phase
4. Assign:
   - unique ID
   - owner
   - priority
   - risk
   - dependencies
   - concise summary
   - acceptance criteria

### ID Convention

Use:

- `R-###` for roadmap items

Recommended grouping:

- `R-0xx`: governance
- `R-1xx`: compiler productionization
- `R-2xx`: type system
- `R-3xx`: tensors
- `R-4xx`: kernels/runtime numerics
- `R-5xx`: autodiff
- `R-6xx`: ML framework
- `R-7xx`: accelerators
- `R-8xx`: interop
- `R-9xx`: package manager/registry
- `R-10xx`: tooling maturity
- `R-11xx`: concurrency/serving
- `R-12xx`: security/ops
- `R-13xx`: docs/adoption
- `R-21xx`: async language core
- `R-22xx`: API library foundation (`spectra.api`)
- `R-23xx`: middleware and security
- `R-24xx`: advanced API features
- `R-25xx`: persistence and database
- `R-26xx`: API tooling and developer experience
- `R-27xx`: observability and API operations
- `R-28xx`: API conformance and release

Do not renumber existing IDs unless absolutely necessary.

---

## Owner Groups

Every roadmap item in `roadmap/roadmap.toml` is owned by exactly one group.
Owners are responsible for design, implementation, tests, and the
acceptance criteria of items in their group, and for raising cross-cutting
risks in the planning files.

| Owner | Scope |
|---|---|
| `frontend` | lexer, parser, AST, diagnostics, language surface |
| `semantic` | type system, imports, traits, validation |
| `midend` | IR lowering, optimization, validation, graph IR |
| `backend` | Cranelift, object emission, target ABIs |
| `runtime` | runtime services, allocators, reactor, async stdlib, host calls |
| `numerics` | tensor core, kernels, BLAS/GPU integration, numerics conformance |
| `ml` | autodiff, modules, optimizers, datasets, model serving, ML safety |
| `web` | HTTP server/client, routing, middleware, WebSocket, SSE, OpenAPI, `spectra.api` package |
| `db` | drivers, query builder, migrations, ORM, connection pool |
| `tooling` | CLI, formatter, lint, LSP, debugger, benchmarks, scaffolder |
| `ecosystem` | package manager, registry, ADRs, documentation, examples, release |

The `web` and `db` groups were introduced together with the API platform
workstream (Phase 22+). Items that previously fell under `runtime` purely
because they were "in the runtime crate" should be reassigned to `web` or
`db` when the work shifts from runtime infrastructure to the public API
surface or the database layer. The cross-cutting review rules defined in
this file apply uniformly across all owner groups.

---

## Splitting and Merging Items

If a roadmap item is too large:

- split it into smaller items
- preserve the original intent
- rewire dependencies explicitly
- update both backlog and TOML

If two items are truly redundant:

- merge only if acceptance criteria overlap heavily
- document the merge in the backlog text
- preserve traceability where possible

---

## Documentation Synchronization Rules

When changing planning docs:

- keep terminology consistent across all three files
- keep phase names aligned
- keep item IDs aligned
- keep acceptance criteria semantically equivalent between backlog and TOML

Do not update only one planning artifact when the change affects all of them.

Expected synchronization:

- strategic change: update all relevant files
- task-level execution change: update backlog and TOML
- pure status change: usually update TOML, optionally backlog if human readability benefits

---

## Validation Rules for Planning Files

After editing planning files, agents should validate:

### For `roadmap/roadmap.toml`

- file parses successfully as TOML
- all item IDs are unique
- all dependency IDs reference existing items
- all phases referenced by items exist

### For `docs/roadmap-backlog.md`

- no orphan item IDs
- no references to removed phases
- no contradiction with `roadmap.toml`

### For `docs/production-ai-implementation-plan.md`

- phases still match project strategy
- new workstreams are represented cleanly
- acceptance direction remains coherent with backlog

---

## When to Update Older Documentation

Older documents such as:

- [docs/project-manager.md](/D:/Lang/SpectraLang/docs/project-manager.md)
- [README.md](/D:/Lang/SpectraLang/README.md)
- language reference files

should be updated if:

- they materially contradict the current implementation
- they materially contradict roadmap reality
- they present outdated implementation status as current truth

Do not automatically expand older docs during unrelated tasks.
Only update them when the inconsistency is relevant and significant.

---

## AI/ML Direction Rules

Because SpectraLang is intended to evolve toward AI and machine learning workloads, agents must prefer planning decisions that strengthen:

- tensor-first language/runtime design
- numerical correctness
- reproducibility
- performance visibility
- accelerator readiness
- interop with existing ML ecosystems

Agents should avoid roadmap drift toward generic-language polish at the expense of the AI core unless the missing feature is a direct blocker.

In practical terms:

- tensor/runtime/autodiff/interop work is generally higher leverage than cosmetic syntax work
- package manager and tooling are high priority if they unblock real ML use
- documentation should keep the AI production target explicit

---

## API Platform Direction Rules

Because SpectraLang is also intended to be a first-class language for
building HTTP and event-driven APIs natively, agents must prefer planning
decisions that strengthen:

- async/await as a first-class language and runtime model
- typed HTTP primitives (`Request`, `Response`, `Method`, `Status`, `Header`, `Cookie`)
- middleware and authentication as composable, documented building blocks
- first-class drivers for the dominant production backends (PostgreSQL, SQLite, Redis)
- observability and threat mitigation as documented defaults, not afterthoughts
- toolchain that turns a scaffolded project into a running service in minutes
- conformance, interop, and production hardening before declaring v1.0

The `spectra.api` package is the canonical home for this surface. New HTTP,
routing, middleware, WebSocket, SSE, or database work that targets the
public API surface belongs in `spectra.api` and is owned by the `web` or
`db` owner groups, not by `runtime` or `frontend`.

In practical terms:

- Phase 21 (async/await) is the foundation; no API platform work begins
  before its core is in place.
- HTTP/1.1 is the first protocol; HTTP/2 and HTTP/3 follow once the
  foundation is stable.
- TLS, authentication, validation, and error handling are non-negotiable
  defaults, not optional extras.
- Driver coverage should match the dominant production backends first;
  exotic drivers are follow-on work.
- The `spectralang api new` / `spectralang api dev` / `spectralang api doc`
  experience is part of the public contract.
- `R-2801` (API conformance v1) is the release-candidate gate for
  `spectra.api` v1.0.

---

## Completion Reporting Guidance

When reporting completed work to users or maintainers, agents should:

- mention the roadmap item IDs affected
- state whether code, backlog, and roadmap were updated
- state whether acceptance criteria were satisfied or revised

Recommended concise format:

- `Completed: R-104, R-105`
- `Updated: code + roadmap.toml + roadmap-backlog.md`
- `Acceptance: satisfied`

---

## Do Not

Agents must not:

- treat strategic planning docs as throwaway notes
- change item IDs casually
- mark work complete without validation
- leave roadmap dependencies stale
- update backlog prose while forgetting `roadmap.toml`
- introduce new planning terminology without aligning existing files

---

## Recommended Default Workflow

For substantial implementation work:

1. Read the relevant roadmap items in:
   - `roadmap/roadmap.toml`
   - `docs/roadmap-backlog.md`
2. Implement code changes.
3. Run the relevant validation/tests.
4. Update roadmap status and dependencies if needed.
5. Update backlog notes if planning context changed.
6. Update strategic plan only if architecture or long-term direction changed.

For planning-only work:

1. Update `docs/production-ai-implementation-plan.md` if strategy changed.
2. Reflect actionable execution changes in `docs/roadmap-backlog.md`.
3. Reflect structured task changes in `roadmap/roadmap.toml`.
4. Validate TOML parse and cross-file consistency.
