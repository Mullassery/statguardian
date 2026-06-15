/// SQL database reader — pure-Rust drivers, MIT/Apache-2.0 licensed.
///
/// Uses `sqlx` for direct database connections with no ODBC/JDBC dependency.
///
/// # Supported databases (Rust layer)
///
/// | Database | URL scheme | License |
/// |---|---|---|
/// | PostgreSQL / CockroachDB / TimescaleDB | `postgresql://` | PostgreSQL (OSS) |
/// | MySQL / MariaDB / PlanetScale | `mysql://` | GPL-2 / Apache-2.0 |
/// | SQLite | `sqlite:///path` | Public Domain |
///
/// # Extended support (Python layer)
///
/// For BigQuery, Snowflake, Redshift, Databricks, ClickHouse and other
/// warehouses, use the Python `execute_sql()` API which routes through
/// `connectorx` (MIT) or `polars.read_database_uri()`:
///
/// ```python
/// report = statguard.execute_sql(
///     contract,
///     connection="bigquery://project/dataset",
///     query="SELECT * FROM events WHERE date = '2026-06-15'",
/// )
/// ```
///
/// # Intentionally excluded
///
/// Oracle and SQL Server are **not** supported in the Rust layer because their
/// official drivers require proprietary ODBC/JDBC components (non-open-source).
/// If you need them, use the Python `execute_sql()` API with connectorx and
/// the appropriate ODBC driver installed separately.

use polars::prelude::*;
use crate::{IoError, IoResult};

/// Identifies the SQL backend from a connection URL scheme.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlBackend {
    Postgres,
    Mysql,
    Sqlite,
    /// Extended backends handled by the Python layer (connectorx).
    PythonLayer(String),
}

impl SqlBackend {
    pub fn from_url(url: &str) -> Self {
        let scheme = url.splitn(2, "://").next().unwrap_or("").to_lowercase();
        match scheme.as_str() {
            "postgresql" | "postgres" | "pg" => SqlBackend::Postgres,
            "mysql" | "mariadb" => SqlBackend::Mysql,
            "sqlite" => SqlBackend::Sqlite,
            // These are handled in the Python layer
            "bigquery" | "snowflake" | "redshift" | "redshift+psycopg2"
            | "databricks" | "clickhouse" | "duckdb" | "mongodb" => {
                SqlBackend::PythonLayer(scheme)
            }
            other => SqlBackend::PythonLayer(other.to_string()),
        }
    }

    pub fn is_rust_native(&self) -> bool {
        !matches!(self, SqlBackend::PythonLayer(_))
    }
}

/// Reads the result of a SQL query into a Polars DataFrame.
///
/// Requires one of: `sql-postgres`, `sql-mysql`, `sql-sqlite` feature flags.
/// For other backends call the Python `execute_sql()` wrapper.
pub struct SqlReader;

impl SqlReader {
    /// Execute `query` against `connection_url` and return a DataFrame.
    ///
    /// This is a synchronous wrapper — it creates an internal Tokio runtime.
    pub fn read(query: &str, connection_url: &str) -> IoResult<DataFrame> {
        match SqlBackend::from_url(connection_url) {
            SqlBackend::PythonLayer(scheme) => Err(IoError::UnsupportedFormat(format!(
                "'{scheme}' is not supported in the Rust SQL layer. \
                 Use statguard.execute_sql() in Python with connectorx installed."
            ))),
            SqlBackend::Postgres => {
                #[cfg(feature = "sql-postgres")]
                return Self::read_postgres(query, connection_url);
                #[cfg(not(feature = "sql-postgres"))]
                Err(IoError::UnsupportedFormat(
                    "PostgreSQL support requires feature `sql-postgres`. \
                     Rebuild with: cargo build --features sql-postgres".into()
                ))
            }
            SqlBackend::Mysql => {
                #[cfg(feature = "sql-mysql")]
                return Self::read_mysql(query, connection_url);
                #[cfg(not(feature = "sql-mysql"))]
                Err(IoError::UnsupportedFormat(
                    "MySQL support requires feature `sql-mysql`. \
                     Rebuild with: cargo build --features sql-mysql".into()
                ))
            }
            SqlBackend::Sqlite => {
                #[cfg(feature = "sql-sqlite")]
                return Self::read_sqlite(query, connection_url);
                #[cfg(not(feature = "sql-sqlite"))]
                Err(IoError::UnsupportedFormat(
                    "SQLite support requires feature `sql-sqlite`. \
                     Rebuild with: cargo build --features sql-sqlite".into()
                ))
            }
        }
    }

