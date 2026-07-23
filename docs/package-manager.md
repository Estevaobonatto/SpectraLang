# Spectra Package Manager

This document describes the Phase 9 package manager, local registry baseline,
and Git-backed package catalog flow.

For a package-author focused walkthrough, see
[`docs/package-author-tutorial.md`](package-author-tutorial.md).

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
- Git dependencies installed into `.spectra/packages` and cached under `.spectra/git`
- package catalog dependencies resolved by `spectralang package add <name>`

Version handling:

- versions must use exact semver `MAJOR.MINOR.PATCH`
- prerelease suffixes such as `1.2.3-alpha.1` are accepted
- semver ranges are future work; catalog lookup currently chooses the newest
  compatible exact semver version when the user does not pin one
- compatibility is checked against the CLI's `spectralang-0.1` release
  compatibility (or the compiled `SPECTRA_COMPATIBILITY_LEVEL` value); a
  package with another compatibility level is rejected before installation or
  compilation
- `package@version` always requests that exact version; an unavailable exact
  version is an error and is never silently replaced by another version

Git packages use this manifest shape after install:

```toml
[dependencies.gitmath]
version = "1.2.3"
git = "https://github.com/org/gitmath.git"
tag = "v1.2.3"
checksum = "<sha256>"
```

Catalogs are configured per project:

```toml
[package.catalogs]
official = "https://github.com/spectralang/packages-index.git"
local = ".spectra/catalogs"
```

## Lockfile

`spectra package lock` writes `spectra.lock` at the workspace root.

The lockfile records:

- package name and version
- deterministic source path
- source kind (`path`, `registry`, or `git`)
- Git URL/ref and resolved commit SHA for Git packages
- SHA-256 package checksum
- manifest hash
- resolved dependency versions and sources

The lockfile is generated with deterministic package ordering so repeated resolution produces stable output for the same manifests.

Use `--locked` with package build/check/run/test/bench/doc/fetch commands to
require an existing lockfile whose package graph, sources, revisions, checksums,
manifest hashes, and dependencies exactly match the current resolution. Missing
or changed lockfiles fail before compilation. Without `--locked`, the normal
workflow refreshes `spectra.lock`.

Package downloads and vendor copies are staged beside their final destinations
and published only after validation. Existing valid caches are preserved when a
download, checksum, path, or copy operation fails. Payload symlinks and paths
that escape the package root are rejected.

Remote Git hosts can be restricted with the opt-in environment variable
`SPECTRA_PACKAGE_ALLOWED_HOSTS=github.com,gitlab.com`. Local Git paths and local
registries remain available for development and offline fixtures; remote hosts
must match the allowlist exactly when the variable is set.

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
spectralang package fetch --root .
```

Dependency management:

```powershell
spectralang package add core --root . --path ../core --version 0.1.0
spectralang package add core --root . --registry .spectra-registry --version 0.1.0
spectralang package add gitmath --root .
spectralang package add gitmath@1.2.3 --root .
spectralang package add gitmath --root . --git https://github.com/org/gitmath.git --tag v1.2.3
```

Catalog discovery:

```powershell
spectralang package search math --root .
spectralang package info gitmath --root .
spectralang package versions gitmath --root .
spectralang package tree --root .
```

Developer registration:

```powershell
spectralang package register --root . --git https://github.com/org/gitmath.git --tag v1.2.3 --catalog ./catalog
spectralang package publish-metadata --root . --git https://github.com/org/gitmath.git --tag v1.2.3 --out package.index.toml
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

## Git Package Catalog

Catalog files use schema `spectra-package-catalog-v1`:

```toml
schema = "spectra-package-catalog-v1"

[[packages]]
name = "gitmath"
version = "1.2.3"
git = "https://github.com/org/gitmath.git"
tag = "v1.2.3"
resolved_rev = "<commit-sha>"
checksum = "<sha256>"
description = "Math helpers"
keywords = ["math"]
compatibility = "spectralang-0.1"
license = "MIT"
modules = ["gitmath.core"]
owner = "org"
```

`spectralang package add <name>` searches configured catalogs, chooses the newest
compatible semver version unless the user pins `name@version`, clones/fetches
the Git repo, checks out the selected ref, copies the package payload into
`.spectra/packages`, writes the dependency into `spectra.toml`, and refreshes
`spectra.lock`.

Resolver determinism rules:

- catalog entries are validated before version ordering;
- identical entries for the same package/version are coalesced;
- conflicting entries for the same package/version fail and report both
  catalog origins;
- catalog order and filesystem order do not affect the selected version;
- duplicate workspace package names fail with both package roots;
- dependency cycles fail with the complete package chain, such as
  `a -> b -> c -> a`.

The R-905 resolver policy is covered by
`scripts/validate_r905_package_resolver.py`, which uses local Git repositories
and validates exact pins, prereleases, compatibility rejection, conflicts,
duplicates, cycle chains, and deterministic lockfiles.

Default tests use local Git fixtures so package validation never depends on a
public network by default.

Catalog publication is intentionally stricter than direct `--git` installs:

- `package register` and `package publish-metadata` require exactly one
  immutable ref, `--tag` or `--rev`; branch-only catalog entries are rejected,
  and `--rev` must be a commit SHA.
- The selected tag/rev must resolve to the package root `HEAD`, and the resolved
  commit SHA is recorded as `resolved_rev`.
- Re-registering the same package version is idempotent only when source URL,
  ref, resolved commit, checksum, exported modules, and compatibility match.
  Changing any of those fields requires a new package version.
- Published metadata validates package/module names, checksum shape, Git ref
  text, namespace ownership for exported modules, and control characters before
  writing the catalog file.

## Workspace Builds

`spectralang package build --root <workspace>`:

1. resolves workspace members
2. resolves local path dependencies
3. writes `spectra.lock`
4. compiles all dependency source roots before dependent roots

Normal `spectralang compile <project-dir>` also understands multi-package manifests and includes dependency sources when a project manifest contains workspace/path dependencies.

## Package-aware imports and diagnostics

Package commands preserve the package name and canonical package root for every
source module. Imports keep their fully qualified module name, for example
`gitmath.core`, and are checked before lowering. A missing module reports the
importing package, the requested package when it is known, and the relevant
source root. If two packages declare the same module name, resolution fails
before compilation and reports both package names and roots.

The semantic compiler also switches package context for each module. `internal`
symbols are therefore available between modules of the same package and are
rejected when imported from a different package. The reproducible integration
coverage is provided by `scripts/validate_r906_package_imports.py`.

Installed Git package modules are included in normal package command source
resolution, so code can use normal imports after install:

```spectra
import { double_plus_seed } from gitmath.core;
```

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
- register a Git package into a catalog
- search/info/versions over package catalogs
- one-command `package add <name>` from a catalog
- transitive Git dependency resolution
- normal import from installed Git packages
- package check/run/test/doc over installed Git packages
- offline fetch validation
- checksum mismatch failure
