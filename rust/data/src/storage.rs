//! Versioned on-disk storage for validated market data.

use crate::provider::DataError;
use crate::types::Bar;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DatasetManifest {
    pub dataset_version: String,
    pub symbol: String,
    pub start_date: String,
    pub end_date: String,
    pub bar_count: usize,
    pub source: String,
}

pub fn dataset_path(root: impl AsRef<Path>, dataset_version: &str, symbol: &str) -> PathBuf {
    root.as_ref()
        .join(dataset_version)
        .join(format!("{}.json", symbol))
}

pub fn manifest_path(root: impl AsRef<Path>, dataset_version: &str, symbol: &str) -> PathBuf {
    root.as_ref()
        .join(dataset_version)
        .join(format!("{}.manifest.json", symbol))
}

pub fn write_dataset(
    root: impl AsRef<Path>,
    manifest: &DatasetManifest,
    bars: &[Bar],
) -> Result<PathBuf, DataError> {
    if bars.is_empty() {
        return Err(DataError::ValidationError(format!(
            "Cannot persist empty dataset for '{}'",
            manifest.symbol
        )));
    }

    let data_path = dataset_path(&root, &manifest.dataset_version, &manifest.symbol);
    let metadata_path = manifest_path(&root, &manifest.dataset_version, &manifest.symbol);
    let parent = data_path.parent().ok_or_else(|| {
        DataError::FetchError(format!(
            "Dataset path has no parent: {}",
            data_path.display()
        ))
    })?;
    fs::create_dir_all(parent)
        .map_err(|e| DataError::FetchError(format!("Failed to create dataset directory: {}", e)))?;

    let data = serde_json::to_vec_pretty(bars)
        .map_err(|e| DataError::FetchError(format!("Failed to serialize bars: {}", e)))?;
    fs::write(&data_path, data)
        .map_err(|e| DataError::FetchError(format!("Failed to write dataset: {}", e)))?;

    let metadata = serde_json::to_vec_pretty(manifest)
        .map_err(|e| DataError::FetchError(format!("Failed to serialize manifest: {}", e)))?;
    fs::write(&metadata_path, metadata)
        .map_err(|e| DataError::FetchError(format!("Failed to write manifest: {}", e)))?;

    Ok(data_path)
}

pub fn read_dataset(
    root: impl AsRef<Path>,
    dataset_version: &str,
    symbol: &str,
) -> Result<Vec<Bar>, DataError> {
    let path = dataset_path(root, dataset_version, symbol);
    let content = fs::read(&path).map_err(|e| {
        DataError::FetchError(format!("Failed to read dataset {}: {}", path.display(), e))
    })?;
    serde_json::from_slice(&content)
        .map_err(|e| DataError::ParseError(format!("Invalid dataset JSON: {}", e)))
}

pub fn read_manifest(
    root: impl AsRef<Path>,
    dataset_version: &str,
    symbol: &str,
) -> Result<DatasetManifest, DataError> {
    let path = manifest_path(root, dataset_version, symbol);
    let content = fs::read(&path).map_err(|e| {
        DataError::FetchError(format!("Failed to read manifest {}: {}", path.display(), e))
    })?;
    serde_json::from_slice(&content)
        .map_err(|e| DataError::ParseError(format!("Invalid manifest JSON: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Bar, Timestamp};
    use tempfile::tempdir;

    #[test]
    fn round_trips_versioned_dataset() {
        let dir = tempdir().unwrap();
        let bars = vec![Bar::same_bar(Timestamp(1), 10.0, 11.0, 9.0, 10.5, 100)];
        let manifest = DatasetManifest {
            dataset_version: "ds_test".to_string(),
            symbol: "TEST".to_string(),
            start_date: "1970-01-01".to_string(),
            end_date: "1970-01-01".to_string(),
            bar_count: bars.len(),
            source: "fixture".to_string(),
        };

        write_dataset(dir.path(), &manifest, &bars).unwrap();
        assert_eq!(read_dataset(dir.path(), "ds_test", "TEST").unwrap(), bars);
        assert_eq!(
            read_manifest(dir.path(), "ds_test", "TEST").unwrap(),
            manifest
        );
    }
}
