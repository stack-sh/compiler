# Stack Compiler

`stack-compiler` is the reference Rust frontend for the [Stack language](https://github.com/stack-sh/specification). It parses Stack source, validates its language semantics, applies specification-defined defaults, and produces a renderer-independent normalized diagram.

The compiler does not resolve themes or icons, calculate layout, render SVG, access the network, or read files. Those concerns belong to downstream libraries and applications.

## Status

Stack 1.0 and this compiler are both under active development. Public Rust APIs may change before the first stable release.

Development and primary CI follow the latest stable Rust and Cargo releases through [`rust-toolchain.toml`](./rust-toolchain.toml). Rust 1.85 remains the minimum supported version and is verified in a separate CI job.

## Cargo package

Add the compiler library from crates.io with:

```sh
cargo add stack-compiler@0.1.0
```

The published package is built and documented on Rust 1.85 or newer. Repository CI performs a full crates.io packaging dry run so the released source archive remains independent of Git checkouts.

Maintainers follow the [initial publication procedure](./docs/releasing.md) before the first registry release.

## Pipeline

```text
Stack source
    -> lexical analysis -----> lossless tokens and trivia
    -> syntax AST
    -> semantic validation
    -> normalized Diagram IR
```

## Commands

| Command | Purpose |
| --- | --- |
| `cargo test` | Run the test suite |
| `cargo llvm-cov --lib --all-features --workspace --fail-under-lines 95 --fail-under-functions 95 --fail-under-regions 95` | Enforce unit-test coverage |
| `cargo fmt --check` | Check Rust formatting |
| `cargo clippy --all-targets --all-features -- -D warnings` | Run the linter |
| `cargo doc --no-deps` | Build API documentation |

Install [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) before running the coverage command. CI measures the library unit tests independently and requires line, function, and region coverage to remain at or above 95 percent.

Package-level Clippy lints reject panic-producing `unwrap`, `expect`, `panic`, `unreachable`, `todo`, and `unimplemented` calls in library and test targets.

## Conformance

The language-independent schemas and fixtures live in [`stack-sh/specification`](https://github.com/stack-sh/specification). This repository records its tested specification commit in [`tests/specification-revision`](./tests/specification-revision).

Run the canonical suite against a local specification checkout:

```sh
STACK_SPECIFICATION_DIR=../specification cargo test --features conformance --test conformance --test language_intelligence_conformance
```

JSON support is development-only and does not add a runtime dependency to the compiler library. CI checks out the recorded specification revision before running the suite.

## Language Intelligence

The `language_intelligence` module implements the protocol-neutral [Stack language-intelligence contract](https://github.com/stack-sh/specification/blob/main/LANGUAGE_INTELLIGENCE.md). Its stateless APIs return diagnostics, semantic completion, plain-text hover information, and hierarchical document symbols for one complete source snapshot. Each response echoes the caller's document version.

Completion accepts an explicit, bounded caller-owned icon catalog. The compiler never fetches a catalog or interprets its labels and documentation as markup. Source positions combine a zero-based UTF-8 byte offset with one-based Unicode-scalar line and column coordinates; inconsistent positions are rejected before analysis.

Editor and transport adapters own document synchronization, cancellation, stale-result filtering, and conversion from protocol-specific coordinates such as UTF-16. Formatting remains owned by the canonical formatter. CI compares the native types with the pinned canonical fixtures and cross-builds the same dependency-free core for `wasm32-unknown-unknown`; a JavaScript or LSP binding is intentionally outside this crate.

## Lossless Source Model

`parse_lossless` and `parse_lossless_bytes` expose every authored token, whitespace segment, line comment, original string spelling, CRLF sequence, and end-exclusive source span. Concatenating token text through `lossless::Document::reconstruct` reproduces the input byte-for-byte. This source-oriented API is separate from normalized IR and performs no filesystem access.

Lossless parsing succeeds for syntactically valid source even when semantic validation would later reject it. Lexical and syntax errors return diagnostics without a partial document.

## Source-Map Sidecar

`compile_with_source_map` and `compile_bytes_with_source_map` return normalized IR together with a Rust-only `SourceMap`. The sidecar resolves the diagram theme, every node icon, and every diagram or group order hint from semantic identity to either an authored source span or `SourceOrigin::Omitted`. It is deterministic, has no portable JSON representation, and is absent whenever compiler-stage errors prevent normalized IR.

## Architecture

- [`docs/decisions/0001-build-a-portable-rust-compiler-core.md`](./docs/decisions/0001-build-a-portable-rust-compiler-core.md)
- [`docs/decisions/0002-separate-syntax-ast-from-normalized-ir.md`](./docs/decisions/0002-separate-syntax-ast-from-normalized-ir.md)
- [`docs/decisions/0003-use-a-handwritten-parser.md`](./docs/decisions/0003-use-a-handwritten-parser.md)
- [`docs/decisions/0004-consume-a-pinned-conformance-suite.md`](./docs/decisions/0004-consume-a-pinned-conformance-suite.md)
- [`docs/decisions/0005-add-a-lossless-source-model.md`](./docs/decisions/0005-add-a-lossless-source-model.md)
- [`docs/decisions/0006-add-a-source-map-sidecar.md`](./docs/decisions/0006-add-a-source-map-sidecar.md)
- [`docs/decisions/0007-add-protocol-neutral-language-intelligence.md`](./docs/decisions/0007-add-protocol-neutral-language-intelligence.md)
- [`docs/specs/compiler-frontend.md`](./docs/specs/compiler-frontend.md)

## License

Apache-2.0
