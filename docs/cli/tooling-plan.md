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

1. **Core formatting rules**
   - Reuse the parser to produce a concrete syntax tree with trivia (comments, whitespace).
   - Implement a pretty-printer that normalizes indentation, line wrapping, and trailing whitespace.
   - Provide configuration via `Spectra.toml` (indent width, max line length, brace style).
2. **CLI integration**
   - Add `spectra fmt <paths>` that runs in-place or with `--check` to validate formatting.
   - Exit with code `65` when formatting changes are required (mirrors common formatter conventions).
3. **Automation hooks**
   - Offer `--stdin`/`--stdout` mode for editor integration.
   - Document formatter usage in project templates and contributor guides.

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
   - Expose `Format Document` and `Lint` commands by invoking the new CLI subcommands.
   - Cache CLI lookups and present errors using the CLI exit codes to differentiate failures.
3. **Roadmap alignment**
   - Track editor features (hover, go-to-definition) in the same repo to keep parity with CLI improvements.
   - Document extension usage in `docs/cli` and update alpha checklist as milestones land.

## Automation & CI Considerations

- Add formatter/linter checks to CI once the commands are available, failing builds on exit codes `65`.
- Provide sample GitHub Actions workflow in `tools/spectra-cli` to demonstrate format/lint gating.
- Encourage projects to use `spectra fmt --check` before publishing to maintain consistent style.
