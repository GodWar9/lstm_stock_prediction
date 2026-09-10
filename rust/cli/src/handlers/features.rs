//! Handler for `quantctl features` commands.

use crate::commands::FeaturesSubcommands;
use anyhow::{Context, Result};
use chrono::NaiveDate;
use quant_config::load_config;
use quant_data::adapters::mock::SyntheticDataProvider;
use quant_data::MarketDataProvider;
use quant_features::{
    export::{FeatureArrowExporter, FeatureDatasetManifest},
    Atr, BollingerBands, Ema, FeatureGraph, FeatureStore, LogReturn, Macd, RollingVolatility, Rsi,
    Sma,
};
use std::path::Path;
use tracing::info;

pub fn handle_features(cmd: &FeaturesSubcommands, config_path: &Path) -> Result<()> {
    match cmd {
        FeaturesSubcommands::Build { feature_set } => {
            info!(feature_set = %feature_set, "Building feature dataset");

            let app_config =
                load_config(config_path).context("Failed to load application configuration")?;

            let start_date = NaiveDate::parse_from_str(&app_config.data.start_date, "%Y-%m-%d")
                .unwrap_or_else(|_| NaiveDate::from_ymd_opt(2023, 1, 1).unwrap());
            let end_date = NaiveDate::parse_from_str(&app_config.data.end_date, "%Y-%m-%d")
                .unwrap_or_else(|_| NaiveDate::from_ymd_opt(2023, 12, 31).unwrap());

            // Setup deterministic synthetic data provider
            let provider = SyntheticDataProvider::new(100.0, 0.015, 42);
            let symbols = &app_config.data.symbols;

            let store = FeatureStore::new();

            for symbol in symbols {
                info!(symbol = %symbol, "Processing symbol for features");
                let bars = provider.fetch_ohlcv(symbol, start_date, end_date)?;
                if bars.is_empty() {
                    info!(symbol = %symbol, "No bars returned, skipping");
                    continue;
                }

                // Build standard feature suite
                let mut graph = FeatureGraph::new(vec![
                    Box::new(Sma::new(5)),
                    Box::new(Sma::new(20)),
                    Box::new(Ema::new(12)),
                    Box::new(Rsi::new(14)),
                    Box::new(Macd::standard()),
                    Box::new(BollingerBands::standard()),
                    Box::new(Atr::new(14)),
                    Box::new(RollingVolatility::daily(20)),
                    Box::new(LogReturn::new(1)),
                ]);

                let rows = graph.compute_batch(&bars);
                let row_count = rows.len();
                info!(
                    symbol = %symbol,
                    bars_ingested = bars.len(),
                    feature_rows_computed = row_count,
                    max_lookback = graph.max_lookback(),
                    "Feature computation complete"
                );

                if !rows.is_empty() {
                    // Export to Parquet in the configured data directory
                    let mut columns: Vec<String> = rows[0].values.keys().cloned().collect();
                    columns.sort();
                    let out_path = FeatureArrowExporter::write_versioned_dataset(
                        Path::new("datasets/features"),
                        &FeatureDatasetManifest {
                            feature_set: feature_set.clone(),
                            feature_set_version: app_config.features.feature_set_version,
                            symbol: symbol.clone(),
                            source_dataset_version: app_config.data.dataset_version.clone(),
                            row_count,
                            columns,
                        },
                        &rows,
                    )
                    .with_context(|| format!("Failed to persist feature dataset for {}", symbol))?;

                    info!(
                        symbol = %symbol,
                        output_file = ?out_path,
                        rows = row_count,
                        "Successfully exported feature dataset to Parquet"
                    );

                    store.insert_batch(symbol, rows);
                }
            }

            println!(
                "Successfully built feature set '{}' across {} symbol(s).",
                feature_set,
                symbols.len()
            );
            Ok(())
        }
    }
}
