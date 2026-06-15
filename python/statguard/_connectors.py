"""
StatGuard connector helpers — SQL databases, cloud warehouses, and Spark.

All connectors use open-source drivers only (MIT / Apache-2.0 / BSD).
Proprietary ODBC/JDBC components (Oracle, SQL Server native drivers) are
intentionally excluded. See LICENSES section below for details.

LICENSES
--------
connectorx  — MIT
polars      — MIT
pyarrow     — Apache-2.0
pyspark     — Apache-2.0
psycopg2    — LGPL-2.1 (or psycopg2-binary for convenience)
PyMySQL     — MIT
sqlalchemy  — MIT

CONNECTORS NOT INCLUDED (proprietary drivers required)
------------------------------------------------------
Oracle      — requires Oracle Instant Client (proprietary)
SQL Server  — requires Microsoft ODBC Driver (proprietary on Linux/macOS)
"""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    pass

# ── SQL / warehouses ──────────────────────────────────────────────────────────

# Tested open-source connection URL schemes:
_OSS_SCHEMES = {
    # Core relational (pure-Python or pure-Rust drivers)
    "postgresql", "postgres",
    "mysql", "mysql+pymysql", "mariadb",
    "sqlite",

    # Open-source OLAP / warehouses (Apache-2.0 / MIT drivers)
    "bigquery",                        # google-cloud-bigquery (Apache-2.0)
    "snowflake",                       # snowflake-connector-python (Apache-2.0)
    "redshift", "redshift+psycopg2",   # amazon-redshift-python-driver (Apache-2.0)
    "databricks", "databricks+connector",  # databricks-sql-connector (Apache-2.0)
    "clickhouse", "clickhouse+http", "clickhouse+native",  # clickhouse-driver (MIT)
    "duckdb",                          # duckdb (MIT)
    "cockroachdb",                     # same as postgres wire protocol
    "trino", "presto",                 # trino-python-client (Apache-2.0)

    # S3-compatible (for Athena-like query services)
    "awsathena+rest",                  # PyAthena (MIT) — Athena via S3
}

# Schemes we explicitly decline with a helpful message
_PROPRIETARY_SCHEMES = {
    "oracle", "oracle+cx_oracle": (
        "Oracle requires the proprietary Oracle Instant Client. "
        "StatGuard only includes open-source connectors. "
        "See: https://www.oracle.com/database/technologies/instant-client.html"
    ),
    "mssql", "mssql+pyodbc", "mssql+pymssql": (
        "SQL Server's official ODBC driver (msodbcsql) is proprietary on Linux/macOS. "
        "StatGuard only includes open-source connectors. "
        "Workaround: export your data to Parquet and use statguard.execute_file()."
    ),
}


def _check_oss(connection_string: str) -> None:
    scheme = connection_string.split("://")[0].lower()
    for prop_scheme, msg in _PROPRIETARY_SCHEMES.items():
        if scheme == prop_scheme:
            raise ValueError(f"StatGuard — proprietary connector declined: {msg}")


def _read_sql_to_polars(connection_string: str, query: str):
    """
    Read a SQL query result into a Polars DataFrame.

    Tries (in order):
    1. polars.read_database_uri()  — uses connectorx (MIT) under the hood
    2. polars.read_database()      — uses adbc or SQLAlchemy
    3. pandas.read_sql() + pl.from_pandas()  — widest compatibility fallback
    """
    import polars as pl

    # Strategy 1: connectorx via polars (fastest, zero-copy Arrow)
    try:
        return pl.read_database_uri(query=query, uri=connection_string)
    except Exception as cx_err:
        pass

    # Strategy 2: ADBC / SQLAlchemy via polars.read_database
    try:
        import sqlalchemy
        engine = sqlalchemy.create_engine(connection_string)
        with engine.connect() as conn:
            return pl.read_database(query=query, connection=conn)
    except Exception as sa_err:
        pass

    # Strategy 3: pandas fallback
    try:
        import pandas as pd
        import sqlalchemy
        engine = sqlalchemy.create_engine(connection_string)
        return pl.from_pandas(pd.read_sql(query, engine))
    except ImportError:
        raise ImportError(
            "No SQL connector found. Install one:\n"
            "  pip install connectorx          # fastest, MIT\n"
            "  pip install sqlalchemy          # widest compat, MIT\n"
            "  pip install 'polars[adbc]'      # ADBC drivers"
        )


