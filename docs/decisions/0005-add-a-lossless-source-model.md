# ADR-0005: Add a Lossless Source Model

## Status

Accepted

## Date

2026-09-03

## Context

The canonical formatter must preserve comments and their token gaps while normalizing whitespace and authored string escapes. The existing syntax AST intentionally stores decoded semantic values and declaration spans, while the lexer discards whitespace and comments. Normalized IR excludes all source trivia by contract.

Implementing another parser in the formatter would let syntax handling drift from the compiler. Adding trivia to normalized IR would make a portable semantic representation depend on source spelling. Mutating every existing AST node to carry trivia would also expand an API whose current responsibility is syntax and validation.

## Decision

Add a separate public lossless source model. `parse_lossless` and `parse_lossless_bytes` run the same lexer and recursive-descent parser used by the established parse path, then expose a flat source-order sequence containing:

- language tokens with their exact authored text and end-exclusive span;
- decoded values for string tokens while retaining the original escape spelling;
- whitespace segments, including tabs, LF, and CRLF;
- complete line-comment lexemes without absorbing their line endings;
- a zero-width end token.

The source text of all tokens and trivia segments concatenates to the original UTF-8 input byte-for-byte. Lossless parsing requires lexical and syntactic validity but is independent of semantic validation, so a syntactically valid document with name or layout errors remains inspectable. Lexical or syntax errors return the existing portable diagnostics without a partial lossless document.

The AST and normalized IR types remain unchanged. The source model does not read files, consult the network or clock, or add a runtime dependency.

## Alternatives Considered

### Put trivia in the normalized IR

- Pros: Downstream tools would consume one representation.
- Cons: Source spelling and comments are not portable diagram meaning and would destabilize semantic interchange.
- Rejected: Normalized IR must remain renderer-independent and formatting-free.

### Add trivia fields throughout the existing AST

- Pros: Formatting structure and source text would live in one tree.
- Cons: Broadly changes established syntax types and makes semantic consumers carry formatter-only data.
- Rejected: A separate token model is additive and keeps existing responsibilities intact.

### Tokenize again in each formatter

- Pros: No compiler API addition.
- Cons: Duplicates string, comment, operator, and position behavior and can drift from compiler diagnostics.
- Rejected: The compiler must own one lexical interpretation of Stack source.

## Consequences

- Formatters and editors can preserve authored bytes without reimplementing lexical rules.
- Callers use the lossless tokens alongside the existing AST when structural context is required.
- The model owns each token's exact text and decoded strings, trading modest allocation for a simple lifetime-free public API.
- Invalid UTF-8, byte order marks, and syntax errors remain diagnostic-only results.
