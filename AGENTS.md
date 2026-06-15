# StatGuard — AI Agent Reference

This file is for AI coding assistants (Claude, Copilot, Cursor, etc.) and
automated tooling. It describes the project layout, key abstractions,
contribution conventions, and common tasks.

---

## What this project is

StatGuard is a **Rust-native data quality and drift monitoring engine** with
a **Python API**. It compiles a declarative DSL into an optimised columnar
execution plan and runs it over Polars DataFrames.

**Stack:** Rust 2021 · Polars 0.44 · PyO3 0.21 · maturin · pest PEG grammar ·
Rayon · serde

---

## Repository layout

```
statguard/
├── Cargo.toml                    workspace root (7 crates)
├── pyproject.toml                maturin build config
├── LICENSE                       MIT
├── README.md                     user-facing docs
├── BENCHMARKS.md                 perf numbers vs Python libraries
├── AGENTS.md                     ← this file
├── CHANGELOG.md
├── CONTRIBUTING.md
│
├── crates/
│   ├── statguard-core/           DSL parser, AST, compiler, DAG, optimizer
│   │   └── src/
│   │       ├── ast.rs            all public AST types (DataContract, FieldDef, …)
│   │       ├── parser/
│   │       │   ├── grammar.pest  PEG grammar for the StatGuard DSL
│   │       │   └── mod.rs        pest parser → AST conversion
│   │       ├── compiler/
│   │       │   ├── dag.rs        DagNode enum + ExecutionDag struct
│   │       │   ├── optimizer.rs  3-pass optimizer (dedup, fuse, cost-sort)
│   │       │   └── mod.rs        Compiler::compile(contract) → ExecutionDag
│   │       └── error.rs          CoreError enum
│   │
│   ├── statguard-engine/         execution runtime
│   │   └── src/
│   │       ├── batch.rs          BatchExecutor (Rayon parallel per column)
│   │       └── lib.rs            Engine struct + run() convenience fn
│   │
│   ├── statguard-validators/     constraint checkers
│   │   └── src/
│   │       ├── schema.rs         SchemaValidator (type, null, regex, range, enum…)
│   │       ├── rules.rs          RuleEngine (completeness, uniqueness, validity…)
│   │       └── lib.rs            Violation struct
│   │
│   ├── statguard-stats/          profiling & drift
│   │   └── src/
│   │       ├── profiler.rs       Profiler → DatasetProfile / ColumnProfile
│   │       ├── drift.rs          DriftEngine → PSI, KS test, stat comparison
│   │       └── hll.rs            HyperLogLog cardinality estimator (precision=14)
│   │
│   ├── statguard-io/             data ingestion
│   │   └── src/lib.rs            DataReader (parquet/csv/json/ipc), StreamingBatcher, RowBuffer
│   │
│   ├── statguard-metrics/        report generation
│   │   └── src/
│   │       ├── report.rs         ValidationReport, DatasetHealthScore, ExecutionSummary
│   │       └── lib.rs
│   │
│   └── statguard-py/             PyO3 Python extension
│       └── src/lib.rs            DataContract, ValidationReport, execute(), execute_file(), …
│
├── python/
│   └── statguard/
│       ├── __init__.py           re-exports from the compiled _statguard extension
│       └── _cli.py               `statguard validate / check` CLI
│
└── tests/
    └── integration_test.rs       25 end-to-end tests
```

---

## Key data flow

```
DSL string
  │  statguard_core::parser::parse()
  ▼
Vec<DataContract>  (ast.rs types)
  │  Compiler::compile()
  ▼
ExecutionDag  (ordered DagNode list, grouped by column)
  │  Optimizer: dedup → fuse null checks → sort by cost
  │
  │  BatchExecutor::execute(df, reference)
  ├─ SchemaValidator::validate()   — per field
  ├─ RuleEngine::evaluate()        — quality rules
  ├─ Rayon parallel column groups  — dag node execution
  ├─ DriftEngine::evaluate()       — PSI + KS vs reference
  └─ Profiler::profile()           — always-on column stats
  │
  ▼
ValidationReport  → JSON / Prometheus / ExecutionSummary
```

---

## Common tasks

### Add a new constraint type

1. Add variant to `ast.rs` `Constraint` enum.
2. Add grammar rule to `parser/grammar.pest`.
3. Parse it in `parser/mod.rs` `parse_constraint()`.
4. Compile it to a `DagNode` in `compiler/mod.rs` `Compiler::compile()`.
5. Execute it in `engine/src/batch.rs` `execute_node()`.
6. Optionally add it to `validators/src/schema.rs` `check_constraints()`.

### Add a new quality metric

1. Add variant to `ast.rs` `MetricFn`.
2. Add keyword to grammar rule `metric_fn`.
3. Parse in `parser/mod.rs` `parse_metric_fn()`.
4. Implement in `validators/src/rules.rs` `compute_metric()`.

### Add a new stat function (drift)

1. Add variant to `ast.rs` `StatFn`.
2. Add keyword to `grammar.pest` `stat_fn`.
3. Parse in `parser/mod.rs` `parse_stat_fn()`.
4. Implement in `stats/src/drift.rs` `compute_stat()`.

### Add a Python binding for a new feature

All Python-visible types/functions are in `crates/statguard-py/src/lib.rs`.
Add a `#[pyfunction]` or method to `#[pymethods]`, then register it in the
`#[pymodule]` function at the bottom of the file.

---

## Build & test commands

```bash
# Check all crates (fast, no linking)
cargo check --workspace --exclude statguard

# Run all tests
cargo test --workspace --exclude statguard

# Release build (for benchmarking)
cargo test --release --workspace --exclude statguard

# Build Python extension (development mode)
maturin develop --release

# Install Python package from source
pip install -e ".[dev]"
```

---

## Coding conventions

- **No row loops** in hot paths — use Polars/Arrow columnar APIs.
- **No unwrap() in library code** — use `?` and `CoreError` / typed errors.
- **Rayon** for column-level parallelism; never spawn OS threads manually.
- Serde derive on all public report types.
- New public API needs a unit test in the same file (`#[cfg(test)]`) and an
  integration test in `tests/integration_test.rs`.

---

## Python package

Built with **maturin** + **pyo3**. The module name is `statguard._statguard`
(compiled), re-exported from `python/statguard/__init__.py`.

When the Python API accepts or returns a DataFrame it uses `pyo3-polars`'s
`PyDataFrame` to bridge between Python Polars and the Rust polars crate.

The CLI entry point is `statguard._cli:main`, registered in `pyproject.toml`.

---

## Testing

- Unit tests: inside each crate's `src/*.rs` under `#[cfg(test)]`.
- Integration tests: `tests/integration_test.rs` — exercises DSL → report.
- No mocks — tests use in-process DataFrames constructed with `polars::df![]`.
