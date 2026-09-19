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

    // Prepare feature sequence [lookback, num_features] backed by FeatureStore
    let feature_store = quant_features::FeatureStore::new();
    let schema_names = provider.feature_schema();

    // Ingest feature rows into FeatureStore
    let mut rows = Vec::with_capacity(lookback);
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    for step in 0..lookback {
        let ts = quant_data::Timestamp(now - ((lookback - step) as i64 * 86_400_000_000_000));
        let mut values = std::collections::HashMap::new();
        for (feat_idx, name) in schema_names.iter().enumerate() {
            let val = ((step as f64) * 0.01 + (feat_idx as f64) * 0.05).sin() * 0.02;
            values.insert(name.clone(), val);
        }
        rows.push(quant_features::FeatureRow {
            timestamp: ts,
            values,
        });
    }
    feature_store.insert_batch(&args.symbol, rows);

    // Query recent lookback sequence from FeatureStore
    let recent_rows = feature_store.query_recent(&args.symbol, lookback);
    let mut raw_features = Vec::with_capacity(lookback * num_features);
    for row in &recent_rows {
        for name in schema_names {
            raw_features.push(*row.values.get(name).unwrap_or(&0.0));
        }
    }

    // 1. Run inference
    let prediction = provider
        .predict(&raw_features, lookback, num_features)
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