    #[cfg(feature = "sql-postgres")]
    fn read_postgres(query: &str, url: &str) -> IoResult<DataFrame> {
        use sqlx::postgres::PgPool;
        let rt = build_runtime()?;
        rt.block_on(async {
            let pool = PgPool::connect(url).await
                .map_err(|e| IoError::ReadError { path: url.to_string(), msg: e.to_string() })?;
            fetch_to_dataframe::<sqlx::Postgres, _>(&pool, query).await
                .map_err(|e| IoError::ReadError { path: url.to_string(), msg: e.to_string() })
        })
    }

    #[cfg(feature = "sql-mysql")]
    fn read_mysql(query: &str, url: &str) -> IoResult<DataFrame> {
        use sqlx::mysql::MySqlPool;
        let rt = build_runtime()?;
        rt.block_on(async {
            let pool = MySqlPool::connect(url).await
                .map_err(|e| IoError::ReadError { path: url.to_string(), msg: e.to_string() })?;
            fetch_to_dataframe::<sqlx::MySql, _>(&pool, query).await
                .map_err(|e| IoError::ReadError { path: url.to_string(), msg: e.to_string() })
        })
    }

    #[cfg(feature = "sql-sqlite")]
    fn read_sqlite(query: &str, url: &str) -> IoResult<DataFrame> {
        use sqlx::sqlite::SqlitePool;
        let rt = build_runtime()?;
        rt.block_on(async {
            let pool = SqlitePool::connect(url).await
                .map_err(|e| IoError::ReadError { path: url.to_string(), msg: e.to_string() })?;
            fetch_to_dataframe::<sqlx::Sqlite, _>(&pool, query).await
                .map_err(|e| IoError::ReadError { path: url.to_string(), msg: e.to_string() })
        })
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

#[cfg(any(feature = "sql-postgres", feature = "sql-mysql", feature = "sql-sqlite"))]
fn build_runtime() -> IoResult<tokio::runtime::Runtime> {
    tokio::runtime::Runtime::new()
        .map_err(|e| IoError::ReadError { path: "runtime".into(), msg: e.to_string() })
}

#[cfg(any(feature = "sql-postgres", feature = "sql-mysql", feature = "sql-sqlite"))]
async fn fetch_to_dataframe<DB, E>(
    executor: E,
    query: &str,
) -> Result<DataFrame, sqlx::Error>
where
    DB: sqlx::Database,
    E: sqlx::Executor<'_, Database = DB> + Copy,
    for<'r> sqlx::query::Query<'r, DB, DB::Arguments<'r>>: sqlx::Execute<'r, DB>,
{
    use sqlx::Row;

    let rows: Vec<DB::Row> = sqlx::query(query).fetch_all(executor).await?;
    if rows.is_empty() {
        return Ok(DataFrame::default());
    }

    // Build column vectors from row data
    let col_names: Vec<&str> = rows[0].columns().iter().map(|c| c.name()).collect();
    let mut columns: Vec<Vec<Option<String>>> = vec![Vec::with_capacity(rows.len()); col_names.len()];

    for row in &rows {
        for (i, _col) in col_names.iter().enumerate() {
            // Cast every column to String for universal compatibility.
            // Callers can cast to typed columns using Polars lazy API.
            let val: Option<String> = row.try_get::<Option<String>, _>(i).ok().flatten();
            columns[i].push(val);
        }
    }

    let series: Vec<Column> = col_names.iter().zip(columns)
        .map(|(name, vals)| Series::new((*name).into(), vals).into_column())
        .collect();

    DataFrame::new(series).map_err(|e| sqlx::Error::Decode(Box::new(
        std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_detection() {
        assert_eq!(SqlBackend::from_url("postgresql://localhost/db"), SqlBackend::Postgres);
        assert_eq!(SqlBackend::from_url("postgres://localhost/db"),   SqlBackend::Postgres);
        assert_eq!(SqlBackend::from_url("mysql://localhost/db"),      SqlBackend::Mysql);
        assert_eq!(SqlBackend::from_url("sqlite:///tmp/test.db"),     SqlBackend::Sqlite);
        assert!(!SqlBackend::from_url("bigquery://project/ds").is_rust_native());
        assert!(!SqlBackend::from_url("snowflake://user@account/db").is_rust_native());
        assert!(!SqlBackend::from_url("clickhouse://localhost/db").is_rust_native());
    }

    #[test]
    fn test_unsupported_backend_returns_helpful_error() {
        let result = SqlReader::read("SELECT 1", "bigquery://my-project/my-dataset");
        match result {
            Err(IoError::UnsupportedFormat(msg)) => {
                assert!(msg.contains("execute_sql()"), "error should mention Python API: {msg}");
            }
            other => panic!("expected UnsupportedFormat, got {other:?}"),
        }
    }
}
