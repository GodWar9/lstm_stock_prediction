//! Handler for `quantctl features` commands.

use crate::commands::FeaturesSubcommands;
use anyhow::{Context, Result};

use quant_config::load_config;

use quant_features::{
    export::{FeatureArrowExporter, FeatureDatasetManifest},
    targets::TargetGenerator,
    FeatureStore, TrainingDatasetManifest,
};
use std::path::Path;
use tracing::info;

pub fn handle_features(cmd: &FeaturesSubcommands, config_path: &Path) -> Result<()> {
    match cmd {
        FeaturesSubcommands::Build { feature_set } => {
            info!(feature_set = %feature_set, "Building feature dataset");

            let app_config =
                load_config(config_path).context("Failed to load application configuration")?;
            if feature_set != "baseline_v1"
                || app_config.features.feature_set != "baseline_v1"
                || app_config.features.target_transformation != "log_return"
            {
                anyhow::bail!("Only baseline_v1 features and log_return targets are implemented by this pipeline");
            }

            let symbols = &app_config.data.symbols;

            let store = FeatureStore::new();

            for symbol in symbols {
                info!(symbol = %symbol, "Processing symbol for features");
                let bars = super::pipeline::bars(config_path, symbol)?;
                if bars.is_empty() {
                    anyhow::bail!("No bars available for {symbol}");
                }

                // Build standard feature suite
                let mut graph = super::pipeline::graph(app_config.data.periods_per_year());

                let rows = graph.compute_batch(&bars);
                let row_count = rows.len();
                if rows.is_empty() {
                    anyhow::bail!("Insufficient bars for {symbol}: {} supplied, need more than {} for feature warmup", bars.len(), graph.max_lookback());
                }
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

                    let targets =
                        TargetGenerator::new(app_config.features.target_horizon as usize, 0.001)
                            .context("Invalid target horizon configuration")?
                            .compute_targets(&bars)
                            .context("Failed to compute forward targets")?;
                    let training_path = FeatureArrowExporter::write_training_ipc(
                        Path::new("datasets/training"),
                        &TrainingDatasetManifest {
                            bar_interval: app_config.data.bar_interval.clone(),
                            dataset_version: app_config.data.dataset_version.clone(),
                            feature_set: feature_set.clone(),
                            feature_set_version: app_config.features.feature_set_version,
                            symbol: symbol.clone(),
                            content_sha256: String::new(),
                            source_market_sha256: quant_data::read_manifest(
                                "datasets/market",
                                &app_config.data.dataset_version,
                                symbol,
                            )?
                            .content_sha256,
                            row_count: 0,
                            feature_columns: Vec::new(),
                            target_horizon: app_config.features.target_horizon as usize,
                        },
                        &rows,
                        &targets,
                    )
                    .with_context(|| {
                        format!("Failed to persist training dataset for {}", symbol)
                    })?;

                    info!(
                        symbol = %symbol,
                        output_file = ?out_path,
                        training_file = ?training_path,
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
