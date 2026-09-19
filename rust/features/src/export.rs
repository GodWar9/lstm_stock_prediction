//! Feature dataset export into Arrow2 Schema and Parquet formats.

use crate::graph::FeatureRow;
use arrow2::array::{Float64Array, Int64Array};
use arrow2::chunk::Chunk;
use arrow2::datatypes::{DataType, Field, Schema};
use arrow2::io::ipc::write::{StreamWriter, WriteOptions as IpcWriteOptions};
use arrow2::io::parquet::write::{
    transverse, CompressionOptions, Encoding, FileWriter, RowGroupIterator, Version, WriteOptions,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExportError {
    #[error("Arrow2 or Parquet write error: {0}")]
    ArrowError(#[from] arrow2::error::Error),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Empty dataset cannot be exported")]
    EmptyDataset,
    #[error("Manifest serialization error: {0}")]
    ManifestError(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FeatureDatasetManifest {
    pub feature_set: String,
    pub feature_set_version: u32,
    pub symbol: String,
    pub source_dataset_version: String,
    pub row_count: usize,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrainingDatasetManifest {
    pub dataset_version: String,
    pub feature_set: String,
    pub feature_set_version: u32,
    pub symbol: String,
    pub row_count: usize,
    pub feature_columns: Vec<String>,
    pub target_horizon: usize,
}

/// Helper to build an Arrow2 Chunk and Schema from FeatureRow records.
pub struct FeatureArrowExporter;

impl FeatureArrowExporter {
    /// Builds Arrow schema and chunk of arrays from a sequence of FeatureRows.
    pub fn build_arrow_chunk(
        rows: &[FeatureRow],
    ) -> Result<(Schema, Chunk<Box<dyn arrow2::array::Array>>), ExportError> {
        if rows.is_empty() {
            return Err(ExportError::EmptyDataset);
        }

        // Collect all unique feature column names sorted for determinism
        let mut column_names = BTreeSet::new();
        for r in rows {
            for k in r.values.keys() {
                column_names.insert(k.clone());
            }
        }

        // Fields: timestamp (Int64), then each feature (Float64)
        let mut fields = vec![Field::new("timestamp", DataType::Int64, false)];
        for name in &column_names {
            fields.push(Field::new(name, DataType::Float64, true));
        }
        let schema = Schema::from(fields);

        // Populate timestamp column
        let timestamps: Vec<i64> = rows.iter().map(|r| r.timestamp.0).collect();
        let ts_array =
            Box::new(Int64Array::from_slice(&timestamps)) as Box<dyn arrow2::array::Array>;

        let mut columns: Vec<Box<dyn arrow2::array::Array>> = vec![ts_array];

        // Populate feature columns with null handling
        for name in &column_names {
            let values: Vec<Option<f64>> =
                rows.iter().map(|r| r.values.get(name).copied()).collect();
            let arr = Box::new(Float64Array::from(values)) as Box<dyn arrow2::array::Array>;
            columns.push(arr);
        }

        let chunk = Chunk::new(columns);
        Ok((schema, chunk))
    }

    /// Writes feature rows to a Parquet output stream.
    pub fn write_parquet<W: Write>(rows: &[FeatureRow], mut writer: W) -> Result<(), ExportError> {
        let (schema, chunk) = Self::build_arrow_chunk(rows)?;

        let options = WriteOptions {
            write_statistics: true,
            compression: CompressionOptions::Uncompressed,
            version: Version::V1,
            data_pagesize_limit: None,
        };

        let encodings: Vec<Vec<Encoding>> = schema
            .fields
            .iter()
            .map(|f| transverse(&f.data_type, |_| Encoding::Plain))
            .collect();

        let row_groups =
            RowGroupIterator::try_new(vec![Ok(chunk)].into_iter(), &schema, options, encodings)?;

        let mut file_writer = FileWriter::try_new(&mut writer, schema, options)?;
        for group in row_groups {
            file_writer.write(group?)?;
        }
        file_writer.end(None)?;

        Ok(())
    }

    /// Persist a feature parquet file and its versioned manifest together.
    pub fn write_versioned_dataset(
        root: impl AsRef<Path>,
        manifest: &FeatureDatasetManifest,
        rows: &[FeatureRow],
    ) -> Result<PathBuf, ExportError> {
        if rows.is_empty() {
            return Err(ExportError::EmptyDataset);
        }

        let output_dir = root
            .as_ref()
            .join(&manifest.feature_set)
            .join(format!("v{}", manifest.feature_set_version));
        fs::create_dir_all(&output_dir)?;

        let output_path = output_dir.join(format!("{}.parquet", manifest.symbol));
        let file = fs::File::create(&output_path)?;
        Self::write_parquet(rows, file)?;

        let manifest_path = output_dir.join(format!("{}.manifest.json", manifest.symbol));
        let manifest_bytes = serde_json::to_vec_pretty(manifest)?;
        fs::write(manifest_path, manifest_bytes)?;

        Ok(output_path)
    }

    /// Writes feature rows joined with forward targets as an Arrow IPC stream.
    pub fn write_training_ipc(
        root: impl AsRef<Path>,
        manifest: &TrainingDatasetManifest,
        rows: &[FeatureRow],
        targets: &[crate::targets::TargetRow],
    ) -> Result<PathBuf, ExportError> {
        if rows.is_empty() || targets.is_empty() {
            return Err(ExportError::EmptyDataset);
        }

        let target_by_timestamp: std::collections::HashMap<_, _> = targets
            .iter()
            .map(|target| (target.timestamp, target))
            .collect();
        let joined: Vec<_> = rows
            .iter()
            .filter_map(|row| {
                target_by_timestamp
                    .get(&row.timestamp)
                    .map(|target| (row, *target))
            })
            .collect();
        if joined.is_empty() {
            return Err(ExportError::EmptyDataset);
        }

        let mut feature_names = BTreeSet::new();
        for (row, _) in &joined {
            feature_names.extend(row.values.keys().cloned());
        }
        let mut fields = vec![
            Field::new("timestamp", DataType::Int64, false),
            Field::new("target_timestamp", DataType::Int64, false),
            Field::new("target", DataType::Float32, false),
            Field::new("asset_id", DataType::UInt32, false),
            Field::new("feature_set_version", DataType::UInt32, false),
            Field::new("target_horizon", DataType::UInt16, false),
        ];
        fields.extend(
            feature_names
                .iter()
                .map(|name| Field::new(name, DataType::Float32, false)),
        );
        let schema = Schema::from(fields);

        let columns: Vec<Box<dyn arrow2::array::Array>> = vec![
            Box::new(Int64Array::from_slice(
                joined
                    .iter()
                    .map(|(row, _)| row.timestamp.0)
                    .collect::<Vec<_>>(),
            )),
            Box::new(Int64Array::from_slice(
                joined
                    .iter()
                    .map(|(_, target)| target.target_timestamp.0)
                    .collect::<Vec<_>>(),
            )),
            Box::new(arrow2::array::Float32Array::from_slice(
                joined
                    .iter()
                    .map(|(_, target)| target.forward_return as f32)
                    .collect::<Vec<_>>(),
            )),
            Box::new(arrow2::array::UInt32Array::from_slice(vec![
                1_u32;
                joined.len()
            ])),
            Box::new(arrow2::array::UInt32Array::from_slice(vec![
                manifest.feature_set_version;
                joined.len()
            ])),
            Box::new(arrow2::array::UInt16Array::from_slice(vec![
                manifest.target_horizon
                    as u16;
                joined.len()
            ])),
        ];
        let mut columns = columns;
        for name in &feature_names {
            columns.push(Box::new(arrow2::array::Float32Array::from_slice(
                joined
                    .iter()
                    .map(|(row, _)| *row.values.get(name).unwrap_or(&0.0) as f32)
                    .collect::<Vec<_>>(),
            )));
        }

        let output_dir = root.as_ref().join(&manifest.dataset_version);
        fs::create_dir_all(&output_dir)?;
        let output_path = output_dir.join(format!("{}.arrow", manifest.symbol));
        let mut file = fs::File::create(&output_path)?;
        let mut writer = StreamWriter::new(&mut file, IpcWriteOptions { compression: None });
        writer.start(&schema, None)?;
        writer.write(&Chunk::new(columns), None)?;
        writer.finish()?;
        fs::write(
            output_dir.join(format!("{}.manifest.json", manifest.symbol)),
            serde_json::to_vec_pretty(&TrainingDatasetManifest {
                row_count: joined.len(),
                feature_columns: feature_names.into_iter().collect(),
                ..manifest.clone()
            })?,
        )?;

        Ok(output_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quant_data::Timestamp;
    use std::collections::HashMap;

    #[test]
    fn test_arrow_chunk_construction() {
        let mut f1 = HashMap::new();
        f1.insert("rsi".to_string(), 55.0);
        f1.insert("sma".to_string(), 102.5);

        let mut f2 = HashMap::new();
        f2.insert("rsi".to_string(), 60.0);
        f2.insert("sma".to_string(), 103.0);

        let rows = vec![
            FeatureRow {
                timestamp: Timestamp(1000),
                values: f1,
            },
            FeatureRow {
                timestamp: Timestamp(2000),
                values: f2,
            },
        ];

        let (schema, chunk) = FeatureArrowExporter::build_arrow_chunk(&rows).unwrap();
        assert_eq!(schema.fields.len(), 3); // timestamp, rsi, sma
        assert_eq!(chunk.len(), 2);
    }

    #[test]
    fn test_parquet_export_roundtrip() {
        let mut f = HashMap::new();
        f.insert("rsi".to_string(), 50.0);

        let rows = vec![FeatureRow {
            timestamp: Timestamp(100),
            values: f,
        }];

        let mut buffer = Vec::new();
        let result = FeatureArrowExporter::write_parquet(&rows, &mut buffer);
        assert!(result.is_ok());
        assert!(!buffer.is_empty());
        // Check Parquet magic bytes "PAR1"
        assert_eq!(&buffer[0..4], b"PAR1");
    }

    #[test]
    fn test_versioned_dataset_writes_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let mut values = HashMap::new();
        values.insert("rsi".to_string(), 50.0);
        let rows = vec![FeatureRow {
            timestamp: Timestamp(100),
            values,
        }];
        let manifest = FeatureDatasetManifest {
            feature_set: "baseline".to_string(),
            feature_set_version: 1,
            symbol: "TEST".to_string(),
            source_dataset_version: "ds_test".to_string(),
            row_count: 1,
            columns: vec!["rsi".to_string()],
        };

        let path =
            FeatureArrowExporter::write_versioned_dataset(dir.path(), &manifest, &rows).unwrap();
        assert!(path.exists());
        assert!(path.with_file_name("TEST.manifest.json").exists());
    }

    #[test]
    fn test_training_ipc_joins_only_rows_with_targets() {
        let dir = tempfile::tempdir().unwrap();
        let rows = vec![
            FeatureRow {
                timestamp: Timestamp(1),
                values: HashMap::from([("sma".to_string(), 10.0)]),
            },
            FeatureRow {
                timestamp: Timestamp(2),
                values: HashMap::from([("sma".to_string(), 11.0)]),
            },
        ];
        let targets = vec![crate::targets::TargetRow {
            timestamp: Timestamp(1),
            target_timestamp: Timestamp(2),
            forward_return: 0.1,
            direction: crate::targets::DirectionLabel::Up,
        }];
        let manifest = TrainingDatasetManifest {
            dataset_version: "ds_test".to_string(),
            feature_set: "baseline".to_string(),
            feature_set_version: 1,
            symbol: "TEST".to_string(),
            row_count: 0,
            feature_columns: Vec::new(),
            target_horizon: 1,
        };

        let path = FeatureArrowExporter::write_training_ipc(dir.path(), &manifest, &rows, &targets)
            .unwrap();
        assert!(path.exists());
        assert!(path.with_file_name("TEST.manifest.json").exists());
        let manifest_content =
            std::fs::read_to_string(path.with_file_name("TEST.manifest.json")).unwrap();
        assert!(manifest_content.contains("\"row_count\": 1"));
        assert!(manifest_content.contains("\"sma\""));
    }
}
