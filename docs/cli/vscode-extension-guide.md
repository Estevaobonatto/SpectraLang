# Spectra VS Code Extension Guide

This document describes how to install, configure, and validate the Spectra VS Code extension that lives under `tools/vscode-extension/`.

## Prerequisites

- Node.js 18 or newer
- npm (bundled with Node.js)
- The Spectra CLI available on your PATH or an absolute path you can point the extension to
- VS Code 1.93.0 or newer

## Local Installation

1. Open VS Code and select **Run and Debug** → **Start Debugging** to launch the extension in the Extension Development Host.
2. Alternatively, build the extension bundle and install it manually:

   ```bash
   npm install
   npm run compile
   vsce package # produce a VSIX once publishing automation lands
   ```

3. In the Extension Development Host, verify the "Spectra" entry appears under the Extensions view.

## Configuration

The extension exposes a dedicated configuration namespace (`spectra.*`). These settings may be set in user or workspace scope via `settings.json` or the Settings UI.

| Setting | Default | Description |
| --- | --- | --- |
| `spectra.cliPath` | `spectra` | Location of the Spectra CLI executable. Point this to a workspace-local build when testing unreleased changes. |
| `spectra.lintOnSave` | `true` | Run `spectra repl --json` whenever a Spectra file is saved, updating VS Code diagnostics. |
| `spectra.formatOnSave` | `false` | Stream the active document through `spectra fmt --stdin` before it is saved. |

Changes to `spectra.cliPath` automatically invalidate the cached metadata probe so the extension re-queries the CLI version.

## Commands

| Command ID | Palette Title | Behavior |
| --- | --- | --- |
| `spectra.diagnostics.run` | Spectra: Run Diagnostics for Current File | Invokes `spectra repl --json <file>` and applies the resulting diagnostics to the active document. |
| `spectra.lintWorkspace` | Spectra: Lint Workspace | Invokes `spectra lint --json` over all workspace roots, replacing the diagnostics collection with the aggregated report. |

Both commands log their CLI interactions to the Spectra output channel for transparency and troubleshooting.

## Diagnostics and Formatting

- Diagnostics use the CLI's JSON payloads (`version: 1`), including related hints when provided by the compiler.
- Workspace lint clears stale entries before replaying the aggregated diagnostics so cross-file issues do not linger.
- Formatting streams document content into `spectra fmt --stdin` and replaces the buffer with the CLI output only when changes are detected.

## Verifying the Extension

The repository includes an automated smoke test harness under `tools/vscode-extension/src/test/`:

```bash
npm install
npm test
```

The suite launches `@vscode/test-electron` against a fixture workspace and a mock Spectra CLI. It validates command registration, per-file diagnostics, workspace lint aggregation, and formatter round-trips without requiring the real compiler.

## Troubleshooting

- Confirm the configured CLI path is executable from the VS Code environment. Errors are surfaced in the Spectra output channel.
- If diagnostics produce empty payloads, inspect the output channel for the raw JSON emitted by the CLI.
- When using workspace-local builds of the CLI, prefer setting `spectra.cliPath` in `.vscode/settings.json` to avoid affecting global user settings.
- Delete `.vscode-test` if a previous test run downloaded a stale VS Code binary and you want a clean environment.

## Roadmap Preview

Future iterations will layer hover and go-to-definition features on top of the compiler's semantic pipeline, expand formatter coverage in the test harness, and publish a signed VSIX once the CLI release process is stabilized.
