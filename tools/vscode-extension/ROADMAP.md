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

1. Route diagnostics through the CLI's JSON channels (e.g., `spectra repl --json`) to capture richer metadata and related quick fixes.
2. Align the workspace lint command with `spectra lint` once its reporter guarantees per-file locations across the project graph.
3. Document installation, configuration, and troubleshooting steps in `docs/cli`, linking to the extension roadmap and grammar status.
4. Add automated smoke tests using `@vscode/test-electron` covering formatter, diagnostics, and command availability.
5. Track hover and go-to-definition requirements, feeding discoveries back into the Spectra compiler roadmap.
