//! Versioned on-disk storage for validated market data.

use crate::provider::DataError;
use crate::types::Bar;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DatasetManifest {
    pub dataset_version: String,
    pub symbol: String,
    pub start_date: String,
    pub end_date: String,
    pub bar_count: usize,
    pub source: String,
    #[serde(default)]
    pub content_sha256: String,
}

pub fn validate_storage_id(id: &str) -> Result<(), DataError> {
    if !quant_config::valid_storage_id(id) {
        return Err(DataError::ValidationError(format!(
            "Invalid storage identifier: {id}"
        )));
    }
    Ok(())
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
    validate_storage_id(&manifest.dataset_version)?;
    validate_storage_id(&manifest.symbol)?;
    if bars.is_empty() {
        return Err(DataError::ValidationError(format!(
            "Cannot persist empty dataset for '{}'",
            manifest.symbol
        )));
    }

    crate::validate_bars_monotonic_and_sound(bars)?;
    if manifest.bar_count != bars.len() {
        return Err(DataError::ValidationError(
            "Manifest row count differs from bars".into(),
        ));
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
    let mut sealed = manifest.clone();
    sealed.content_sha256 = format!("{:x}", Sha256::digest(&data));
    let metadata = serde_json::to_vec_pretty(&sealed)
        .map_err(|e| DataError::FetchError(format!("Failed to serialize manifest: {}", e)))?;
    // Never rewrite a version used by an existing model. The manifest is published last;
    // readers fail closed while an incomplete write has no valid manifest.
    write_new(&data_path, &data)?;
    if let Err(error) = write_new(&metadata_path, &metadata) {
        let _ = fs::remove_file(&data_path);
        return Err(error);
    }

    Ok(data_path)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), DataError> {
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|e| DataError::FetchError(format!("Cannot create {}: {e}. Use a new dataset_version; existing data is never overwritten", path.display())))?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(DataError::FetchError(format!(
            "Failed to persist dataset: {error}"
        )));
    }
    Ok(())
}

pub fn read_dataset(
    root: impl AsRef<Path>,
    dataset_version: &str,
    symbol: &str,
) -> Result<Vec<Bar>, DataError> {
    validate_storage_id(dataset_version)?;
    validate_storage_id(symbol)?;
    let path = dataset_path(&root, dataset_version, symbol);
    let content = fs::read(&path).map_err(|e| {
        DataError::FetchError(format!("Failed to read dataset {}: {}", path.display(), e))
    })?;
    let manifest = read_manifest(root, dataset_version, symbol)?;
    if manifest.dataset_version != dataset_version
        || manifest.symbol != symbol
        || manifest.content_sha256 != format!("{:x}", Sha256::digest(&content))
    {
        return Err(DataError::ValidationError(
            "Market dataset integrity mismatch; ingest a new dataset version".into(),
        ));
    }
    let bars: Vec<Bar> = serde_json::from_slice(&content)
        .map_err(|e| DataError::ParseError(format!("Invalid dataset JSON: {}", e)))?;
    crate::validate_bars_monotonic_and_sound(&bars)?;
    if manifest.bar_count != bars.len() {
        return Err(DataError::ValidationError(
            "Market row count differs from manifest".into(),
        ));
    }
    Ok(bars)
}

pub fn read_manifest(
    root: impl AsRef<Path>,
    dataset_version: &str,
    symbol: &str,
) -> Result<DatasetManifest, DataError> {
    validate_storage_id(dataset_version)?;
    validate_storage_id(symbol)?;
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
        let mut manifest = DatasetManifest {
            dataset_version: "ds_test".to_string(),
            symbol: "TEST".to_string(),
            start_date: "1970-01-01".to_string(),
            end_date: "1970-01-01".to_string(),
            bar_count: bars.len(),
            source: "fixture".to_string(),
            content_sha256: String::new(),
        };

        write_dataset(dir.path(), &manifest, &bars).unwrap();
        assert_eq!(read_dataset(dir.path(), "ds_test", "TEST").unwrap(), bars);
        manifest.content_sha256 = format!(
            "{:x}",
            Sha256::digest(fs::read(dataset_path(dir.path(), "ds_test", "TEST")).unwrap())
        );
        assert_eq!(
            read_manifest(dir.path(), "ds_test", "TEST").unwrap(),
            manifest
        );
        assert!(write_dataset(dir.path(), &manifest, &bars).is_err());
        assert_eq!(read_dataset(dir.path(), "ds_test", "TEST").unwrap(), bars);
        fs::write(dataset_path(dir.path(), "ds_test", "TEST"), b"[]").unwrap();
        assert!(read_dataset(dir.path(), "ds_test", "TEST").is_err());
    }

    #[test]
    fn storage_rejects_paths_before_file_access() {
        let dir = tempdir().unwrap();
        for id in ["../outside", "/tmp/escape", "a\\b", "CON", "A:stream", ""] {
            assert!(read_dataset(dir.path(), id, "AAPL").is_err());
            assert!(read_manifest(dir.path(), "valid", id).is_err());
        }
    }
}
