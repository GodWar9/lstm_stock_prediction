//! Model metadata schema and validation for production ONNX artifacts.

use crate::provider::InferenceError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Target definition configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TargetDefinition {
    pub horizon: usize,
    pub transformation: String,
    #[serde(default = "default_price_field")]
    pub price_field: String,
    #[serde(default = "default_target_type")]
    pub target_type: String,
}

fn default_price_field() -> String {
    "close".to_string()
}

fn default_target_type() -> String {
    "regression".to_string()
}

/// Model network architecture configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArchitectureConfig {
    pub hidden_size: usize,
    pub num_layers: usize,
    #[serde(default)]
    pub dropout: f64,
}

/// Training hyperparameters metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HyperparametersConfig {
    #[serde(default)]
    pub lr: f64,
    #[serde(default)]
    pub batch_size: usize,
    #[serde(default)]
    pub weight_decay: f64,
}

/// Full metadata schema for versioned model artifacts (Docs/03-MODEL-TRAINING.md).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelMetadata {
    pub model_id: String,
    pub model_version: u32,
    #[serde(default)]
    pub training_dataset_version: String,
    #[serde(default)]
    pub feature_set_version: u32,
    pub feature_schema: Vec<String>,
    pub target_definition: TargetDefinition,
    pub lookback: usize,
    pub architecture: ArchitectureConfig,
    pub hyperparameters: HyperparametersConfig,
    #[serde(default)]
    pub training_period: Vec<String>,
    #[serde(default)]
    pub validation_period: Vec<String>,
    #[serde(default)]
    pub test_period: Vec<String>,
    #[serde(default)]
    pub random_seed: u64,
    #[serde(default)]
    pub framework_version: String,
    pub onnx_opset: usize,
    #[serde(default)]
    pub evaluation_metrics: HashMap<String, f64>,
    #[serde(default)]
    pub git_commit: String,
    #[serde(default)]
    pub created_at: String,
}

impl ModelMetadata {
    /// Load metadata from a JSON file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, InferenceError> {
        let content = std::fs::read_to_string(path.as_ref()).map_err(|e| {
            InferenceError::ArtifactMissing(format!(
                "Failed to read metadata file at {}: {}",
                path.as_ref().display(),
                e
            ))
        })?;

        let metadata: Self = serde_json::from_str(&content).map_err(|e| {
            InferenceError::MetadataValidationFailed(format!(
                "Invalid metadata.json format at {}: {}",
                path.as_ref().display(),
                e
            ))
        })?;

        metadata.validate()?;
        Ok(metadata)
    }

    /// Strict validation enforcing production requirements.
    pub fn validate(&self) -> Result<(), InferenceError> {
        if self.model_id.trim().is_empty() {
            return Err(InferenceError::MetadataValidationFailed(
                "model_id cannot be empty".to_string(),
            ));
        }

        if self.feature_schema.is_empty() {
            return Err(InferenceError::MetadataValidationFailed(
                "feature_schema cannot be empty".to_string(),
            ));
        }

        if self.lookback == 0 {
            return Err(InferenceError::MetadataValidationFailed(
                "lookback must be greater than 0".to_string(),
            ));
        }

        if self.architecture.hidden_size == 0 || self.architecture.num_layers == 0 {
            return Err(InferenceError::MetadataValidationFailed(
                "architecture hidden_size and num_layers must be positive".to_string(),
            ));
        }

        if self.onnx_opset < 9 || self.onnx_opset > 21 {
            return Err(InferenceError::MetadataValidationFailed(format!(
                "Unsupported ONNX opset version: {}. Expected 9..=21",
                self.onnx_opset
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_metadata_deserialization() {
        let json = r#"{
            "model_id": "lstm_v1",
            "model_version": 1,
            "feature_schema": ["ret", "vol"],
            "target_definition": { "horizon": 1, "transformation": "log_return" },
            "lookback": 20,
            "architecture": { "hidden_size": 64, "num_layers": 2, "dropout": 0.2 },
            "hyperparameters": { "lr": 0.001, "batch_size": 32, "weight_decay": 0.0001 },
            "onnx_opset": 17
        }"#;

        let meta: ModelMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(meta.model_id, "lstm_v1");
        assert_eq!(meta.lookback, 20);
        assert!(meta.validate().is_ok());
    }

    #[test]
    fn test_invalid_metadata_rejected() {
        let invalid_json = r#"{
            "model_id": "",
            "model_version": 1,
            "feature_schema": [],
            "target_definition": { "horizon": 1, "transformation": "log_return" },
            "lookback": 0,
            "architecture": { "hidden_size": 0, "num_layers": 0 },
            "hyperparameters": {},
            "onnx_opset": 5
        }"#;

        let meta: Result<ModelMetadata, _> = serde_json::from_str(invalid_json);
        if let Ok(m) = meta {
            assert!(m.validate().is_err());
        }
    }
}
