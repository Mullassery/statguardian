# StatGuard

**Data quality, validation, and statistical drift monitoring for production data pipelines — built in Rust.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Version](https://img.shields.io/badge/version-0.1.0-blue)](https://github.com/Mullassery/statguard/releases)
[![Rust](https://img.shields.io/badge/built%20with-Rust-orange.svg)](https://www.rust-lang.org)

You write one contract file. StatGuard compiles it into an optimised columnar execution plan and validates your datasets — schema, quality rules, statistical drift, and anomalies — across every major file format and lakehouse table format.

**Python is the frontend. Rust is the engine.**

## Install

```bash
pip install statguard
uv add statguard
curl -sSfL https://raw.githubusercontent.com/Mullassery/statguard/main/install.sh | sh
```

See [INSTALL.md](INSTALL.md) for source builds and verification steps.

---

## Quick start

### 1. Write a contract

```
# orders.sg
dataset orders {
    schema {
        order_id:   string, not_null, unique, primary_key
        customer_id: string, not_null
        amount:     float,  positive, max=100000.0
        currency:   string, not_null, enum=["USD","EUR","GBP","JPY"]
        status:     string, not_null, enum=["pending","paid","cancelled","refunded"]
    }

    quality {
        @blocking: completeness(order_id) > 0.9999
        @warning:  uniqueness(order_id)   == 1.0
    }

    stats {
        amount.mean drift < 0.15
        amount.p95  drift < 0.25
    }

    anomalies {
        detect_outliers(amount, method="iqr")
        @blocking: detect_duplicates(order_id)
    }
}
```

### 2. Validate — any format

```python
import polars as pl
import statguard

contract = statguard.DataContract.from_file("orders.sg")

# Auto-detected from extension: Parquet, CSV, JSON, Avro, ORC, Arrow IPC, Delta, Iceberg
report = statguard.execute_file(contract, "orders.parquet")
report = statguard.execute_file(contract, "orders.csv")
report = statguard.execute_file(contract, "orders.avro")

# Delta Lake
report = statguard.execute_delta(contract, "/data/orders_delta/")
report = statguard.execute_delta(contract, "/data/orders_delta/", version=5)  # time travel

# Apache Iceberg
report = statguard.execute_iceberg(contract, "/data/orders_iceberg/")
report = statguard.execute_iceberg(contract, "/data/orders_iceberg/", snapshot_id=9876543)

# Polars DataFrame (in-memory)
df = pl.read_parquet("orders.parquet")
report = statguard.execute(contract, df)

print(report.summary())
# [StatGuard] PASS ✓ | dataset=orders | score=0.97 (A) | rows=500000 | violations=2 | 3ms
```

### 3. Drift detection

```python
# Compare today vs yesterday
report = statguard.execute_delta(
    contract, "/data/orders_delta/",
    version=10,
    reference_path="/data/orders_delta/",
    reference_version=9,
)

# Iceberg snapshot comparison
snapshots = statguard.list_iceberg_snapshots("/data/orders_iceberg/")
report = statguard.execute_iceberg(
    contract, "/data/orders_iceberg/",
    snapshot_id=snapshots[-1]["snapshot_id"],
    reference_snapshot=snapshots[-2]["snapshot_id"],
)

for d in report.drift_results():
    print(f"{d['column']}.{d['stat']}: drift={d['drift']:.4f}  PSI={d['psi']:.4f}  KS={d['ks_stat']:.4f}")
```

### 4. CLI

```bash
# Validate any format — auto-detected
statguard validate --contract orders.sg --file orders.parquet
statguard validate --contract orders.sg --file /data/orders_delta/
statguard validate --contract orders.sg --file /data/orders_iceberg/

# Drift: compare two datasets
statguard validate --contract orders.sg --file today.parquet --reference yesterday.parquet

# Output formats
statguard validate --contract orders.sg --file data.parquet --format json
statguard validate --contract orders.sg --file data.parquet --format prometheus

# Fail CI on any violation
statguard validate --contract orders.sg --file data.parquet --fail-on-warning

# DSL syntax check
statguard check --contract orders.sg
```

→ Full CLI reference: [docs/CLI.md](docs/CLI.md)

### 5. Streaming

```python
reports = statguard.execute_streaming(contract, "huge.parquet", batch_size=50_000)
for i, r in enumerate(reports):
    if not r.passed:
        print(f"Batch {i} FAILED: {r.summary()}")
        break
```

### 6. Cloud storage (S3, GCS, Azure)

```python
report = statguard.execute_cloud(contract, "s3://bucket/events/2026/06/*.parquet")
report = statguard.execute_cloud(contract, "gs://bucket/events.csv")
report = statguard.execute_cloud(contract, "az://container/data/")

# Drift across two cloud datasets
report = statguard.execute_cloud(
    contract,
    uri="s3://bucket/events/today/",
    reference_uri="s3://bucket/events/yesterday/",
)
```

### 7. SQL databases and warehouses

```python
# PostgreSQL, MySQL, SQLite (pure Rust)
report = statguard.execute_sql(
    contract,
    connection_string="postgresql://user:pass@localhost:5432/mydb",
    query="SELECT * FROM orders WHERE created_date >= '2026-01-01'",
)

# BigQuery, Snowflake, Redshift, Databricks, ClickHouse, DuckDB (Python layer)
report = statguard.execute_sql(
    contract,
    connection_string="bigquery://project/dataset",
    query="SELECT * FROM events LIMIT 1000000",
)

# Drift between two SQL queries
report = statguard.execute_sql(
    contract,
    connection_string="postgresql://localhost/db",
    query="SELECT * FROM events WHERE date = CURRENT_DATE",
    reference_query="SELECT * FROM events WHERE date = CURRENT_DATE - 1",
)
```

### 8. Apache Spark

```python
from pyspark.sql import SparkSession
import statguard

spark = SparkSession.builder \
    .config("spark.sql.execution.arrow.pyspark.enabled", "true") \
    .getOrCreate()

contract = statguard.DataContract.from_file("events.sg")
spark_df = spark.read.parquet("s3a://bucket/events/")

report = statguard.execute_spark(contract, spark_df)

# Drift between Spark DataFrames
today = spark.read.parquet("s3a://bucket/today/")
yesterday = spark.read.parquet("s3a://bucket/yesterday/")
report = statguard.execute_spark(contract, today, reference_spark_df=yesterday)
```

Works on: local, YARN, Kubernetes, Databricks, AWS EMR, Google Dataproc, Azure HDInsight.

---

## Why not just use pandera or Great Expectations?

You can — until the dataset is large, or you need drift detection, or you want one tool that covers files, Delta Lake, Iceberg, cloud storage, and SQL without gluing libraries together.

**100,000 rows × 4 columns, 5 checks — Apple M-series:**

| Tool | Best time | vs StatGuard |
|---|---|---|
| **StatGuard 0.1** | **2.0 ms** | baseline |
| Pure Python loops | 11.5 ms | 5.8× slower |
| pandera 0.31 (pandas) | 26.5 ms | 13× slower |
| Pydantic v2 (TypeAdapter bulk) | 43.5 ms | 22× slower |
| Pydantic v2 (row-by-row) | 46.2 ms | 23× slower |
| Great Expectations 1.18 | 50.4 ms | 25× slower |

> Pydantic allocates one Python object per row regardless of batch size. StatGuard never touches individual rows — it operates on entire Arrow columns.

See [BENCHMARKS.md](BENCHMARKS.md) for full methodology, scaling table, and reproduce steps.

**Feature comparison:**

| | Pydantic v2 | pandera | Great Expectations | WhyLogs | **StatGuard** |
|---|---|---|---|---|---|
| Performance | Row-by-row Python | Python/pandas | Python-heavy | Python | **Rust — 13–25× faster** |
| Schema / type validation | ✓ | ✓ | ✓ | ✗ | ✓ |
| Tabular quality rules | ✗ | ✓ | ✓ | ✗ | ✓ |
| Drift detection (PSI + KS) | ✗ | ✗ | ✗ | ✓ | ✓ |
| Anomaly detection | ✗ | ✗ | partial | partial | ✓ |
| Delta Lake (no Spark) | ✗ | ✗ | ✗ | ✗ | ✓ |
| Apache Iceberg (no Spark) | ✗ | ✗ | ✗ | ✗ | ✓ |
| Avro / ORC | ✗ | ✗ | partial | ✗ | ✓ |
| Streaming support | ✗ | ✗ | ✗ | partial | ✓ |
| Single contract DSL | ✗ | ✗ | ✗ | ✗ | ✓ |
| pip / uv install | ✓ | ✓ | ✓ | ✓ | ✓ |

---

## Format and connector compatibility

| | StatGuard | pandera | Great Expectations | Pydantic v2 |
|---|---|---|---|---|
| **Files** (Parquet, CSV, JSON, Avro, ORC, Arrow IPC) | ✓ native | ✓ via pandas | ✓ via pandas | ✗ load first |
| **Delta Lake** (no Spark) | ✓ | ✗ | ✗ | ✗ |
| **Apache Iceberg** (no Spark) | ✓ | ✗ | ✗ | ✗ |
| **Cloud** (S3, GCS, Azure) | ✓ | via extras | ✓ native | ✗ |
| **Spark DataFrames** | ✓ Arrow bridge | ✓ | ✓ native | ✗ |
| **SQL / warehouses** | 13 OSS connectors | via SQLAlchemy | 12 connectors | ✗ |
| **Kafka** (roadmap) | planned | ✗ | ✗ | ✗ |
| **Flink** (roadmap) | planned | ✗ | ✗ | ✗ |
| **Airflow** (roadmap) | planned | ✗ | partial | ✗ |

→ Full matrix: [docs/FORMAT_COMPATIBILITY.md](docs/FORMAT_COMPATIBILITY.md)

---

## DSL reference

```
dataset <name> {
    schema {
        <field>: <type>[, <constraint>]*
    }
    quality {
        [@<severity>:] <metric>(<field>) <op> <value>
    }
    stats {
        [@<severity>:] <field>.<stat> drift <op> <value>
    }
    anomalies {
        [@<severity>:] <fn>(<field>[, <arg>=<value>]*)
    }
    stream {              // optional — streaming window config
        window    = "5m"
        watermark = "30s"
        emit      = "on_window_close"
    }
}
```

### Types
`int` · `float` · `string` · `bool` · `date` · `datetime` · `bytes`

### Constraints

| Constraint | Example |
|---|---|
| `not_null` | `id: int, not_null` |
| `unique` | `email: string, unique` |
| `primary_key` | `id: int, primary_key` |
| `positive` / `negative` | `amount: float, positive` |
| `coerce` | `age: int, coerce` (type mismatch → warning, not blocking) |
| `regex=` | `email: string, regex="^[^@]+@[^@]+\.[^@]+$"` |
| `between(lo, hi)` | `age: int, between(0, 120)` |
| `min=` / `max=` | `score: float, min=0.0, max=1.0` |
| `len(min, max)` | `code: string, len(3, 10)` |
| `enum=[...]` | `status: string, enum=["A","B","C"]` |

### Quality metrics
`completeness` · `uniqueness` · `validity` · `consistency` · `non_null_rate`

### Drift stat functions
`mean` · `std` · `median` · `min` · `max` · `p05` · `p95` · `p99` · `p999`

PSI and KS statistic are always computed alongside every drift rule — no extra config needed.

### Anomaly functions

| Function | Description |
|---|---|
| `detect_outliers(col, method="iqr")` | IQR 1.5× rule or z-score > 3σ |
| `detect_duplicates(col)` | Exact duplicate detection |
| `detect_nulls(col)` | Null-value anomalies |
| `detect_cardinality_explosion(col)` | Sudden cardinality spike |
| `detect_pattern_breaks(col, pattern=...)` | Regex pattern consistency |

### Severity levels
`@blocking` · `@error` (default) · `@warning` · `@info`

`@blocking` violations abort further column checks and set `report.passed = False`.

---

## Python API

```python
import statguard

# Compile contract
contract = statguard.DataContract.from_dsl("...")       # from string
contract = statguard.DataContract.from_file("orders.sg")

# Execute (returns ValidationReport)
statguard.execute(contract, polars_df, reference=None)
statguard.execute_file(contract, path, reference_path=None)
statguard.execute_streaming(contract, path, batch_size=10_000)

# Lakehouse
statguard.execute_delta(contract, table_path, version=None,
                        reference_path=None, reference_version=None)
statguard.compare_delta_versions(contract, table_path, ref_v, cur_v=None)
statguard.execute_iceberg(contract, table_path, snapshot_id=None,
                          reference_snapshot=None)
statguard.list_iceberg_snapshots(table_path)

# Cloud, SQL, Spark
statguard.execute_cloud(contract, uri, reference_uri=None)
statguard.execute_sql(contract, connection_string, query, reference_query=None)
statguard.execute_spark(contract, spark_df, reference_spark_df=None)

# Utilities
statguard.validate_dsl(dsl_string)  # syntax check only

# ValidationReport
report.passed            # bool
report.health_score      # float [0, 1]
report.grade             # "A" / "B" / "C" / "D" / "F"
report.row_count         # int
report.violation_count   # int
report.duration_ms       # int
report.violations()      # list[dict]
report.drift_results()   # list[dict]
report.column_profiles() # list[dict]
report.to_json()
report.to_json_pretty()
report.to_prometheus()
report.summary()         # one-line string
```

---

## Report output

```json
{
  "id": "a1b2c3d4-...",
  "dataset": "orders",
  "executed_at": "2026-06-15T10:00:00Z",
  "duration_ms": 2,
  "row_count": 500000,
  "passed": true,
  "health": {
    "score": 0.972,
    "grade": "A",
    "schema_score": 0.980,
    "drift_score": 0.950
  },
  "violations": [
    {
      "column": "amount",
      "check": "outlier_detection",
      "severity": "Error",
      "message": "14 outlier(s) in 'amount' (method=iqr)",
      "row_indices": [142, 891, 3204]
    }
  ],
  "drift_results": [
    {
      "column": "amount",
      "stat": "mean",
      "reference_value": 84.20,
      "current_value": 91.50,
      "drift": 0.087,
      "threshold": 0.15,
      "psi": 0.012,
      "ks_stat": 0.041,
      "passed": true
    }
  ],
  "column_profiles": [
    {
      "name": "amount",
      "mean": 91.5,
      "std": 142.3,
      "p95": 310.0,
      "null_rate": 0.0,
      "distinct_count": 184291
    }
  ]
}
```

---

## Use cases

| Use case | How |
|---|---|
| **dbt / Airflow pipeline gate** | `statguard validate --fail-on-warning` in task |
| **ML feature drift monitor** | `stats { feature.mean drift < 0.05 }` + reference dataset |
| **Lakehouse quality layer** | `execute_delta()` / `execute_iceberg()` on every write |
| **Kafka / streaming quality** | `execute_streaming()` with micro-batch window |
| **Prometheus scraping** | `--format prometheus` or `report.to_prometheus()` |
| **CI data contract tests** | `statguard check` for DSL lint, `validate` for data |

---

## Architecture

```
statguard/
├── crates/
│   ├── statguard-core/       DSL (pest PEG grammar) → AST → compiler → ExecutionDag
│   ├── statguard-engine/     Rayon parallel executor — batch + streaming
│   ├── statguard-validators/ Type, null, regex, range, enum, uniqueness checks
│   ├── statguard-stats/      PSI, KS test, HyperLogLog profiler, percentile stats
│   ├── statguard-io/         Universal reader — auto-detects all formats
│   │                         • Parquet, CSV, JSON, IPC, Avro, ORC (local + cloud)
│   │                         • Delta Lake (pure Rust transaction log replay)
│   │                         • Apache Iceberg (v1/v2 metadata parsing, no Spark)
│   │                         • S3, GCS, Azure (Polars lazy, opt-in features)
│   │                         • SQL: PostgreSQL, MySQL, SQLite (pure Rust via sqlx)
│   │                         • StreamingBatcher, RowBuffer
│   ├── statguard-metrics/    ValidationReport, health scores, Prometheus output
│   └── statguard-py/         PyO3 bindings — Rust layer public API
└── python/
    ├── statguard/
    │   ├── __init__.py        Re-exports from both layers
    │   ├── _connectors.py     Cloud (S3/GCS/Azure), SQL (13 connectors), Spark
    │   └── _cli.py            CLI: validate, check commands
    └── docs/
        └── FORMAT_COMPATIBILITY.md
```

### Execution pipeline

```
DSL text  →  pest parser  →  DataContract AST
                                   │
                              Compiler::compile()
                                   │
                           raw DagNode list
                                   │
                    Optimizer: dedup → fuse null checks → cost-sort
                                   │
                           ExecutionDag (column-grouped)
                                   │
              ┌────────────────────┼──────────────────────────┐
     SchemaValidator         Rayon parallel            DriftEngine
     RuleEngine              per-column nodes          (PSI + KS)
                                   │
                           Profiler (always-on)
                                   │
                         ValidationReport
```

### Why it's fast

- **Columnar execution** — every check operates on an entire Arrow column, never row by row
- **Compiled DAG** — validation logic is a fixed execution plan, not interpreted rules at runtime
- **Cost-ordered checks** — `null` (cost 1) runs before `regex` (cost 4) before `uniqueness` (cost 5); cheap failures abort expensive work early
- **Rayon parallelism** — columns execute concurrently, scaling with core count
- **Zero-copy IO** — Arrow IPC and Parquet data never leaves the Arrow memory model
- **HyperLogLog** — O(1) memory, ~0.8% error rate for cardinality estimation on every column

---

## Dependencies and licensing

StatGuard is MIT licensed. All core dependencies use MIT, Apache-2.0, or BSD licenses.

**Note on PostgreSQL support:** Using `execute_sql()` with PostgreSQL requires `psycopg2` (LGPL-2.1 with exceptions), which adds an LGPL component to your application. See [LICENSES.md](LICENSES.md) for full compliance details and impact on binary distributions.

All optional features use OSI-approved open-source licenses only. Proprietary drivers (Oracle, SQL Server ODBC) are intentionally excluded.

→ Full license matrix: [LICENSES.md](LICENSES.md)

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and [AGENTS.md](AGENTS.md).

```bash
cargo test --workspace --exclude statguard
cargo clippy --workspace
cargo fmt --all
```

---

## License

MIT © 2026 [Georgi Mammen Mullassery](https://github.com/Mullassery)
