//! Fitted feature scaler for immutable inference normalization.

use crate::provider::InferenceError;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Immutable fitted feature scaler loaded from model artifact `scaler.json`.
/// Deliberately exposes ONLY `transform()`, strictly preventing any training-time `fit()` calls.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FittedScaler {
    #[serde(alias = "means")]
    pub mean: Vec<f64>,
    #[serde(alias = "stds", alias = "scale")]
    pub std: Vec<f64>,
    #[serde(default)]
    pub feature_names: Option<Vec<String>>,
}

impl FittedScaler {
    /// Create a new FittedScaler with given mean and std vectors.
    pub fn new(
        mean: Vec<f64>,
        mut std: Vec<f64>,
        feature_names: Option<Vec<String>>,
    ) -> Result<Self, InferenceError> {
        if mean.is_empty() {
            return Err(InferenceError::MetadataValidationFailed(
                "Scaler mean vector cannot be empty".to_string(),
            ));
        }
        if mean.len() != std.len() {
            return Err(InferenceError::ShapeMismatch {
                expected: format!("mean length {}", mean.len()),
                got: format!("std length {}", std.len()),
            });
        }

        // Clamp near-zero std to 1.0 to prevent division by zero
        for s in &mut std {
            if s.abs() < 1e-12 || !s.is_finite() {
                *s = 1.0;
            }
        }

        Ok(Self {
            mean,
            std,
            feature_names,
        })
    }

    /// Load scaler parameters from a JSON file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, InferenceError> {
        let content = std::fs::read_to_string(path.as_ref()).map_err(|e| {
            InferenceError::ArtifactMissing(format!(
                "Failed to read scaler file at {}: {}",
                path.as_ref().display(),
                e
            ))
        })?;

        let raw: Self = serde_json::from_str(&content).map_err(|e| {
            InferenceError::MetadataValidationFailed(format!(
                "Invalid scaler.json format at {}: {}",
                path.as_ref().display(),
                e
            ))
        })?;

        Self::new(raw.mean, raw.std, raw.feature_names)
    }

    /// Number of features expected per timestep.
    pub fn num_features(&self) -> usize {
        self.mean.len()
    }

    /// Normalize a raw feature slice (single timestep or flattened sequence [T, F]).
    pub fn transform(&self, features: &[f64]) -> Result<Vec<f64>, InferenceError> {
        let n_feat = self.num_features();
        if features.is_empty() || !features.len().is_multiple_of(n_feat) {
            return Err(InferenceError::ShapeMismatch {
                expected: format!("multiple of {} features", n_feat),
                got: format!("{} elements", features.len()),
            });
        }

        let mut scaled = Vec::with_capacity(features.len());
        for (idx, &val) in features.iter().enumerate() {
            let feat_idx = idx % n_feat;
            let m = self.mean[feat_idx];
            let s = self.std[feat_idx];
            scaled.push((val - m) / s);
        }

        Ok(scaled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scaler_transform_single_timestep() {
        let scaler = FittedScaler::new(vec![10.0, 100.0], vec![2.0, 10.0], None).unwrap();
        let raw = vec![14.0, 120.0];
        let scaled = scaler.transform(&raw).unwrap();
        assert_eq!(scaled, vec![2.0, 2.0]);
    }

    #[test]
    fn test_scaler_transform_multi_timestep_sequence() {
        let scaler = FittedScaler::new(vec![0.0, 5.0], vec![1.0, 2.0], None).unwrap();
        // 2 timesteps x 2 features = 4 elements
        let raw = vec![1.0, 7.0, 2.0, 9.0];
        let scaled = scaler.transform(&raw).unwrap();
        assert_eq!(scaled, vec![1.0, 1.0, 2.0, 2.0]);
    }

    #[test]
    fn test_scaler_zero_std_handling() {
        let scaler = FittedScaler::new(vec![10.0], vec![0.0], None).unwrap();
        let raw = vec![15.0];
        let scaled = scaler.transform(&raw).unwrap();
        // 0.0 std was clamped to 1.0
        assert_eq!(scaled, vec![5.0]);
    }

    #[test]
    fn test_scaler_shape_mismatch() {
        let scaler = FittedScaler::new(vec![0.0, 0.0], vec![1.0, 1.0], None).unwrap();
        let odd_raw = vec![1.0, 2.0, 3.0];
        assert!(scaler.transform(&odd_raw).is_err());
    }
}
