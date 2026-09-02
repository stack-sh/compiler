# Stack Compiler Agent Guide

## Source of Truth

The canonical language contract is `stack-sh/specification`. Never introduce syntax, defaults, semantics, or `STK` diagnostic meanings that are not defined there.

## Technology

- Rust 2024 edition
- Latest stable Rust and Cargo for development and primary CI
- Rust 1.85 minimum supported version, verified separately in CI
- Standard library only unless an ADR accepts a dependency
- No unsafe Rust

## Commands

- Format: `cargo fmt --check`
- Test: `cargo test`
- Unit-test coverage: `cargo llvm-cov --lib --all-features --workspace --fail-under-lines 95 --fail-under-functions 95 --fail-under-regions 95`
- Lint: `cargo clippy --all-targets --all-features -- -D warnings`
- Documentation: `cargo doc --no-deps`

## Conventions

- Keep parsing, semantic validation, and normalization as separate stages.
- Preserve source spans and duplicate declarations in the AST so validation can report the authored mistake.
- Keep the normalized IR deterministic, renderer-independent, and free of filesystem or network handles.
- Use specification-assigned `STK` codes only for their normative meanings.
- Add focused tests for every diagnostic or language rule implemented.
- Keep line, function, and region coverage at or above 95 percent.
- Keep GitHub Actions on their latest supported major versions; Dependabot checks for updates weekly.
- Do not use panic-producing `unwrap`, `expect`, `panic`, `unreachable`, `todo`, or `unimplemented` macros in any target; package-level Clippy lints enforce this boundary.
- Write repository content, code comments, issues, and pull requests in English.
- Keep temporary implementation plans and task lists outside the repository under `/tmp`.

## Boundaries

- Always run formatting, tests, coverage, Clippy, and documentation checks before delivery.
- Ask before adding a runtime dependency or changing a public representation.
- Do not create a commit unless the user explicitly requests one.
- Never add theme resolution, icon retrieval, layout, SVG rendering, HTTP, authentication, or storage access to this repository.
- Never add temporary plan or todo files to the repository or its ignore rules.
- Never commit credentials, generated build output, or editor-specific state.
