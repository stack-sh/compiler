# Compiler Frontend Specification

## Objective

Build the first reference frontend for Stack 1.0. It must parse canonical Stack source, expose a source-oriented AST, collect portable semantic diagnostics, apply language defaults, and produce normalized renderer-independent IR.

The primary users are Stack CLI, browser, editor, layout, and hosted-service implementations that need identical language semantics.

## Technology

- Rust 2024 edition
- Minimum Rust version: 1.85
- Development and primary CI toolchain: latest stable Rust and Cargo
- No runtime dependencies in the initial frontend
- No unsafe Rust

## Commands

- Build: `cargo build`
- Test: `cargo test`
- Unit-test coverage: `cargo llvm-cov --lib --all-features --workspace --fail-under-lines 95 --fail-under-functions 95 --fail-under-regions 95`
- Format: `cargo fmt --check`
- Lint: `cargo clippy --all-targets --all-features -- -D warnings`
- Documentation: `cargo doc --no-deps`

## Project Structure

```text
src/ast.rs          Syntax-oriented AST and spans
src/diagnostic.rs   Portable diagnostic types and codes
src/lexer.rs        UTF-8 text tokenization
src/lossless.rs     Exact tokens, trivia, authored text, and spans
src/parser.rs       Recursive-descent grammar implementation
src/source_map.rs   Engine-facing source origins by semantic identity
src/validation.rs   Semantic validation and normalization
src/validation/     Focused validation unit tests
src/ir.rs           Renderer-independent normalized model
src/lib.rs          Public parse and compile APIs
tests/              Public API and conformance-oriented tests
tests/specification-revision
                    Exact canonical specification revision tested by CI
docs/decisions/     Architectural decisions
```

## Public API Shape

```rust
pub fn parse(source: &str) -> ParseOutput;
pub fn parse_bytes(source: &[u8]) -> ParseOutput;
pub fn parse_lossless(source: &str) -> LosslessParseOutput;
pub fn parse_lossless_bytes(source: &[u8]) -> LosslessParseOutput;
pub fn compile(source: &str) -> CompileOutput;
pub fn compile_bytes(source: &[u8]) -> CompileOutput;
pub fn compile_with_source_map(source: &str) -> SourceMappedCompileOutput;
pub fn compile_bytes_with_source_map(source: &[u8]) -> SourceMappedCompileOutput;
```

`ParseOutput` contains an AST only when decoding, lexing, and parsing succeed. `CompileOutput` contains normalized IR only when all compiler-stage errors are absent. Both outputs contain diagnostics. Semantic warnings may accompany successful IR.

`LosslessParseOutput` contains an exact token-and-trivia document only when decoding, lexing, and syntax parsing succeed. Its tokens retain original text and spans, while string tokens additionally expose decoded values. Reconstructing the document concatenates every token and trivia segment without normalization. Semantic validation remains a separate stage.

`SourceMappedCompileOutput` pairs successful normalized IR with a Rust-only source map. The sidecar identifies authored theme and icon values and complete layout order statements, while explicitly distinguishing omitted defaults. Compiler-stage errors suppress both IR and the source map. The normalized IR types and canonical JSON schema contain no source-map fields.

## Code Style

Prefer explicit domain types and exhaustive matches:

```rust
match operator {
    ast::EdgeOperator::Forward => ir::EdgeDirection::Forward,
    ast::EdgeOperator::Bidirectional => ir::EdgeDirection::Bidirectional,
    ast::EdgeOperator::Association => ir::EdgeDirection::Association,
}
```

- Use `snake_case` for modules and functions and `UpperCamelCase` for types.
- Keep parser functions aligned with named EBNF productions.
- Preserve authored duplicates in AST collections; reject them during validation.
- Avoid speculative abstractions and platform adapters.
- Avoid panic-producing extraction and placeholder macros. Package-level Clippy lints deny `unwrap`, `expect`, `panic`, `unreachable`, `todo`, and `unimplemented` calls.

## Testing Strategy

- Unit tests cover lexer escapes, positions, parser productions, defaults, and each implemented diagnostic.
- Integration tests exercise public `parse` and `compile` APIs.
- Feature-gated integration tests execute the canonical conformance suite from the recorded specification revision.
- Invalid cases assert stable diagnostic codes rather than entire prose messages.
- Library unit-test line, function, and region coverage must each remain at or above 95 percent.
- Every compiler change must pass formatting, tests, coverage, Clippy, and documentation builds.
- CI must also pass the complete test and Clippy suites on the minimum supported Rust version.

## Boundaries

### Always

- Follow `stack-sh/specification` grammar, semantics, limits, and diagnostic assignments.
- Keep AST, validation, and normalized IR as separate stages.
- Return deterministic diagnostics and IR.
- Treat source as untrusted plain text.

### Ask First

- Add a runtime dependency.
- Change an established public Rust type or normalized IR field.
- Implement syntax not present in the canonical specification.

### Never

- Fetch themes or icons.
- Perform layout or rendering.
- Read source from disk inside the core API.
- Interpret source strings as markup, paths, code, or URLs.
- Produce IR when compiler-stage errors exist.

## Initial Success Criteria

- All valid cases in the pinned canonical conformance suite parse and compile.
- Specification-defined defaults appear in normalized IR.
- UTF-8, BOM, invalid string, syntax, name-resolution, semantic, layout-scope, and complexity diagnostics use their assigned codes.
- Independent semantic errors are collected in one validation pass.
- The crate builds without unsafe code or runtime dependencies.
- Formatting, tests, Clippy, and API documentation checks pass.

## Deferred Work

- WebAssembly bindings
- Native CLI commands
- Canonical formatter implementation
- Theme and icon resolution
- Layout and renderer integration
- Multi-error syntax recovery
- Native Rust API stabilization and package versioning
