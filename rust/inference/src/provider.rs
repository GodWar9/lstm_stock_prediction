//! Trait abstraction for prediction providers and OnnxLstmProvider.

use crate::metadata::ModelMetadata;
use crate::onnx_session::OnnxSession;
use crate::scaler::FittedScaler;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum InferenceError {
    #[error("Model not loaded: {0}")]
    ModelNotLoaded(String),

    #[error("Inference failed: {0}")]
    InferenceFailed(String),

    #[error("Invalid input shape: expected {expected}, got {got}")]
    ShapeMismatch { expected: String, got: String },

    #[error("Metadata validation failed: {0}")]
    MetadataValidationFailed(String),

    #[error("Model artifact missing: {0}")]
    ArtifactMissing(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Result of a single prediction from a quantitative model.
#[derive(Debug, Clone, PartialEq)]
pub struct Prediction {
    /// Raw model output (predicted forward return).
    pub value: f64,
    /// Calibrated model confidence score (optional, between 0.0 and 1.0).
    pub confidence: Option<f64>,
    /// Model identifier for auditability and observability.
    pub model_id: String,
    /// Optional timestamp (nanoseconds / unix epoch) corresponding to the prediction input.
    pub as_of: Option<i64>,
}

/// Trait for any inference backend (ONNX, mock, ensemble).
pub trait PredictionProvider: Send + Sync {
    /// Predict from a raw feature window of shape `[seq_len, num_features]`.
    fn predict(
        &self,
        raw_features: &[f64],
        seq_len: usize,
        num_features: usize,
    ) -> Result<Prediction, InferenceError>;

    /// Predict over a batch of sequences of shape `[batch_size, seq_len, num_features]`.
    fn predict_batch(
        &self,
        raw_features_batch: &[f64],
        batch_size: usize,
        seq_len: usize,
        num_features: usize,
    ) -> Result<Vec<Prediction>, InferenceError> {
        let seq_elements = seq_len * num_features;
        if raw_features_batch.len() != batch_size * seq_elements {
            return Err(InferenceError::ShapeMismatch {
                expected: format!(
                    "batch size {} * {} elements = {}",
                    batch_size,
                    seq_elements,
                    batch_size * seq_elements
                ),
                got: format!("{} elements", raw_features_batch.len()),
            });
        }

        let mut results = Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            let start = i * seq_elements;
            let end = start + seq_elements;
            let pred = self.predict(&raw_features_batch[start..end], seq_len, num_features)?;
            results.push(pred);
        }
        Ok(results)
    }

    /// Model identifier string.
    fn model_id(&self) -> &str;

    /// Feature column names expected by this model.
    fn feature_schema(&self) -> &[String];

    /// Expected sequence lookback (number of bars).
    fn lookback(&self) -> usize;
}

/// Production ONNX LSTM prediction provider backed by pure-Rust tract runtime and fitted scaler.
pub struct OnnxLstmProvider {
    session: OnnxSession,
    scaler: FittedScaler,
    metadata: ModelMetadata,
}

impl OnnxLstmProvider {
    /// Load a full model artifact directory containing `model.onnx`, `scaler.json`, and `metadata.json`.
    pub fn load(artifact_dir: impl AsRef<Path>) -> Result<Self, InferenceError> {
        let dir = artifact_dir.as_ref();
        if !dir.exists() || !dir.is_dir() {
            return Err(InferenceError::ArtifactMissing(format!(
                "Artifact directory does not exist: {}",
                dir.display()
            )));
        }

        let metadata_path = dir.join("metadata.json");
        let scaler_path = dir.join("scaler.json");
        let model_path = dir.join("model.onnx");

        if !model_path.exists() {
            return Err(InferenceError::ArtifactMissing(format!(
                "model.onnx not found at {}",
                model_path.display()
            )));
        }

        // 1. Load and validate metadata
        let metadata = ModelMetadata::load(&metadata_path)?;

        // 2. Load and validate scaler
        let scaler = FittedScaler::load(&scaler_path)?;
        if scaler.num_features() != metadata.feature_schema.len() {
            return Err(InferenceError::ShapeMismatch {
                expected: format!(
                    "scaler features {} matching metadata schema {}",
                    scaler.num_features(),
                    metadata.feature_schema.len()
                ),
                got: format!(
                    "{} scaler features vs {} schema columns",
                    scaler.num_features(),
                    metadata.feature_schema.len()
                ),
            });
        }

        // 3. Load ONNX model with tract runtime
        let session = OnnxSession::load(
            &model_path,
            &metadata.model_id,
            metadata.lookback,
            scaler.num_features(),
        )?;

        Ok(Self {
            session,
            scaler,
            metadata,
        })
    }

    /// Reference to the fitted scaler.
    pub fn scaler(&self) -> &FittedScaler {
        &self.scaler
    }

    /// Reference to the verified model metadata.
    pub fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }
}

