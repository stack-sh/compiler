# ADR-0006: Add a Source-Map Sidecar

## Status

Accepted

## Date

2026-09-03

## Context

The layout, icon, and theme stages run after compiler normalization. Their portable diagnostics must point back to authored Stack source, including an unsatisfied `order` hint, an unavailable icon, or an unavailable theme. Normalized IR deliberately excludes source locations so it remains portable, deterministic semantic data rather than a representation of one source spelling.

Defaults introduce another distinction: downstream code must know whether `default` or a missing icon came from omitted syntax or from an authored value. Returning only optional spans would conflate an unknown semantic identity with a known value that was omitted.

## Decision

Add `compile_with_source_map` and `compile_bytes_with_source_map`. A successful result pairs the unchanged normalized diagram with a Rust-only `SourceMap`. A compiler-stage error returns neither diagram nor source map.

The sidecar provides:

- the authored theme identifier span or `SourceOrigin::Omitted`;
- one node-icon origin for every node, addressed by globally unique node identifier;
- one layout-order origin for the diagram and every group, addressed by diagram or group identity.

An authored node icon points to its string token. An authored theme points to its identifier. An authored order hint covers the complete statement from the `order` keyword through the closing list bracket, including intervening trivia. Omitted values use an explicit `SourceOrigin::Omitted`; an unknown node or group identity returns `None`.

Node entries follow depth-first declaration order. Layout entries place the diagram first and groups in depth-first declaration order. Lookups use those stable vectors rather than randomized maps because Stack 1.0 limits the collections to small sizes.

The sidecar reuses lossless tokens to recover the complete order-statement span. It has no JSON schema, is not part of portable interchange, and adds no filesystem, network, clock, or runtime dependency.

## Alternatives Considered

### Add spans to normalized IR

- Pros: One object would contain semantic data and diagnostic locations.
- Cons: Portable equality would depend on source spelling and every downstream schema would inherit compiler implementation details.
- Rejected: Normalized IR must remain source-independent.

### Return optional spans directly

- Pros: Smaller API surface.
- Cons: `None` cannot distinguish an omitted default from an unknown semantic identity.
- Rejected: Downstream fallback diagnostics need the distinction explicitly.

### Re-scan source in the engine

- Pros: Keeps the compiler API smaller.
- Cons: Duplicates lexical and scope interpretation and can associate diagnostics with the wrong declaration.
- Rejected: The compiler already owns source parsing and semantic identity.

## Consequences

- Engine stages can emit diagnostics at stable authored ranges without changing portable IR.
- Omitted defaults are explicit and do not masquerade as missing map entries.
- The source map is available only from the additive mapped compile APIs.
- Invalid programs do not expose potentially ambiguous semantic mappings.
