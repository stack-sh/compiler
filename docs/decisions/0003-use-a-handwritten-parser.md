# ADR-0003: Use a Handwritten Parser

## Status

Accepted

## Date

2026-09-02

## Context

Stack 1.0 has a small, intentionally constrained grammar with contextual keywords. Diagnostics require exact source spans and stable Stack-specific error codes. The parser must preserve duplicate declarations and properties for a later semantic validation pass.

A parser generator could reduce some grammar code, but it would add a runtime dependency and make error recovery, contextual identifiers, and diagnostic mapping depend on generator behavior.

## Decision

Use a handwritten lexer and recursive-descent parser for Stack 1.x.

The lexer produces spanned tokens and handles comments, strings, escapes, punctuation, operators, and UTF-8 position tracking. Keywords remain ordinary word tokens and are interpreted by the parser according to grammatical context.

The parser follows the canonical EBNF directly. It rejects unknown declarations, properties, values, and operators. Initial syntax recovery may stop after the first lexical or syntax error because multi-error parser recovery is optional in Stack 1.0. Semantic validation must still collect independent errors in one pass.

No parser dependency is added initially. A future ADR may replace this parser if grammar growth or recovery requirements make the handwritten implementation materially harder to maintain.

## Alternatives Considered

### Parser generator

- Pros: Declarative grammar and generated parsing machinery.
- Cons: Additional dependency, less direct control over Stack diagnostics, and generator-specific recovery behavior.
- Rejected for the initial grammar: The language is small enough for direct implementation.

### Parser combinator library

- Pros: Reusable primitives and compact parser code.
- Cons: Additional dependency and error composition that may not align with normative diagnostic codes.
- Rejected for the initial grammar: Standard-library code is sufficient.

## Consequences

- Parser code remains explicit and easy to compare with the EBNF.
- Source range and diagnostic behavior stay under project control.
- Grammar changes require corresponding manual parser and test updates.
- The project must keep parsing functions small and structurally aligned with specification productions.
