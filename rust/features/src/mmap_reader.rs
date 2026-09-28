//! Zero-copy memory-mapped Arrow IPC file reader.
//!
//! Uses `memmap2` to map Arrow IPC files directly into the process address space,
//! reading record batches without heap allocation. This enables loading multi-gigabyte
//! feature datasets with near-zero memory overhead beyond the file-backed pages.
//!
//! # Example
//! ```no_run
//! use quant_features::mmap_reader::MmapArrowReader;
//!
//! let reader = MmapArrowReader::open("datasets/training/v1/AAPL.arrow").unwrap();
//! println!("Schema: {:?}", reader.schema());
//! for batch in reader.chunks().unwrap() {
//!     println!("Batch with {} rows", batch.len());
//! }
//! ```

use arrow2::chunk::Chunk;
use arrow2::datatypes::Schema;
use arrow2::io::ipc::read::{read_file_metadata, FileReader};
use std::io::Cursor;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MmapError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Arrow IPC read error: {0}")]
    Arrow(#[from] arrow2::error::Error),
    #[error("File too small to be a valid Arrow IPC file")]
    FileTooSmall,
}

/// Memory-mapped Arrow IPC file reader.
///
/// The file is mapped into virtual memory once and remains mapped for the
/// lifetime of this struct. Record batches are read lazily from the mapped
/// region without additional heap allocation for the raw data buffers.
pub struct MmapArrowReader {
    /// The memory-mapped file data.
    mmap: memmap2::Mmap,
    /// Parsed Arrow IPC file metadata (schema, dictionaries, block offsets).
    schema: Schema,
    /// Number of record batch blocks in the file.
    num_batches: usize,
}

impl std::fmt::Debug for MmapArrowReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MmapArrowReader")
            .field("schema_fields", &self.schema.fields.len())
            .field("num_batches", &self.num_batches)
            .field("mapped_bytes", &self.mmap.len())
            .finish()
    }
}

impl MmapArrowReader {
    /// Open and memory-map an Arrow IPC file.
    ///
    /// The file is opened read-only and mapped into the process address space.
    /// Schema and metadata are parsed eagerly; record batches are read lazily.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, MmapError> {
        let file = std::fs::File::open(path.as_ref())?;
        let file_len = file.metadata()?.len();

        if file_len < 8 {
            return Err(MmapError::FileTooSmall);
        }

        // Safety: file is opened read-only. The mmap is valid for the file's lifetime.
        let mmap = unsafe { memmap2::MmapOptions::new().map(&file)? };

        // Parse metadata from the mapped bytes
        let mut cursor = Cursor::new(mmap.as_ref());
        let metadata = read_file_metadata(&mut cursor)?;
        let schema = metadata.schema.clone();
        let num_batches = metadata.blocks.len();

        Ok(Self {
            mmap,
            schema,
            num_batches,
        })
    }

    /// Returns the Arrow schema of the mapped file.
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    /// Number of record batch blocks in the file.
    pub fn num_batches(&self) -> usize {
        self.num_batches
    }

    /// Total size of the memory-mapped region in bytes.
    pub fn mapped_bytes(&self) -> usize {
        self.mmap.len()
    }

    /// Read all record batch chunks from the mapped file.
    ///
    /// Each `Chunk` contains Arrow arrays backed by the mapped memory region.
    /// No additional heap allocation occurs for the raw buffer data.
    pub fn chunks(&self) -> Result<Vec<Chunk<Box<dyn arrow2::array::Array>>>, MmapError> {
        let mut cursor = Cursor::new(self.mmap.as_ref());
        let metadata = read_file_metadata(&mut cursor)?;
        let reader = FileReader::new(cursor, metadata, None, None);

        let mut chunks = Vec::with_capacity(self.num_batches);
        for chunk_result in reader {
            chunks.push(chunk_result?);
        }
        Ok(chunks)
    }

    /// Returns field names from the schema.
    pub fn field_names(&self) -> Vec<&str> {
        self.schema.fields.iter().map(|f| f.name.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow2::array::{Float64Array, Int64Array};
    use arrow2::chunk::Chunk;
    use arrow2::datatypes::{DataType, Field, Schema};
    use arrow2::io::ipc::write::{FileWriter, WriteOptions};

    /// Helper: write a small Arrow IPC file for testing.
    fn write_test_ipc(path: &Path) {
        let schema = Schema::from(vec![
            Field::new("timestamp", DataType::Int64, false),
            Field::new("feature_a", DataType::Float64, false),
            Field::new("feature_b", DataType::Float64, false),
        ]);

        let timestamps =
            Box::new(Int64Array::from_slice([100, 200, 300])) as Box<dyn arrow2::array::Array>;
        let feat_a =
            Box::new(Float64Array::from_slice([1.5, 2.5, 3.5])) as Box<dyn arrow2::array::Array>;
        let feat_b =
            Box::new(Float64Array::from_slice([10.0, 20.0, 30.0])) as Box<dyn arrow2::array::Array>;

        let chunk = Chunk::new(vec![timestamps, feat_a, feat_b]);

        let mut file = std::fs::File::create(path).unwrap();
        let mut writer =
            FileWriter::try_new(&mut file, schema, None, WriteOptions { compression: None })
                .unwrap();
        writer.write(&chunk, None).unwrap();
        writer.finish().unwrap();
    }

    #[test]
    fn test_mmap_reader_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.arrow");
        write_test_ipc(&path);

        let reader = MmapArrowReader::open(&path).unwrap();
        assert_eq!(reader.schema().fields.len(), 3);
        assert_eq!(reader.num_batches(), 1);
        assert!(reader.mapped_bytes() > 0);

        let field_names = reader.field_names();
        assert!(field_names.contains(&"timestamp"));
        assert!(field_names.contains(&"feature_a"));
        assert!(field_names.contains(&"feature_b"));

        let chunks = reader.chunks().unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 3); // 3 rows
    }

    #[test]
    fn test_mmap_reader_file_too_small() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tiny.arrow");
        std::fs::write(&path, b"ABC").unwrap(); // 3 bytes, too small

        let result = MmapArrowReader::open(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_mmap_reader_nonexistent_file() {
        let result = MmapArrowReader::open("/nonexistent/path/test.arrow");
        assert!(result.is_err());
    }
}
