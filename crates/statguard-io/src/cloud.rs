/// Cloud storage reader — S3, GCS, Azure Blob Storage.
///
/// Uses Polars' lazy columnar readers which delegate to the Apache Arrow
/// `object_store` crate (Apache-2.0). No proprietary ODBC or SDK required.
///
/// # Supported URI schemes
///
/// | Scheme | Backend | Feature flag |
/// |--------|---------|--------------|
/// | `s3://` / `s3a://` | AWS S3, MinIO, Ceph, Cloudflare R2 | `s3` |
/// | `gs://` / `gcs://` | Google Cloud Storage | `gcs` |
/// | `az://` / `abfss://` / `adl://` | Azure Blob / ADLS Gen2 | `azure` |
///
/// # Authentication
///
/// Credentials are read from standard environment variables at runtime:
///
/// **S3**: `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_DEFAULT_REGION`
/// **GCS**: `GOOGLE_APPLICATION_CREDENTIALS` (path to service-account JSON)
/// **Azure**: `AZURE_STORAGE_ACCOUNT`, `AZURE_STORAGE_ACCESS_KEY`
///
/// Instance profiles, workload identity, and Managed Identity are also
/// picked up automatically when running on EC2/GCE/Azure VMs.
///
/// # Glob patterns
///
/// Polars lazy readers support glob patterns on cloud storage:
///
/// ```text
/// s3://my-bucket/events/year=2026/month=06/*.parquet
/// gs://my-bucket/data/**/*.parquet
/// ```

use polars::prelude::*;
use crate::{IoError, IoResult};

/// Returns `true` if `path` is a cloud storage URI handled by this module.
pub fn is_cloud_uri(path: &str) -> bool {
    path.starts_with("s3://")
        || path.starts_with("s3a://")
        || path.starts_with("s3n://")
        || path.starts_with("gs://")
        || path.starts_with("gcs://")
        || path.starts_with("az://")
        || path.starts_with("abfss://")
        || path.starts_with("adl://")
        || path.starts_with("wasbs://")
}

/// Reads a cloud-stored file or glob pattern into a Polars DataFrame.
///
/// Format is inferred from the file extension in the URI.
/// For partitioned datasets (directories), use a glob pattern.
pub struct CloudReader;

impl CloudReader {
    /// Auto-detect format and read from a cloud URI or glob pattern.
    pub fn read(uri: &str) -> IoResult<DataFrame> {
        // Determine format from the URI path, stripping query strings
        let path = uri.split('?').next().unwrap_or(uri);
        // Strip trailing slash, get extension of last segment
        let ext = path
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .and_then(|seg| {
                // Handle glob patterns: treat *.parquet as parquet
                let seg = seg.trim_start_matches('*').trim_start_matches('.');
                seg.rsplit('.').next()
            })
            .unwrap_or("");

        match ext {
            "parquet"            => Self::read_parquet(uri),
            "csv" | "tsv"        => Self::read_csv(uri),
            "json" | "ndjson"    => Self::read_ndjson(uri),
            "ipc" | "arrow"      => Self::read_ipc(uri),
            // Glob or directory — assume Parquet (most common cloud format)
            _ if uri.ends_with('/') || uri.contains('*') => Self::read_parquet(uri),
            _ => Err(IoError::UnsupportedFormat(format!(
                "cannot infer format from cloud URI '{uri}' — use read_cloud_parquet/csv/json explicitly"
            ))),
        }
    }

    /// Read Parquet from cloud (supports glob patterns and partitioned datasets).
    pub fn read_parquet(uri: &str) -> IoResult<DataFrame> {
        LazyFrame::scan_parquet(uri, ScanArgsParquet::default())
            .map_err(|e| IoError::ReadError { path: uri.to_string(), msg: e.to_string() })?
            .collect()
            .map_err(IoError::Polars)
    }

    /// Read CSV from cloud.
    pub fn read_csv(uri: &str) -> IoResult<DataFrame> {
        LazyCsvReader::new(uri)
            .with_infer_schema_length(Some(1000))
            .finish()
            .map_err(|e| IoError::ReadError { path: uri.to_string(), msg: e.to_string() })?
            .collect()
            .map_err(IoError::Polars)
    }

    /// Read newline-delimited JSON (NDJSON) from cloud.
    pub fn read_ndjson(uri: &str) -> IoResult<DataFrame> {
        LazyJsonLineReader::new(uri)
            .finish()
            .map_err(|e| IoError::ReadError { path: uri.to_string(), msg: e.to_string() })?
            .collect()
            .map_err(IoError::Polars)
    }

    /// Read Arrow IPC from cloud.
    pub fn read_ipc(uri: &str) -> IoResult<DataFrame> {
        LazyFrame::scan_ipc(uri, ScanArgsIpc::default())
            .map_err(|e| IoError::ReadError { path: uri.to_string(), msg: e.to_string() })?
            .collect()
            .map_err(IoError::Polars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_cloud_uri() {
        assert!(is_cloud_uri("s3://bucket/file.parquet"));
        assert!(is_cloud_uri("s3a://bucket/data/*.parquet"));
        assert!(is_cloud_uri("gs://bucket/data/"));
        assert!(is_cloud_uri("gcs://bucket/file.csv"));
        assert!(is_cloud_uri("az://container/blob.parquet"));
        assert!(is_cloud_uri("abfss://container@account.dfs.core.windows.net/file.parquet"));
        assert!(!is_cloud_uri("/local/path/file.parquet"));
        assert!(!is_cloud_uri("file:///local/file.parquet"));
        assert!(!is_cloud_uri("relative/path.csv"));
    }

    // The routing test below exercises Polars' lazy reader which requires a
    // cloud feature flag at compile time. Only run when at least one is enabled.
    #[cfg(any(feature = "s3", feature = "gcs", feature = "azure"))]
    #[test]
    fn test_format_inference_from_uri() {
        let uri = "s3://test-bucket/data.parquet";
        let result = CloudReader::read(uri);
        match result {
            Err(IoError::UnsupportedFormat(_)) => panic!("should not be UnsupportedFormat"),
            _ => {}
        }
    }
}
