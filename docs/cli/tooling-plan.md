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
- ✅ Added formatter-focused regression tests under `tools/spectra-cli`.
- ✅ Usage and configuration documented in `docs/cli/formatter-guide.md`, including sample `Spectra.toml` snippets.

### Next Steps

1. **Syntax-aware rewriter**
   - Reuse the parser to build a concrete syntax tree with trivia so spacing rules honor comments and complex constructs.
   - Extend configuration to include brace style, trailing comma policy, and import sorting once CST support lands.
2. **Editor & automation integration**
   - Add CI gate (`spectra fmt --check`) and provide a sample GitHub Actions workflow.
3. **Performance & UX**
   - Cache parsed configs per workspace during multi-file runs to avoid redundant IO.
   - Benchmark large projects and introduce parallelism or incremental formatting if needed.

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
