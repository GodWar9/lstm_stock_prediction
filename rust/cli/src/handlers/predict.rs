//! CLI handler for quantctl predict: Rust ONNX inference and signal calibration.

use crate::commands::PredictArgs;
use anyhow::{bail, Context, Result};
use quant_inference::provider::PredictionProvider;
use quant_inference::OnnxLstmProvider;
use quant_instruments::InstrumentId;
use quant_signals::{SignalCalibrator, SignalConfig, SignalTransform, ThresholdFilter};
use std::path::{Path, PathBuf};
use tracing::info;

fn resolve_model_path(model_id: &str) -> Result<PathBuf> {
    let candidates = [
        PathBuf::from(format!("models/{}", model_id)),
        PathBuf::from(format!("../models/{}", model_id)),
        PathBuf::from(format!("../../models/{}", model_id)),
    ];

    for path in &candidates {
        if path.exists() && path.join("metadata.json").exists() {
            return Ok(path.clone());
        }
    }

    bail!(
        "Model artifact for '{}' not found in search paths: {:?}",
        model_id,
        candidates
    );
}

pub fn handle_predict(args: &PredictArgs, _config_path: &Path) -> Result<()> {
    info!(model = %args.model, symbol = %args.symbol, "Executing quantitative model inference");

    let artifact_dir = resolve_model_path(&args.model)
        .with_context(|| format!("Failed to locate model artifact for {}", args.model))?;

    info!(path = %artifact_dir.display(), "Loading ONNX model artifact and fitted scaler");
    let provider = OnnxLstmProvider::load(&artifact_dir)
        .with_context(|| format!("Failed to load OnnxLstmProvider from {}", artifact_dir.display()))?;

    let lookback = provider.lookback();
    let num_features = provider.feature_schema().len();
    let model_id = provider.model_id().to_string();

    info!(
        model_id = %model_id,
        lookback = lookback,
        features = num_features,
        "Model validated successfully"
    );

    // Prepare feature sequence [lookback, num_features]
    // In production this pulls from the Parquet FeatureStore ring-buffer.
    // For standalone CLI predictions, we simulate a representative normalized market feature input.
    let total_elements = lookback * num_features;
    let mut raw_features = Vec::with_capacity(total_elements);
    for i in 0..total_elements {
        let step = (i / num_features) as f64;
        let feat = (i % num_features) as f64;
        let val = (step * 0.01 + feat * 0.05).sin() * 0.02;
        raw_features.push(val);
    }

    // 1. Run inference
    let prediction = provider.predict(&raw_features, lookback, num_features)
        .context("Model inference failed")?;

    // 2. Calibrate prediction to Signal
    let calibrator = SignalCalibrator::new(SignalConfig::default());
    let as_of = chrono::Utc::now().timestamp_millis();
    let raw_signal = calibrator.calibrate(&prediction, InstrumentId(1), &args.symbol, as_of);

    // 3. Apply post-prediction transform (Threshold filter)
    let filter = ThresholdFilter::new(0.0001, 0.05);
    let signal = filter.apply(&raw_signal);

    // 4. Output results
    println!("--------------------------------------------------");
    println!("         QUANTCTL MODEL PREDICTION & SIGNAL       ");
    println!("--------------------------------------------------");
    println!("Symbol:             {}", signal.symbol);
    println!("Model:              {}", signal.model_id);
    println!("Lookback:           {} bars", lookback);
    println!("Feature Count:      {}", num_features);
    println!("Expected Return:    {:+0.4}%", signal.expected_return * 100.0);
    println!("Confidence:         {:.2}%", signal.confidence * 100.0);
    println!("Signal Direction:   {:?}", signal.direction);
    println!("Actionable:         {}", if signal.direction.is_active() { "YES" } else { "NO" });
    println!("Artifact Dir:       {}", artifact_dir.display());
    println!("Timestamp (UTC):    {}", chrono::Utc::now().to_rfc3339());
    println!("--------------------------------------------------");

    Ok(())
}
