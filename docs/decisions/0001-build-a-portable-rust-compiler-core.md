# ADR-0001: Build a Portable Rust Compiler Core

## Status

Accepted

## Date

2026-09-02

## Context

Stack source must produce the same meaning in a browser, a native CLI, and a hosted rendering service. The initial hosted design may use Cloudflare Workers, but the language implementation must not depend on one HTTP runtime, storage provider, or commercial product boundary.

The compiler is small and CPU-bound. It does not need theme assets, network access, a filesystem, or renderer state. Rust can produce native libraries and binaries for CLI use and WebAssembly modules for browser or Worker use from the same core implementation.

## Decision

Implement the Stack compiler frontend as a pure Rust library.

This repository owns the pipeline from decoded Stack source through normalized diagram IR:

1. lexical analysis;
2. syntax parsing;
3. identifier and default resolution;
4. semantic and complexity validation;
5. normalized IR construction.

The core API accepts source bytes or text and returns typed values and structured diagnostics. It must be deterministic and must not perform network, filesystem, clock, random, environment, or platform-specific operations.

Native Rust is the first supported target. WebAssembly bindings and a native CLI may wrap the same core in later changes, but target-specific adapters must remain outside the compiler stages.

Theme and icon resolution, layout, SVG rendering, HTTP handling, authentication, caching, and storage are explicitly outside this repository.

## Alternatives Considered

### TypeScript-only compiler

- Pros: Direct integration with browsers and Cloudflare Workers; shared types with web applications.
- Cons: Makes a native CLI less direct and ties the reference implementation more closely to JavaScript runtimes.
- Rejected: Rust provides the desired native and WebAssembly portability while a thin TypeScript host can still call the compiled module.

### Go compiler in a container service

- Pros: Straightforward native service deployment and good server concurrency.
- Cons: Browser execution and Worker integration are less direct; adopting a container as the language boundary couples local rendering to a hosted API.
- Rejected: A portable local engine is a primary product property, not only a deployment optimization.

### Hosted API as the only compiler interface

- Pros: One centrally deployed implementation and simple client code.
- Cons: Requires network access, adds operating cost and latency, and prevents private offline rendering.
- Rejected: Hosted rendering may be offered later as a managed convenience, but it is not required for language correctness.

## Consequences

- Browser, CLI, and hosted products can share one language implementation.
- The compiler can be tested without infrastructure or external assets.
- Rust-to-WebAssembly interface design and binary size require explicit verification later.
- Downstream repositories consume normalized IR rather than syntax-specific AST details.
- Commercial services must add value through hosting, collaboration, private assets, governance, or support rather than exclusive access to language semantics.
