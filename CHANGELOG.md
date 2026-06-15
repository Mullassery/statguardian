# Changelog

All notable changes to StatGuard are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
StatGuard uses [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

_Nothing yet._

---

## [0.1.0] — 2026-06-15

### Added

**Core**
- PEG grammar DSL (`grammar.pest`) supporting `dataset`, `schema`, `quality`,
  `stats`, `anomalies`, and `stream` sections
- Severity prefixes: `@info`, `@warning`, `@error` (default), `@blocking`
- Full AST (`ast.rs`): `DataContract`, `FieldDef`, `QualityRule`, `StatsRule`,
  `AnomalyRule`, `StreamConfig`
- 3-pass compiler optimizer: deduplication → null-check fusion → cost-sort
- Compiled `ExecutionDag` with column grouping for parallel execution

**Schema validation**
- Type checking: `int`, `float`, `string`, `bool`, `date`, `datetime`, `bytes`
- Constraints: `not_null`, `unique`, `primary_key`, `positive`, `negative`,
  `coerce`, `regex=`, `between()`, `min=`, `max=`, `len()`, `enum=[]`

**Quality rules**
- Metrics: `completeness`, `uniqueness`, `validity`, `consistency`, `non_null_rate`
- Comparison operators: `>`, `<`, `>=`, `<=`, `==`, `!=`

**Drift detection**
- Population Stability Index (PSI)
- Kolmogorov–Smirnov (KS) statistic
- Stat functions: `mean`, `std`, `median`, `min`, `max`, `p05`, `p95`, `p99`, `p999`

**Anomaly detection**
- `detect_outliers(method="iqr"|"zscore")`
- `detect_duplicates`
- `detect_nulls`
- `detect_cardinality_explosion`
- `detect_pattern_breaks`

**Profiling**
- Per-column: null rate, distinct count (HyperLogLog precision=14), min/max/mean/std,
  percentiles (p05/p25/p50/p75/p95/p99), 10-bucket histogram
- Profiling runs on every execution at no extra cost

**Output**
- `ValidationReport` (JSON, pretty JSON, Prometheus text format)
- `DatasetHealthScore` with letter grade (A/B/C/D/F)
- `ExecutionSummary` one-liner for CI / logging

**IO**
- Auto-detecting file reader: Parquet, CSV, JSON/NDJSON, Arrow IPC
- `StreamingBatcher` for large files (batch-size slicing)
- `RowBuffer` for micro-batch streaming pipelines

**Python API**
- `DataContract.from_dsl(str)` / `DataContract.from_file(path)`
- `execute(contract, df, reference=None)` → `ValidationReport`
- `execute_file(contract, path, reference_path=None)` → `ValidationReport`
- `execute_streaming(contract, path, batch_size=10000)` → `List[ValidationReport]`
- `validate_dsl(str)` — syntax check without execution
- `statguard validate / check` CLI with JSON, summary, and Prometheus output formats
- `pip install statguard` / `uv add statguard`

**Tests**
- 25 unit + integration tests across all crates
- All tests pass on the first release

[Unreleased]: https://github.com/Mullassery/StatGuard/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Mullassery/StatGuard/releases/tag/v0.1.0