def execute_sql(contract, connection_string: str, query: str,
                reference_query: str | None = None):
    """
    Execute a SQL query and validate the result with a StatGuard contract.

    Open-source connectors only. Requires one of:
        pip install connectorx          # MIT — recommended (fastest)
        pip install sqlalchemy          # MIT — widest compatibility
        pip install 'polars[adbc]'      # ADBC drivers

    Supported databases
    -------------------
    PostgreSQL / CockroachDB / TimescaleDB  postgresql://user:pass@host:5432/db
    MySQL / MariaDB / PlanetScale           mysql+pymysql://user:pass@host:3306/db
    SQLite                                  sqlite:///path/to/file.db
    Google BigQuery (Apache-2.0)            bigquery://project/dataset
    Snowflake (Apache-2.0 driver)           snowflake://user:pass@account/db
    Amazon Redshift (Apache-2.0 driver)     redshift+psycopg2://user:pass@host:5439/db
    Databricks SQL (Apache-2.0 driver)      databricks+connector://token@host:443/db
    ClickHouse (MIT driver)                 clickhouse+http://user:pass@host:8123/db
    DuckDB (MIT)                            duckdb:///path/to/file.db
    Trino / Presto (Apache-2.0)             trino://user@host:8080/catalog/schema

    Args:
        contract:           A compiled DataContract.
        connection_string:  SQLAlchemy-style connection URL.
        query:              SQL SELECT query to validate.
        reference_query:    Optional SQL query for drift reference dataset.

    Returns:
        ValidationReport
    """
    _check_oss(connection_string)

    df = _read_sql_to_polars(connection_string, query)
    ref_df = _read_sql_to_polars(connection_string, reference_query) \
             if reference_query else None

    from statguard._statguard import execute as _execute
    return _execute(contract, df, ref_df)


# ── Spark ──────────────────────────────────────────────────────────────────────

def execute_spark(contract, spark_df, reference_spark_df=None):
    """
    Validate a PySpark DataFrame with a StatGuard contract.

    Converts Spark → Apache Arrow → Polars in-process using the Arrow
    columnar protocol. No data serialisation to Python dicts.

    On-cluster (Databricks, EMR, GKE): the Arrow collection happens on
    the driver node — data is already local when this function is called.

    Requires PySpark 3.0+ with Arrow enabled:
        spark.conf.set("spark.sql.execution.arrow.pyspark.enabled", "true")

    Args:
        contract:             A compiled DataContract.
        spark_df:             pyspark.sql.DataFrame to validate.
        reference_spark_df:   Optional PySpark DataFrame for drift reference.

    Returns:
        ValidationReport
    """
    import polars as pl

    def _spark_to_polars(sdf):
        # Best path: collect as Arrow batches (zero pandas overhead)
        try:
            import pyarrow as pa
            # PySpark 3.3+ internal API — widely used in OSS tooling
            batches = sdf._collect_as_arrow()
            return pl.from_arrow(pa.Table.from_batches(batches))
        except AttributeError:
            pass

        # PySpark 3.0+ toPandas with Arrow enabled
        try:
            pandas_df = sdf.toPandas()
            return pl.from_pandas(pandas_df)
        except Exception as e:
            raise RuntimeError(
                f"Could not convert Spark DataFrame to Polars: {e}\n"
                "Ensure spark.sql.execution.arrow.pyspark.enabled = true"
            ) from e

    df = _spark_to_polars(spark_df)
    ref_df = _spark_to_polars(reference_spark_df) if reference_spark_df is not None else None

    from statguard._statguard import execute as _execute
    return _execute(contract, df, ref_df)


def execute_cloud(contract, uri: str, reference_uri: str | None = None):
    """
    Validate a file stored on cloud object storage (S3, GCS, Azure).

    Format is auto-detected from the URI extension.
    Glob patterns are supported for partitioned datasets.

    Requires appropriate credentials in environment variables:
        S3:    AWS_ACCESS_KEY_ID + AWS_SECRET_ACCESS_KEY
        GCS:   GOOGLE_APPLICATION_CREDENTIALS
        Azure: AZURE_STORAGE_ACCOUNT + AZURE_STORAGE_ACCESS_KEY

    Also works with:
        - AWS IAM instance profiles (on EC2)
        - GCP Workload Identity (on GKE)
        - Azure Managed Identity (on Azure VMs)
        - MinIO / Ceph / Cloudflare R2 (S3-compatible, set AWS_ENDPOINT_URL)

    Args:
        contract:      A compiled DataContract.
        uri:           Cloud URI. Examples:
                         s3://bucket/data/events.parquet
                         s3://bucket/events/year=2026/*.parquet
                         gs://bucket/data/
                         az://container/file.parquet
                         abfss://container@account.dfs.core.windows.net/data/
        reference_uri: Optional cloud URI for drift reference.

    Returns:
        ValidationReport
    """
    from statguard._statguard import execute_file as _execute_file
    return _execute_file(contract, uri, reference_uri)
