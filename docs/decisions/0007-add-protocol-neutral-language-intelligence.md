# ADR-0007: Add Protocol-Neutral Language Intelligence

## Status

Accepted

## Date

2026-09-05

## Context

Native editors, browser editors, and future hosted tools need the same Stack diagnostics, completion, hover, and document-symbol semantics. The canonical specification defines versioned request and response data, but an editor transport such as the Language Server Protocol also introduces document synchronization, negotiated position encodings, request cancellation, and stale-result handling.

Putting those transport concerns inside the compiler would make the pure frontend stateful and couple portable language semantics to one client protocol. Conversely, implementing semantic queries independently in each adapter would duplicate parsing, defaults, identifier resolution, completion ordering, and source-range behavior.

Completion also needs icon identifiers that are not part of the Stack grammar. The compiler cannot discover them without taking a dependency on a theme catalog, filesystem, network, or application state.

## Decision

Add a public `language_intelligence` module that implements the compiler-owned parts of the canonical language-intelligence 1.0 contract:

- diagnostics for UTF-8 text or bytes;
- syntax- and scope-aware completion;
- semantic hover with reference resolution;
- hierarchical document symbols.

Every operation is stateless and analyzes one complete source snapshot. Responses echo an opaque caller-supplied `document_version` but the compiler does not retain or order versions. Completion and hover validate that the supplied zero-based UTF-8 byte offset and one-based Unicode-scalar line and column identify the same scalar boundary.

Completion accepts an explicit request-local icon catalog. The compiler bounds the number and length of entries, rejects invalid or duplicate identifiers, treats all descriptive fields as plain text, and never loads catalog data itself. Language values and document identifiers come only from canonical syntax and the parsed source.

The module exposes dependency-free Rust domain types rather than JSON or protocol-specific structures. A feature-gated integration test maps those types to the exact canonical fixture responses using development-only JSON support. CI also cross-builds the same library for `wasm32-unknown-unknown`; WebAssembly bindings do not fork the semantic implementation.

Canonical formatting remains a formatter responsibility. LSP, JavaScript, and other adapters own serialization, position-encoding conversion, document lifecycle and incremental state, cancellation, stale-result filtering, and presentation mapping.

## Alternatives Considered

### Implement an LSP server in the compiler crate

- Pros: Editors could consume one ready-made process.
- Cons: Makes a deterministic library own transport, process, document-state, and protocol-version concerns.
- Rejected: An LSP adapter should translate to the portable compiler API rather than define its semantics.

### Add JSON serialization to the runtime API

- Pros: WebAssembly and process adapters could forward values directly.
- Cons: Adds a runtime dependency and prematurely fixes a transport representation for every Rust consumer.
- Rejected: A development-only fixture adapter proves compatibility while public native types remain transport-neutral.

### Let callers provide completion callbacks

- Pros: Catalog discovery could be lazy and application-specific.
- Cons: Introduces runtime behavior, nondeterminism, and target-specific callback boundaries into the core.
- Rejected: A bounded immutable catalog keeps each request deterministic and portable.

### Implement language intelligence separately in every consumer

- Pros: Each integration can optimize for its local editor framework.
- Cons: Diagnostics, resolution, ordering, and ranges would drift between native and browser products.
- Rejected: All consumers must share one compiler-owned semantic implementation.

## Consequences

- Native and WebAssembly consumers can share deterministic language semantics.
- The compiler remains dependency-free, stateless, and free of filesystem or network access.
- Callers must supply a complete source snapshot, exact source coordinates, a document version, and any catalog entries needed for completion.
- Protocol adapters remain responsible for lifecycle correctness and coordinate conversion.
- Format responses are composed with the canonical formatter rather than this compiler module.
- Public Rust types may evolve before the crate's first stable release, but fixture compatibility remains pinned to an exact specification revision.
