# Spectra CLI Tooling Plan

## Logging & Exit Codes (Ready)

- CLI emits errors prefixed with `error:` to simplify log parsing and automation hooks.
- Exit codes are stable and public:

| Code | Meaning                                      |
|-----:|----------------------------------------------|
|   0  | success                                      |
|  64  | usage error (invalid flags, missing inputs)  |
|  65  | compilation failed after diagnostics         |
|  74  | I/O failure while reading or writing files   |

- Help text documents these values so build pipelines and editors can react appropriately.

## Formatter Roadmap

### Status Overview

- ✅ CLI command `spectra fmt` formats files in-place, supports `--check`, `--stdin`, and `--stdout`, and exits with `65` when changes are needed.
- ✅ Formatter normalizes indentation, operator spacing, grouped `let` alignment (line-length aware), blank-line coalescing, and preserves line endings.
- ✅ `[formatter]` section in `Spectra.toml` now supports `indent_width` and `max_line_length`; settings are auto-discovered from the nearest manifest.
- ✅ `spectra fmt --config <path>` allows explicit config selection and reports unknown `[formatter]` keys as structured errors.
- ✅ Formatter caches `Spectra.toml` lookups per directory to avoid redundant IO across large projects.
- ✅ Added formatter-focused regression tests under `tools/spectra-cli`.
- ✅ Usage and configuration documented in `docs/cli/formatter-guide.md`, including sample `Spectra.toml` snippets.
- ✅ Sample GitHub Actions workflow (`tools/spectra-cli/.github/workflows/spectra-fmt-check.yml`) demonstrates `spectra fmt --check` gating.
- ✅ Token-aware CST formatting path introduced (with legacy fallback) to preserve trivia-aware spacing and unary operator handling.
- ✅ `spectra fmt --explain` surfaces line-oriented diffs for files that need formatting and reuses the compiler exit codes for gating.
- ✅ `spectra fmt --explain=json` emits structured diff payloads for editor and automation integrations.

### Next Steps

1. **CST policy extensions**
   - Layer configurable policies (brace style, trailing commas, import ordering) on top of the CST traversal.
   - Expand AST-aware passes to cover doc comment wrapping, nested comment indentation, and trailing comma heuristics.
2. **Editor & automation integration**
   - Document the JSON explain schema and versioning, including examples in the CLI guide.
   - Expose structured telemetry (formatted file counts, cache hits) for downstream integrations.
3. **Performance & UX**
   - Benchmark large workspaces and explore parallel formatting of independent files.
   - Reuse cached configuration state across successive CLI invocations (daemon or IPC-friendly mode).
   - Refine `--explain` presentation with hunked output and optional color for large diffs.

## Linter Roadmap

1. **Rule engine foundation**
   - Introduce lint pass in the semantic pipeline, emitting warnings and configurable error escalation.
   - Start with high-signal rules (unused bindings, unreachable code, shadowing).
2. **CLI surface**
   - Add `spectra lint` command plus `--lint` flag for build commands to enable linting during compilation.
   - Support `--allow`/`--deny` per rule via `Spectra.toml` and command-line overrides.
3. **Output conventions**
   - Emit diagnostics through the existing reporter so lint findings share the `error:`/`warning:` prefixes.
   - Reserve exit code `65` when `--deny` escalates to hard failures; otherwise exit `0` with warnings logged.

## VS Code Extension & Syntax Highlighting

1. **Language server scaffold**
   - Publish a minimal VS Code extension that shells out to `spectra repl --json` to provide diagnostics.
   - Ship Spectra grammar for TextMate highlighting, generated from parser tokens.
2. **Formatter & linter wiring**
   - Expose `Format Document` and `Lint` commands by invoking the new CLI subcommands (formatter streaming modes now available for editors).
   - Cache CLI lookups and present errors using the CLI exit codes to differentiate failures.
3. **Roadmap alignment**
   - Track editor features (hover, go-to-definition) in the same repo to keep parity with CLI improvements.
   - Document extension usage in `docs/cli` and update alpha checklist as milestones land.

## Automation & CI Considerations

- Add formatter/linter checks to CI once the commands are available, failing builds on exit codes `65`.
- Provide sample GitHub Actions workflow in `tools/spectra-cli` to demonstrate format/lint gating.
- Encourage projects to use `spectra fmt --check` before publishing to maintain consistent style.
