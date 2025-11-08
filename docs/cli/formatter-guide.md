# Spectra Formatter Guide

The Spectra CLI bundles a formatter that normalizes indentation, spacing, blank lines, and grouped `let` bindings. Use it to keep your project consistent and to integrate with editors and CI pipelines.

## Command Overview

Run the formatter over one or more files or directories:

```shell
spectra fmt <paths>
```

Key flags:

- `--check`: verify formatting without writing changes. Returns exit code `65` if changes are needed.
- `--stdin`: read source from standard input and emit the formatted version to standard output.
- `--stdout`: format a single on-disk file and write the result to standard output rather than editing the file in place.
- `--config <path>`: load formatter settings from an explicit `Spectra.toml` file, useful when editors work on scratch copies outside the project tree.

## Configuration

Formatter settings live under the `[formatter]` table inside `Spectra.toml`. The CLI searches from each formatted file up to the filesystem root and uses the nearest manifest it finds. The search order is skipped when `--config` is supplied.

Supported keys:

| Key               | Type | Default | Description |
|-------------------|------|---------|-------------|
| `indent_width`    | int  | `4`     | Number of spaces per indentation level. Must be between `1` and `12`. |
| `max_line_length` | int  | `100`   | Maximum desired line length. Guard rails alignment of grouped bindings. Minimum value is `40`. |

Unknown keys produce a usage error so typos are caught early.

### Example `Spectra.toml`

```toml
[package]
name = "spectra_app"
version = "0.1.0"

[build]
entry = "src/main.spectra"

[formatter]
indent_width = 2
max_line_length = 88
```

Place this file at the project root. The formatter will apply the configuration to any file under that directory tree.

## Editor Integration

- Use `spectra fmt --stdin --stdout` for editor commands that stream file contents through the CLI.
- Combine `--config` with an absolute path when your editor writes temporary files outside the project tree.

## Continuous Integration

Add a formatting gate to your CI workflow to keep pull requests consistent. A minimal shell step looks like:

```shell
spectra fmt --check .
```

When the formatter reports changes, the command exits with code `65`, causing the build to fail until the author applies `spectra fmt` locally.

The repository includes an end-to-end example workflow at `tools/spectra-cli/.github/workflows/spectra-fmt-check.yml` that runs the formatter in CI and surfaces diffs when formatting fails.
