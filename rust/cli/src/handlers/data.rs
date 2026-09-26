//! Handlers for `quantctl data` subcommands.

use crate::commands::DataSubcommands;
use chrono::NaiveDate;
use quant_config::load_config;
use quant_data::{
    check_duplicate_timestamps, read_dataset, validate_bars_monotonic_and_sound, write_dataset,
    DatasetManifest, MarketDataProvider, SyntheticDataProvider, YfinanceAdapter,
};
use std::path::Path;
use tracing::info;

pub fn handle_data(command: &DataSubcommands, config_path: &Path) -> anyhow::Result<()> {
    let cfg = load_config(config_path)?;

    match command {
        DataSubcommands::Ingest { symbol } => {
            let start = NaiveDate::parse_from_str(&cfg.data.start_date, "%Y-%m-%d")?;
            let end = NaiveDate::parse_from_str(&cfg.data.end_date, "%Y-%m-%d")?;

            let target_symbols: Vec<String> = if let Some(sym) = symbol {
                vec![sym.clone()]
            } else {
                cfg.data.symbols.clone()
            };

            info!(
                provider = %cfg.data.provider,
                symbols = ?target_symbols,
                start = %start,
                end = %end,
                "Starting market data ingestion"
            );

            // Synthetic data is opt-in for development and tests; production ingestion must fail
            // instead of silently replacing missing market data.
            let provider: Box<dyn MarketDataProvider> = if cfg.data.provider == "yfinance" {
                Box::new(YfinanceAdapter::default_paths())
            } else if cfg.data.provider == "synthetic" {
                Box::new(SyntheticDataProvider::default())
            } else {
                anyhow::bail!("Unknown data provider: {}", cfg.data.provider);
            };

            for sym in &target_symbols {
                println!(
                    "Ingesting historical bars for '{}' [{} -> {}]...",
                    sym, start, end
                );
                let bars = provider.fetch_ohlcv(sym, start, end)?;
                validate_bars_monotonic_and_sound(&bars)?;
                check_duplicate_timestamps(&bars)?;
                let manifest = DatasetManifest {
                    dataset_version: cfg.data.dataset_version.clone(),
                    symbol: sym.clone(),
                    start_date: start.to_string(),
                    end_date: end.to_string(),
                    bar_count: bars.len(),
                    source: cfg.data.provider.clone(),
                    content_sha256: String::new(),
                };
                let path = write_dataset("datasets/market", &manifest, &bars)?;
                println!(
                    "SUCCESS: Persisted {} validated bars for '{}' at {}.",
                    bars.len(),
                    sym,
                    path.display()
                );
            }
            Ok(())
        }
        DataSubcommands::Validate { dataset_version } => {
            let version = dataset_version
                .as_deref()
                .unwrap_or(&cfg.data.dataset_version);
            info!(dataset_version = %version, "Validating ingested dataset integrity");
            for symbol in &cfg.data.symbols {
                let bars = read_dataset("datasets/market", version, symbol)?;
                validate_bars_monotonic_and_sound(&bars)?;
                check_duplicate_timestamps(&bars)?;
                println!("VALID: {} ({} bars)", symbol, bars.len());
            }
            println!(
                "SUCCESS: Dataset '{}' passed persisted-data validation.",
                version
            );
            Ok(())
        }
    }
}
