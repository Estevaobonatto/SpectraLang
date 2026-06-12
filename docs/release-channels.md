# SpectraLang Release Channels

SpectraLang uses explicit release channel metadata for the CLI, packages, and
release evidence.

## Channels

| Channel | Purpose | Compatibility expectation |
|---------|---------|---------------------------|
| `nightly` | active development builds and local scaffolds | may change between builds; migration notes should be published for breaking changes |
| `beta` | release candidates and ecosystem validation | compatible within the declared compatibility level unless documented in migration notes |
| `stable` | production releases | compatible within the declared compatibility level; breaking changes require a new compatibility level |

The default package scaffold uses:

```toml
[release]
channel = "nightly"
compatibility = "spectralang-0.1"
```

## Manifest Metadata

Packages may declare release metadata in `spectra.toml`:

```toml
[project]
name = "model_pipeline"
version = "0.2.0-beta.1"

[release]
channel = "beta"
compatibility = "spectralang-0.1"
deprecated_since = "0.3.0"
migration = "Use model_pipeline_v2 before stable promotion."
```

Rules:

- `channel` must be `nightly`, `beta`, or `stable`.
- `compatibility` must be a non-empty ASCII identifier such as `spectralang-0.1`.
- `deprecated_since` is optional.
- `migration` is required when `deprecated_since` is set.

## CLI Reporting

Use `release-info` for automation and human inspection:

```powershell
.\target\debug\spectralang.exe release-info --json --root .
```

The JSON schema is `spectralang.release-info.v1` and includes:

- CLI version, channel, and compatibility level.
- Package name, version, channel, compatibility, deprecation, and migration.
- Deprecation warnings suitable for CI logs.

The CLI channel defaults to `nightly`. Release builds may set compile-time
environment variables:

- `SPECTRA_RELEASE_CHANNEL=nightly|beta|stable`
- `SPECTRA_COMPATIBILITY_LEVEL=spectralang-0.1`

## Package Metadata

`spectralang package lock` writes release metadata into `spectra.lock`.
`spectralang package publish` writes the same channel and compatibility metadata
into registry `package.toml` alongside the checksum.

Deprecated packages emit:

```text
warning[release-deprecated]: package 'name' is deprecated since VERSION; migration: ...
```

This warning is non-fatal. Promotion policy should treat it as a release-review
signal: stable release candidates should not depend on deprecated packages unless
the migration decision is explicitly accepted.
