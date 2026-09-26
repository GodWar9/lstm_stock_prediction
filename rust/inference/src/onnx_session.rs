//! ONNX Runtime session using tract-onnx for pure-Rust inference.

use crate::provider::{InferenceError, Prediction, PredictionProvider};
use std::path::Path;
use tract_onnx::prelude::*;

type TractPlan = SimplePlan<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>;

/// ONNX model session backed by tract.
pub struct OnnxSession {
    model: TractPlan,
    model_id: String,
    seq_len: usize,
    num_features: usize,
}

impl std::fmt::Debug for OnnxSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnnxSession")
            .field("model_id", &self.model_id)
            .field("seq_len", &self.seq_len)
            .field("num_features", &self.num_features)
            .finish()
    }
}

impl OnnxSession {
    /// Load an ONNX model from disk with default graph optimizations.
    pub fn load(
        path: impl AsRef<Path>,
        model_id: impl Into<String>,
        seq_len: usize,
        num_features: usize,
    ) -> Result<Self, InferenceError> {
        Self::load_optimized(path, model_id, seq_len, num_features, true)
    }

    /// Load an ONNX model with explicit control over constant folding and decluttering.
    pub fn load_optimized(
        path: impl AsRef<Path>,
        model_id: impl Into<String>,
        seq_len: usize,
        num_features: usize,
        declutter: bool,
    ) -> Result<Self, InferenceError> {
        let raw_model = tract_onnx::onnx()
            .model_for_path(path.as_ref())
            .map_err(|e| {
                InferenceError::InferenceFailed(format!("Failed to load ONNX model: {}", e))
            })?
            .with_input_fact(
                0,
                InferenceFact::dt_shape(
                    f32::datum_type(),
                    tvec![1, seq_len as i64, num_features as i64],
                ),
            )
            .map_err(|e| {
                InferenceError::InferenceFailed(format!("Failed to set input fact: {}", e))
            })?;

        let typed = raw_model
            .into_typed()
            .map_err(|e| InferenceError::InferenceFailed(format!("Failed to type model: {}", e)))?;

        let decluttered = if declutter {
            typed.into_decluttered().map_err(|e| {
                InferenceError::InferenceFailed(format!("Failed to declutter model: {}", e))
            })?
        } else {
            typed
        };

        let optimized = decluttered.into_optimized().map_err(|e| {
            InferenceError::InferenceFailed(format!("Failed to optimize model: {}", e))
        })?;

        let model = optimized.into_runnable().map_err(|e| {
            InferenceError::InferenceFailed(format!("Failed to make model runnable: {}", e))
        })?;

        Ok(Self {
            model,
            model_id: model_id.into(),
            seq_len,
            num_features,
        })
    }

    /// High-performance execution taking pre-scaled f32 feature slice directly.
    /// Avoids redundant f64 -> f32 conversion on inference hot-paths.
    pub fn predict_f32_slice(
        &self,
        features_f32: &[f32],
        seq_len: usize,
        num_features: usize,
    ) -> Result<Prediction, InferenceError> {
        if seq_len != self.seq_len || num_features != self.num_features {
            return Err(InferenceError::ShapeMismatch {
                expected: format!("[1, {}, {}]", self.seq_len, self.num_features),
                got: format!("[1, {}, {}]", seq_len, num_features),
            });
        }

        let expected_len = seq_len * num_features;
        if features_f32.len() != expected_len {
            return Err(InferenceError::ShapeMismatch {
                expected: format!("{} elements", expected_len),
                got: format!("{} elements", features_f32.len()),
            });
        }

        let input = tract_ndarray::Array3::from_shape_vec(
            (1, seq_len, num_features),
            features_f32.to_vec(),
        )
        .map_err(|e| InferenceError::InferenceFailed(format!("Array shape error: {}", e)))?;

        let input_tensor: Tensor = input.into();
        let result = self
            .model
            .run(tvec![input_tensor.into()])
            .map_err(|e| InferenceError::InferenceFailed(format!("Inference run failed: {}", e)))?;

        let output = result[0].to_array_view::<f32>().map_err(|e| {
            InferenceError::InferenceFailed(format!("Output extraction failed: {}", e))
        })?;

        let value = output.iter().next().copied().unwrap_or(0.0) as f64;

        Ok(Prediction {
            value,
            confidence: None,
            model_id: self.model_id.clone(),
            as_of: None,
        })
    }
}

impl PredictionProvider for OnnxSession {
    fn predict(
        &self,
        features: &[f64],
        seq_len: usize,
        num_features: usize,
    ) -> Result<Prediction, InferenceError> {
        let f32_data: Vec<f32> = features.iter().map(|&v| v as f32).collect();
        self.predict_f32_slice(&f32_data, seq_len, num_features)
    }

    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn feature_schema(&self) -> &[String] {
        &[]
    }

    fn lookback(&self) -> usize {
        self.seq_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_onnx_path() -> PathBuf {
        // Try paths relative to workspace root
        let candidates = [
            PathBuf::from("models/lstm_v1/model.onnx"),
            PathBuf::from("../../models/lstm_v1/model.onnx"),
        ];
        for p in &candidates {
            if p.exists() {
                return p.clone();
            }
        }
        // Fallback; test will skip if missing
        PathBuf::from("models/lstm_v1/model.onnx")
    }

    #[test]
    fn test_onnx_session_load_and_predict() {
        let path = test_onnx_path();
        if !path.exists() {
            eprintln!("Skipping test: ONNX model not found at {:?}", path);
            return;
        }

        let session = OnnxSession::load(&path, "lstm_v1", 20, 9).unwrap();
        assert_eq!(session.model_id(), "lstm_v1");

        let dummy_features: Vec<f64> = (0..20 * 9).map(|i| i as f64 * 0.01).collect();
        let pred = session.predict(&dummy_features, 20, 9).unwrap();
        assert!(pred.value.is_finite());
    }
}
