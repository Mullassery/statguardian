# Contributing to StatGuard

Thank you for contributing! This document covers how to set up the project,
the conventions we follow, and the review process.

---

## Prerequisites

| Tool | Version | Notes |
|---|---|---|
| Rust | ≥ 1.75 | `rustup update stable` |
| Python | ≥ 3.8 | for Python bindings |
| maturin | ≥ 1.7 | `pip install maturin` |
| Polars (Python) | ≥ 0.20 | `pip install polars` |

## Quick setup

```bash
git clone https://github.com/Mullassery/StatGuard.git
cd StatGuard

# Build and test the Rust crates
cargo test --workspace --exclude statguard

# Build the Python extension (development mode)
maturin develop --release

# Run Python tests
pip install ".[dev]"
pytest tests/
```

## Project structure

See [AGENTS.md](AGENTS.md) for a complete layout and key abstractions.

## Making changes

### Rust crates

1. Make your changes in the relevant crate under `crates/`.
2. Write or update tests (`#[cfg(test)]` in the same file, or `tests/integration_test.rs`).
3. Run `cargo clippy --workspace` and fix any warnings.
4. Run `cargo fmt --all`.
5. Run `cargo test --workspace --exclude statguard`.

### Python bindings

1. Edit `crates/statguard-py/src/lib.rs`.
2. Rebuild with `maturin develop --release`.
3. Test from Python.

### DSL grammar

The grammar lives in `crates/statguard-core/src/parser/grammar.pest`.
After editing:

1. Rebuild with `cargo build`.
2. Update `parse_*` functions in `crates/statguard-core/src/parser/mod.rs`.
3. Add a DSL test in `parser/mod.rs` `tests` module.

## Code conventions

- **No row loops** — use Polars/Arrow columnar APIs in hot paths.
- **No `unwrap()` in library code** — use `?` and typed errors.
- All public report/AST types must derive `Serialize, Deserialize`.
- New public functions need doc comments (`///`).
- Keep changes focused; one logical change per PR.

## Pull request checklist

- [ ] `cargo test --workspace --exclude statguard` passes
- [ ] `cargo clippy --workspace` has no warnings
- [ ] `cargo fmt --all --check` passes
- [ ] New feature has at least one new test
- [ ] CHANGELOG.md updated under `[Unreleased]`

## Reporting bugs

Open an issue at <https://github.com/Mullassery/StatGuard/issues> with:

- StatGuard version
- Minimal reproducing DSL and DataFrame
- Expected vs actual behaviour

## License

By contributing you agree that your changes will be licensed under the
[MIT License](LICENSE).
