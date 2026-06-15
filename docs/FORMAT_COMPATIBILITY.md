# Format & Table Compatibility Comparison

How StatGuard's format support compares to Pydantic, pandera, and Great Expectations.

**Verified against:** StatGuard 0.1 · Pydantic 2.13 · pandera 0.31 · Great Expectations 1.18

---

## File formats

| Format | StatGuard | pandera | Great Expectations | Pydantic v2 |
|---|---|---|---|---|
| **Parquet** | ✓ native | ✓ via pandas/polars | ✓ via pandas or Spark | ✗ (load first) |
| **CSV / TSV** | ✓ native | ✓ via pandas/polars | ✓ via pandas or Spark | ✗ (load first) |
| **JSON / NDJSON** | ✓ native | ✓ via pandas/polars | ✓ via pandas or Spark | ✓ native (dicts) |
| **Arrow IPC** | ✓ native | ✓ via pyarrow→pandas | ✗ | ✗ (load first) |
| **Avro** | ✓ native | ✓ via fastavro→pandas | ✓ via Spark | ✗ (load first) |
| **ORC** | ✓ (opt-in) | ✓ via pyarrow→pandas | ✓ via Spark | ✗ (load first) |
| **Excel** | ✗ | ✓ via openpyxl→pandas | ✓ via pandas | ✗ (load first) |
| **HDF5** | ✗ | ✓ via pandas | ✗ | ✗ |

---

## Lakehouse table formats

| Format | StatGuard | pandera | Great Expectations | Pydantic v2 |
|---|---|---|---|---|
| **Delta Lake** | ✓ native (log replay) | ✗ | ✓ via PySpark + Delta | ✗ |
| **Apache Iceberg** | ✓ native (v1 + v2) | ✗ | ✓ via PySpark + Iceberg | ✗ |
| **Apache Hudi** | ✗ | ✗ | ✓ via Spark | ✗ |
| **Apache Hive** | ✗ | ✗ | ✓ via Spark | ✗ |

### Delta Lake — detail

| Feature | StatGuard | Great Expectations |
|---|---|---|
| Read current snapshot | ✓ | ✓ (Spark required) |
| Time-travel by version | ✓ | ✓ (Spark required) |
| Time-travel by timestamp | ✓ | ✓ (Spark required) |
| Two-version drift comparison | ✓ built-in | ✗ |
| Dependency | none (pure Rust log replay) | PySpark + `delta-spark` JAR |
| Local use (no cluster) | ✓ | ✗ |

### Apache Iceberg — detail

| Feature | StatGuard | Great Expectations |
|---|---|---|
| Read current snapshot | ✓ | ✓ (Spark required) |
| Time-travel by snapshot ID | ✓ | ✓ (Spark required) |
| Time-travel by timestamp | ✓ | ✓ (Spark required) |
| Named branch / tag reads | ✓ (`read_ref("main")`) | ✗ |
| Snapshot listing | ✓ (`list_iceberg_snapshots()`) | ✗ |
| Two-snapshot drift comparison | ✓ built-in | ✗ |
| Spec version | v1 + v2 | depends on Spark/Iceberg version |
| Dependency | none (pure Rust metadata parse) | PySpark + Iceberg JAR |
| Local use (no cluster) | ✓ | ✗ |

---

## Cloud storage

| Storage | StatGuard | pandera | Great Expectations | Pydantic v2 |
|---|---|---|---|---|
| Local filesystem | ✓ | ✓ | ✓ | ✗ |
| Amazon S3 | ✗ ¹ | ✓ via pandas/s3fs | ✓ native datasource | ✗ |
| Google Cloud Storage | ✗ ¹ | ✓ via pandas/gcsfs | ✓ native datasource | ✗ |
| Azure Blob Storage | ✗ ¹ | ✓ via pandas/adlfs | ✓ native datasource | ✗ |
| DBFS (Databricks) | ✗ | ✗ | ✓ native datasource | ✗ |

¹ Cloud storage for StatGuard is on the roadmap. Workaround: download to local disk or mount via s3fs/gcsfuse.

---

## SQL / warehouse connectors

| Backend | StatGuard | pandera | Great Expectations | Pydantic v2 |
|---|---|---|---|---|
| PostgreSQL | ✗ | ✓ via pandas+sqlalchemy | ✓ native datasource | ✗ |
| MySQL / Aurora | ✗ | ✓ via pandas+sqlalchemy | ✓ native datasource | ✗ |
| Snowflake | ✗ | ✓ via pandas+sqlalchemy | ✓ native datasource | ✗ |
| BigQuery | ✗ | ✓ via pandas+bigquery | ✓ native datasource | ✗ |
| Redshift | ✗ | ✓ via pandas+sqlalchemy | ✓ native datasource | ✗ |
| Databricks SQL | ✗ | ✗ | ✓ native datasource | ✗ |
| Microsoft Fabric | ✗ | ✗ | ✓ native datasource | ✗ |
| SQLite | ✗ | ✓ via pandas | ✓ native datasource | ✗ |

---

## Compute engines

| Engine | StatGuard | pandera | Great Expectations | Pydantic v2 |
|---|---|---|---|---|
| Polars | ✓ native (primary) | ✓ experimental | ✗ | ✗ |
| Pandas | ✓ (via Polars convert) | ✓ primary | ✓ primary | ✗ |
| Apache Spark | ✗ | ✓ (separate install) | ✓ native | ✗ |
| Dask | ✗ | ✗ | ✗ | ✗ |
| Ray | ✗ | ✗ | ✗ | ✗ |

---

## Auto-detection

StatGuard's `DataReader.read_file()` and `execute_file()` detect format automatically
from the path — no format parameter needed:

```python
# These all work with the same API call:
statguard.execute_file(contract, "data.parquet")
statguard.execute_file(contract, "data.csv")
statguard.execute_file(contract, "data.avro")
statguard.execute_file(contract, "data.json")
statguard.execute_file(contract, "/path/to/delta_table/")    # has _delta_log/
statguard.execute_file(contract, "/path/to/iceberg_table/")  # has metadata/
```

Other libraries require explicit format-specific loading code before validation.

---

## Summary

| Criterion | StatGuard | pandera | Great Expectations | Pydantic v2 |
|---|---|---|---|---|
| Formats supported (native) | 8 | 0 ² | 0 ² | 0 ² |
| Delta Lake (no Spark) | ✓ | ✗ | ✗ | ✗ |
| Iceberg (no Spark) | ✓ | ✗ | ✗ | ✗ |
| Auto-detect format | ✓ | ✗ | ✗ | ✗ |
| Cloud storage | roadmap | via extras | ✓ | ✗ |
| SQL / warehouses | roadmap | via extras | ✓ | ✗ |
| Spark integration | roadmap | via extras | ✓ | ✗ |

² pandera, GX, and Pydantic all require external libraries to load data
into a DataFrame/dict before validation can begin. StatGuard reads
and validates in a single pipeline call.
