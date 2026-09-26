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

pub fn handle_predict(args: &PredictArgs, config_path: &Path) -> Result<()> {
    info!(model = %args.model, symbol = %args.symbol, "Executing quantitative model inference");

    let artifact_dir = resolve_model_path(&args.model)
        .with_context(|| format!("Failed to locate model artifact for {}", args.model))?;

    info!(path = %artifact_dir.display(), "Loading ONNX model artifact and fitted scaler");
    let provider = OnnxLstmProvider::load(&artifact_dir).with_context(|| {
        format!(
            "Failed to load OnnxLstmProvider from {}",
            artifact_dir.display()
        )
    })?;

    let lookback = provider.lookback();
    let num_features = provider.feature_schema().len();
    let model_id = provider.model_id().to_string();

    info!(
        model_id = %model_id,
        lookback = lookback,
        features = num_features,
        "Model validated successfully"
    );

    let bars = super::pipeline::bars(config_path, &args.symbol)?;
    let rows = super::pipeline::graph().compute_batch(&bars);
    let ordered = super::pipeline::ordered(&rows, provider.feature_schema())?;
    if ordered.len() < lookback {
        bail!("Insufficient feature rows for model lookback");
    }
    let raw_features: Vec<f64> = ordered[ordered.len() - lookback..]
        .iter()
        .flat_map(|(_, row)| row.iter().copied())
        .collect();

    // 1. Run inference
    let prediction = provider
        .predict(&raw_features, lookback, num_features)
        .context("Model inference failed")?;

    // 2. Calibrate prediction to Signal
    let calibrator = SignalCalibrator::new(SignalConfig::default());
    let as_of = bars.last().unwrap().timestamp.as_nanos();
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
    println!(
        "Expected Return:    {:+0.4}%",
        signal.expected_return * 100.0
    );
    println!("Confidence:         {:.2}%", signal.confidence * 100.0);
    println!("Signal Direction:   {:?}", signal.direction);
    println!(
        "Actionable:         {}",
        if signal.direction.is_active() {
            "YES"
        } else {
            "NO"
        }
    );
    println!("Artifact Dir:       {}", artifact_dir.display());
    println!("Timestamp (UTC):    {}", chrono::Utc::now().to_rfc3339());
    println!("--------------------------------------------------");

    Ok(())
}
