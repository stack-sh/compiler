# Stack Compiler

`stack-compiler` is the reference Rust frontend for the [Stack language](https://github.com/stack-sh/specification). It parses Stack source, validates its language semantics, applies specification-defined defaults, and produces a renderer-independent normalized diagram.

The compiler does not resolve themes or icons, calculate layout, render SVG, access the network, or read files. Those concerns belong to downstream libraries and applications.

## Status

Stack 1.0 and this compiler are both under active development. Public Rust APIs may change before the first stable release.

Development and primary CI follow the latest stable Rust and Cargo releases through [`rust-toolchain.toml`](./rust-toolchain.toml). Rust 1.85 remains the minimum supported version and is verified in a separate CI job.

## Pipeline

```text
Stack source
    -> lexical analysis
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

## Architecture

- [`docs/decisions/0001-build-a-portable-rust-compiler-core.md`](./docs/decisions/0001-build-a-portable-rust-compiler-core.md)
- [`docs/decisions/0002-separate-syntax-ast-from-normalized-ir.md`](./docs/decisions/0002-separate-syntax-ast-from-normalized-ir.md)
- [`docs/decisions/0003-use-a-handwritten-parser.md`](./docs/decisions/0003-use-a-handwritten-parser.md)
- [`docs/specs/compiler-frontend.md`](./docs/specs/compiler-frontend.md)

## License

Apache-2.0