impl PredictionProvider for OnnxLstmProvider {
    fn predict(
        &self,
        raw_features: &[f64],
        seq_len: usize,
        num_features: usize,
    ) -> Result<Prediction, InferenceError> {
        if seq_len != self.metadata.lookback || num_features != self.scaler.num_features() {
            return Err(InferenceError::ShapeMismatch {
                expected: format!(
                    "[{}, {}]",
                    self.metadata.lookback,
                    self.scaler.num_features()
                ),
                got: format!("[{}, {}]", seq_len, num_features),
            });
        }

        // Normalize raw features using the frozen artifact scaler
        let scaled_features = self.scaler.transform(raw_features)?;

        // Run inference through ONNX session
        let raw_pred = self
            .session
            .predict(&scaled_features, seq_len, num_features)?;

        // Return calibrated prediction tagged with model provenance
        Ok(Prediction {
            value: raw_pred.value,
            confidence: raw_pred.confidence,
            model_id: self.metadata.model_id.clone(),
            as_of: None,
        })
    }

    fn model_id(&self) -> &str {
        &self.metadata.model_id
    }

    fn feature_schema(&self) -> &[String] {
        &self.metadata.feature_schema
    }

    fn lookback(&self) -> usize {
        self.metadata.lookback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn find_artifact_dir() -> PathBuf {
        let candidates = [
            PathBuf::from("models/lstm_v1"),
            PathBuf::from("../../models/lstm_v1"),
            PathBuf::from("../models/lstm_v1"),
        ];
        for c in &candidates {
            if c.join("metadata.json").exists() {
                return c.clone();
            }
        }
        PathBuf::from("models/lstm_v1")
    }

    #[test]
    fn test_onnx_lstm_provider_load_and_predict() {
        let dir = find_artifact_dir();
        if !dir.join("model.onnx").exists() || !dir.join("metadata.json").exists() {
            eprintln!(
                "Skipping test: model artifact directory not found at {:?}",
                dir
            );
            return;
        }

        let provider = OnnxLstmProvider::load(&dir).unwrap();
        assert_eq!(provider.model_id(), "lstm_v1");
        let lookback = provider.lookback();
        let num_features = provider.feature_schema().len();
        assert!(lookback > 0);
        assert_eq!(num_features, 9);

        // Dummy raw features: lookback timesteps * num_features
        let raw_features = vec![0.05; lookback * num_features];
        let pred = provider
            .predict(&raw_features, lookback, num_features)
            .unwrap();
        assert_eq!(pred.model_id, "lstm_v1");
        assert!(pred.value.is_finite());
    }

    #[test]
    fn test_onnx_lstm_provider_batch_predict() {
        let dir = find_artifact_dir();
        if !dir.join("model.onnx").exists() {
            return;
        }

        let provider = OnnxLstmProvider::load(&dir).unwrap();
        let lookback = provider.lookback();
        let num_features = provider.feature_schema().len();
        let batch_size = 3;
        let batch_features = vec![0.02; batch_size * lookback * num_features];
        let preds = provider
            .predict_batch(&batch_features, batch_size, lookback, num_features)
            .unwrap();
        assert_eq!(preds.len(), batch_size);
        for p in preds {
            assert!(p.value.is_finite());
        }
    }
}
