# ADR-0004: Consume a Pinned Canonical Conformance Suite

## Status

Accepted

## Date

2026-09-02

## Context

`stack-sh/specification` now owns portable normalized IR, diagnostic schemas, and implementation-independent conformance fixtures. The Rust compiler must prove that its native IR and diagnostics map to those contracts without copying canonical fixture data into this repository.

Tracking the specification default branch implicitly would make compiler CI non-reproducible. A specification change could break an unchanged compiler commit, and passing CI would no longer identify the language revision actually supported. Vendoring fixtures would avoid network access but would create two sources of truth.

The compiler core is intentionally dependency-free. JSON is needed only by the conformance adapter and must not become part of source parsing or runtime compilation.

## Decision

Record the exact supported specification commit in `tests/specification-revision`. Compiler CI checks out that immutable revision and passes its path explicitly to a feature-gated integration test through `STACK_SPECIFICATION_DIR`.

The conformance test discovers canonical valid and invalid case directories. It maps native Rust IR and diagnostics to semantic JSON values and compares them with expected documents. Diagnostic comparison includes code, severity, and end-exclusive source range but excludes human-readable message, help, and related-information wording.

`serde_json` is a development-only dependency. The library keeps no runtime dependencies and exposes no JSON-specific public API. The conformance test is gated by the `conformance` Cargo feature so a normal package checkout can run `cargo test` without fetching another repository.

When the compiler adopts a newer language revision, the fixture runner must pass locally before `tests/specification-revision` changes. Once the specification publishes stable releases, a release commit may replace draft commit pins while retaining exact reproducibility.

## Alternatives Considered

### Copy fixtures into the compiler repository

- Pros: Tests are self-contained and offline.
- Cons: Expected data can drift from the canonical specification.
- Rejected: Language fixtures must have one owner.

### Track `stack-sh/specification` main automatically

- Pros: Immediate detection of specification changes.
- Cons: An unchanged compiler revision can begin failing without any local change.
- Rejected: Compatibility claims must identify an exact tested revision.

### Add serialization dependencies to the compiler library

- Pros: Downstream tools could request JSON directly from the core crate.
- Cons: Expands the public API and runtime dependency surface before WASM and package boundaries are designed.
- Rejected: A test-only adapter proves the contract without committing to a runtime transport API.

### Use a Git submodule

- Pros: Git records an exact external revision.
- Cons: Adds clone and contributor workflow complexity for a test suite that normal builds do not require.
- Rejected: A revision file and explicit CI checkout provide the same reproducibility with less repository coupling.

## Consequences

- Compiler CI states exactly which public specification revision it supports.
- Canonical fixtures remain owned by one repository.
- Conformance runs require a specification checkout and explicit environment variable.
- Updating specification support is a deliberate reviewed change.
- The compiler lockfile includes development-only JSON dependencies, while the built library remains dependency-free.
