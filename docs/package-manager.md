# Spectra Package Manager

This document describes the Phase 9 package manager and local registry baseline.

## Manifest

Spectra packages use `spectra.toml`:

```toml
[project]
name = "my_package"
version = "0.1.0"
entry = "src/main.spectra"
src_dirs = ["src"]

[workspace]
members = ["packages/core"]

[dependencies]
core = { version = "0.1.0", path = "packages/core" }
```

Supported dependency sources:

- local path dependencies
- local registry dependencies installed into `.spectra/packages`

Version handling:

- versions must use exact semver `MAJOR.MINOR.PATCH`
- prerelease suffixes such as `1.2.3-alpha.1` are accepted
- semver ranges are future work

The Phase 9 baseline intentionally requires a concrete local path after resolution. Remote registry protocols and network downloads are not part of this baseline.

## Lockfile

`spectra package lock` writes `spectra.lock` at the workspace root.

The lockfile records:

- package name and version
- deterministic source path
- manifest hash
- resolved dependency versions and sources

The lockfile is generated with deterministic package ordering so repeated resolution produces stable output for the same manifests.

## Commands

```powershell
spectralang package lock --root .
spectralang package build --root .
spectralang package check --root .
spectralang package run --root .
spectralang package test --root .
spectralang package bench --root .
spectralang package doc --root .
spectralang package update --root .
```

Dependency management:

```powershell
spectralang package add core --root . --path ../core --version 0.1.0
spectralang package add core --root . --registry .spectra-registry --version 0.1.0
```

Publishing to a local registry:

```powershell
spectralang package publish --root packages/core --registry .spectra-registry
```

## Local Registry

The local registry layout is:

```text
registry/
  package_name/
    version/
      package.toml
      package/
        spectra.toml
        src/
```

`package.toml` contains:

- package name
- version
- checksum

`spectralang package add --registry` validates the checksum before copying the package into `.spectra/packages/<name>-<version>`.

## Workspace Builds

`spectralang package build --root <workspace>`:

1. resolves workspace members
2. resolves local path dependencies
3. writes `spectra.lock`
4. compiles all dependency source roots before dependent roots

Normal `spectralang compile <project-dir>` also understands multi-package manifests and includes dependency sources when a project manifest contains workspace/path dependencies.

## Validation

The main test runner validates:

```powershell
.\run_tests.ps1
```

Covered package scenarios:

- multi-package workspace compile
- package lock
- package build
- package check
- package doc
- publish to local registry
- install dependency from local registry with checksum validation
- build a registry consumer
