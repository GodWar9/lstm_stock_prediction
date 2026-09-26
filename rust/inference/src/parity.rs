//! Numerical parity gate for staged and published artifacts.
use crate::{FittedScaler, ModelMetadata, OnnxSession};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::{fs, path::Path};

#[derive(Deserialize)]
struct Case {
    raw_features: Vec<f64>,
    expected: f64,
}

pub fn verify_runtime_parity(dir: &Path) -> Result<serde_json::Value> {
    let metadata = ModelMetadata::load(dir.join("metadata.json"))?;
    let scaler = FittedScaler::load(dir.join("scaler.json"))?;
    let validation: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join("validation.json"))?)?;
    let cases: Vec<Case> = serde_json::from_value(validation["runtime_parity"]["cases"].clone())
        .context("Missing runtime parity reference cases; retrain the model")?;
    if cases.is_empty() || cases.len() > 16 {
        bail!("Runtime parity needs 1-16 reference windows");
    }
    let session = OnnxSession::load(
        dir.join("model.onnx"),
        &metadata.model_id,
        metadata.lookback,
        scaler.num_features(),
    )?;
    let mut max_error = 0.0_f64;
    for case in &cases {
        if case.raw_features.len() != metadata.lookback * scaler.num_features()
            || !case.expected.is_finite()
            || case.raw_features.iter().any(|v| !v.is_finite())
        {
            bail!("Invalid runtime parity reference shape or values");
        }
        let scaled = scaler.transform_f32(&case.raw_features)?;
        let actual = session
            .predict_f32_slice(&scaled, metadata.lookback, scaler.num_features())?
            .value;
        let error = (actual - case.expected).abs();
        if !error.is_finite() || error > 1e-5 {
            bail!("Rust runtime parity failed: absolute error {error}");
        }
        max_error = max_error.max(error);
    }
    Ok(
        serde_json::json!({"passed": true, "runtime": "tract", "max_abs_error": max_error,
        "tolerance": 1e-5, "windows_checked": cases.len()}),
    )
}
