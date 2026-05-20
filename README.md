# SpectraLang

SpectraLang is an experimental programming language and toolchain implemented in Rust. The repository contains the front-end compiler, semantic analysis, SSA-based midend, Cranelift backend, runtime, CLI, LSP, examples, and installer assets.

The project is currently in alpha. The language surface is evolving, some features are incomplete, and parts of the tooling are still under active development.

## Current Scope

- Lexer, parser, AST, semantic analysis, and linting
- SSA-style intermediate representation and optimization passes
- Cranelift-based JIT execution
- Object file emission for AOT workflows
- Runtime and standard library plumbing
- CLI and LSP tooling
- Windows installer assets

## Workspace Layout

- compiler: lexer, parser, AST, semantic analysis, linting, compilation pipeline
- midend: IR lowering, validation, and optimization passes
- backend: JIT and AOT code generation using Cranelift
- runtime: runtime services, memory management, FFI, stdlib hooks
- tools/spectra-cli: command-line interface
- tools/spectra-lsp: language server implementation
- examples: sample Spectra programs
- docs: language and project documentation
- installer: installer scripts and packaging assets

## Prerequisites

- Rust stable toolchain
- Cargo
- On Windows, MSVC build tools if you intend to build native artifacts or installer assets

## Build

```bash
cargo build
```

## Run An Example

```bash
spectralang run examples/basic.spectra
```

## Useful CLI Commands

```bash
spectralang check examples/basic.spectra
spectralang lint examples/basic.spectra
spectralang compile examples/basic.spectra
spectralang fmt examples/basic.spectra
```

## Project Status

SpectraLang is not yet a stable production language. At the moment:

- the language reference is still alpha
- cross-file/module linkage is limited
- standalone executable generation is not fully integrated end-to-end
- the standard library is still incomplete

See the documentation in docs for the current implemented surface and project notes.

## License

This repository is licensed under GPL-3.0. See LICENSE for the full text.