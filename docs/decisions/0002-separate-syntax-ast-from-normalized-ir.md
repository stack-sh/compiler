# ADR-0002: Separate the Syntax AST from Normalized IR

## Status

Accepted

## Date

2026-09-02

## Context

A parser must represent what an author wrote, including omitted properties, source order, duplicate properties, unresolved identifiers, and precise source locations. Layout and rendering code should not need to interpret those syntax choices or independently apply Stack defaults.

Using the syntax AST as the cross-repository interface would expose grammar details to every downstream consumer. It would also force each consumer to repeat semantic validation and normalization.

## Decision

Expose two distinct representations:

- The syntax AST mirrors Stack declarations and properties. It preserves source spans, authored order, omissions, and duplicates needed for diagnostics.
- The normalized IR represents a semantically valid diagram. It applies specification-defined defaults, resolves structural membership, separates nodes, groups, and edges, and uses typed enums for closed semantic values.

Semantic validation is a distinct pass between the two representations. The compiler produces normalized IR only when lexical, syntax, and semantic errors are absent. Warnings do not prevent IR construction.

The normalized IR must be deterministic, serializable without runtime handles, and independent of themes, layout engines, and renderers. Renderer-selected defaults, resolved theme data, icon SVG, coordinates, and text metrics do not belong in this IR.

Source positions use one-based line and column numbers in diagnostics. Internally, spans also retain zero-based UTF-8 byte offsets. Range ends are exclusive.

## Alternatives Considered

### Let layout consume the AST

- Pros: No additional representation or lowering pass.
- Cons: Couples layout to grammar and duplicates defaulting, reference resolution, and validation across consumers.
- Rejected: Syntax is not a stable renderer contract.

### Normalize the AST in place

- Pros: Fewer data types and allocations.
- Cons: Loses whether a value was authored or defaulted and prevents precise duplicate-property diagnostics.
- Rejected: Diagnostic tooling and downstream rendering need different information.

### Include resolved theme and icon assets in compiler IR

- Pros: One object contains everything needed by layout.
- Cons: Makes compilation depend on a catalog version and external assets, and prevents pure offline language validation.
- Rejected: Theme resolution creates a later visual representation owned by downstream code.

## Consequences

- AST types may evolve with grammar additions, while normalized IR is the intentional downstream boundary.
- Parser and validator tests can assert authored syntax independently from normalized meaning.
- The compiler performs a small allocation cost to lower from AST to IR.
- Future formatter work may add a concrete syntax or token representation for comment preservation without placing trivia in normalized IR.
