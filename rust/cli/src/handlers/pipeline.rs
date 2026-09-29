//! Shared, validated production inputs. Synthetic data is selected at ingestion only.
use anyhow::{bail, Context, Result};
use quant_config::load_config;
use quant_data::{read_dataset, validate_bars_monotonic_and_sound, Bar};
use quant_features::{FeatureGraph, FeatureRow};
use std::path::Path;

pub fn graph(periods_per_year: f64) -> FeatureGraph {
    quant_features::standard_graph(periods_per_year)
}

pub fn bars(config: &Path, symbol: &str) -> Result<Vec<Bar>> {
    let cfg = load_config(config)?;
    let manifest = quant_data::read_manifest("datasets/market", &cfg.data.dataset_version, symbol)?;
    if manifest.bar_interval != cfg.data.bar_interval {
        bail!("Configured bar interval differs from the dataset");
    }
    let bars = read_dataset("datasets/market", &cfg.data.dataset_version, symbol)
        .context("Ingest the configured dataset with quantctl data ingest first")?;
    validate_bars_monotonic_and_sound(&bars)?;
    if bars.iter().any(|b| b.availability_timestamp < b.timestamp)
        || bars
            .windows(2)
            .any(|b| b[1].availability_timestamp <= b[0].availability_timestamp)
    {
        bail!("Bar availability must follow market time and be strictly increasing");
    }
    if bars.is_empty() {
        bail!("Market dataset is empty");
    }
    Ok(bars)
}

pub fn ordered(rows: &[FeatureRow], schema: &[String]) -> Result<Vec<(i64, Vec<f64>)>> {
    rows.iter().map(|row| {
        let values = schema.iter().map(|name| {
            let value = *row.values.get(name).with_context(|| format!("Model feature '{name}' is unavailable; retrain on the Rust Arrow dataset"))?;
            if !value.is_finite() { bail!("Non-finite feature {name}"); }
            Ok(value)
        }).collect::<Result<Vec<_>>>()?;
        Ok((row.timestamp.as_nanos(), values))
    }).collect()
}

pub fn check_model_interval(artifact: &Path, cfg: &quant_config::AppConfig) -> Result<()> {
    let meta: serde_json::Value =
        serde_json::from_slice(&std::fs::read(artifact.join("metadata.json"))?)?;
    if meta["bar_interval"].as_str().unwrap_or("1d") != cfg.data.bar_interval {
        bail!("Model bar interval differs from the dataset; train an interval-matched model");
    }
    Ok(())
}
