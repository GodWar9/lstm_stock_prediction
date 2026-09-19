//! In-memory and partitioned Feature Store.

use crate::graph::FeatureRow;
use crate::traits::FeatureId;
use quant_data::Timestamp;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FeatureStoreError {
    #[error("Version mismatch for feature {name}: expected {expected}, found {found}")]
    VersionMismatch {
        name: String,
        expected: u32,
        found: u32,
    },
    #[error("Feature not found: {0}")]
    NotFound(String),
}

/// Feature store partitioned by symbol / instrument and indexed by timestamp.
#[derive(Debug, Default, Clone)]
pub struct FeatureStore {
    schema_versions: Arc<RwLock<BTreeMap<String, u32>>>,
    // Symbol -> (Timestamp -> HashMap<feature_name, value>)
    data: Arc<RwLock<BTreeMap<String, BTreeMap<Timestamp, FeatureRow>>>>,
}

impl FeatureStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register expected feature version in store schema.
    pub fn register_feature(&self, id: &FeatureId) -> Result<(), FeatureStoreError> {
        let mut versions = self.schema_versions.write().unwrap();
        if let Some(&existing) = versions.get(&id.name) {
            if existing != id.version {
                return Err(FeatureStoreError::VersionMismatch {
                    name: id.name.clone(),
                    expected: existing,
                    found: id.version,
                });
            }
        } else {
            versions.insert(id.name.clone(), id.version);
        }
        Ok(())
    }

    /// Store a feature row for a symbol.
    pub fn insert_row(&self, symbol: &str, row: FeatureRow) {
        let mut data = self.data.write().unwrap();
        let symbol_entry = data.entry(symbol.to_string()).or_default();
        symbol_entry.insert(row.timestamp, row);
    }

    /// Store multiple rows for a symbol.
    pub fn insert_batch(&self, symbol: &str, rows: Vec<FeatureRow>) {
        let mut data = self.data.write().unwrap();
        let symbol_entry = data.entry(symbol.to_string()).or_default();
        for row in rows {
            symbol_entry.insert(row.timestamp, row);
        }
    }

    /// Retrieve rows for a symbol in a closed timestamp interval `[start, end]`.
    pub fn query_range(&self, symbol: &str, start: Timestamp, end: Timestamp) -> Vec<FeatureRow> {
        let data = self.data.read().unwrap();
        if let Some(symbol_entry) = data.get(symbol) {
            symbol_entry
                .range(start..=end)
                .map(|(_, row)| row.clone())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Returns the total row count stored for a symbol.
    pub fn count_rows(&self, symbol: &str) -> usize {
        let data = self.data.read().unwrap();
        data.get(symbol).map(|e| e.len()).unwrap_or(0)
    }

    /// Clears data for a symbol or all symbols.
    pub fn clear(&self) {
        let mut data = self.data.write().unwrap();
        data.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_feature_registration_versioning() {
        let store = FeatureStore::new();
        let f1 = FeatureId::new("rsi", 1);
        let f1_same = FeatureId::new("rsi", 1);
        let f1_conflict = FeatureId::new("rsi", 2);

        assert!(store.register_feature(&f1).is_ok());
        assert!(store.register_feature(&f1_same).is_ok());
        assert!(store.register_feature(&f1_conflict).is_err());
    }

    #[test]
    fn test_store_query_range() {
        let store = FeatureStore::new();
        let mut vals = HashMap::new();
        vals.insert("sma".to_string(), 42.0);

        store.insert_row(
            "AAPL",
            FeatureRow {
                timestamp: Timestamp(100),
                values: vals.clone(),
            },
        );
        store.insert_row(
            "AAPL",
            FeatureRow {
                timestamp: Timestamp(200),
                values: vals.clone(),
            },
        );
        store.insert_row(
            "AAPL",
            FeatureRow {
                timestamp: Timestamp(300),
                values: vals.clone(),
            },
        );

        let rows = store.query_range("AAPL", Timestamp(150), Timestamp(250));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].timestamp, Timestamp(200));

        let all = store.query_range("AAPL", Timestamp(0), Timestamp(500));
        assert_eq!(all.len(), 3);
        assert_eq!(store.count_rows("AAPL"), 3);
        assert_eq!(store.count_rows("MSFT"), 0);
    }
}
