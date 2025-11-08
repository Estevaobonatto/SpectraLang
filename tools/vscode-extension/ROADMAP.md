# Spectra VS Code Extension Roadmap

## Architecture Overview

- **Extension entry point (`src/extension.ts`)**
  - Creates the Spectra output channel and wires activation hooks.
  - Registers the Spectra diagnostics manager, formatting provider, and format-on-save integration.
  - Exposes commands for ad-hoc diagnostics and workspace lint, while tracking configuration changes for the CLI path.
- **CLI integration (`src/cliClient.ts` and `src/cliCapabilities.ts`)**
  - `runSpectraCli` spawns the Spectra CLI with cancellable streams for stdout/stderr collection.
  - CLI metadata caching probes the configured executable once for version information, reusing results across the session.
- **Diagnostics pipeline (`src/diagnostics.ts`)**
  - Parses Spectra CLI output into VS Code diagnostics, updating a dedicated collection per document.
  - Automatically triggers lint-on-save and cleans up diagnostics when documents close.
- **Formatter integration (`src/formatter.ts`)**
  - Streams document contents to `spectra fmt --stdin`, returning text edits or surfacing CLI failures.
  - Supports opt-in format-on-save while respecting the editor's indentation configuration.
- **Language assets**
  - TextMate grammar (`syntaxes/spectra.tmLanguage.json`) supplies syntax highlighting scoped to `source.spectra`.
  - `language-configuration.json` defines comments, brackets, and indentation rules for Spectra files.

## Near-Term Tasks

### Recently Completed

1. Routed diagnostics through the CLI's JSON channels (`spectra repl --json`) to capture richer metadata and related hints.
2. Aligned the workspace lint command with `spectra lint --json`, reusing the aggregated report to update VS Code diagnostics.
3. Added an automated smoke-test harness (`npm test`) powered by `@vscode/test-electron` and a mock Spectra CLI.

### In Flight

1. Document installation, configuration, and troubleshooting steps in `docs/cli`, linking to the extension roadmap and grammar status.
2. Expand the smoke tests to cover formatter round-trips, malformed JSON handling, and missing-CLI error paths.
3. Track hover and go-to-definition requirements, feeding discoveries back into the Spectra compiler roadmap.
