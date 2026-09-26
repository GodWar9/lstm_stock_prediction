//! Shared, validated production inputs. Synthetic data is selected at ingestion only.
use anyhow::{bail, Context, Result};
use quant_config::load_config;
use quant_data::{read_dataset, validate_bars_monotonic_and_sound, Bar};
use quant_features::{
    Atr, BollingerBands, Ema, FeatureGraph, FeatureRow, LogReturn, Macd, RollingVolatility, Rsi,
    Sma,
};
use std::path::Path;

pub fn graph() -> FeatureGraph {
    // Keep the established schema: the former duplicate SMA key retained SMA(20).
    FeatureGraph::new(vec![
        Box::new(Sma::new(20)),
        Box::new(Ema::new(12)),
        Box::new(Rsi::new(14)),
        Box::new(Macd::standard()),
        Box::new(BollingerBands::standard()),
        Box::new(Atr::new(14)),
        Box::new(RollingVolatility::daily(20)),
        Box::new(LogReturn::new(1)),
    ])
}

pub fn bars(config: &Path, symbol: &str) -> Result<Vec<Bar>> {
    let cfg = load_config(config)?;
    let bars = read_dataset("datasets/market", &cfg.data.dataset_version, symbol)
        .context("Ingest the configured dataset with quantctl data ingest first")?;
    validate_bars_monotonic_and_sound(&bars)?;
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
