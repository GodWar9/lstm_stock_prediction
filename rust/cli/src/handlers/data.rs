//! Handlers for `quantctl data` subcommands.

use crate::commands::DataSubcommands;
use chrono::NaiveDate;
use quant_config::load_config;
use quant_data::{
    check_duplicate_timestamps, read_dataset, validate_bars_monotonic_and_sound, write_dataset,
    DatasetManifest, LocalCsvProvider, MarketDataProvider, SyntheticDataProvider, YfinanceAdapter,
};
use std::path::Path;
use tracing::info;

pub fn handle_data(command: &DataSubcommands, config_path: &Path) -> anyhow::Result<()> {
    let cfg = load_config(config_path)?;

    match command {
        DataSubcommands::Ingest { symbol, journal } => {
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

            if cfg.data.provider == "alpaca_journal" {
                let journal = journal.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("--journal <closed capture directory> is required")
                })?;
                for sym in &target_symbols {
                    let (audit, bars) =
                        quant_data::capture::import_minutes(journal, sym, start, end)?;
                    let manifest = DatasetManifest {
                        market: None,
                        bar_interval: "1m".into(),
                        capture_sha256: Some(audit.content_sha256),
                        dataset_version: cfg.data.dataset_version.clone(),
                        symbol: sym.clone(),
                        start_date: start.to_string(),
                        end_date: end.to_string(),
                        bar_count: bars.len(),
                        source: format!(
                            "{}alpaca:{}:first_publication",
                            if audit.synthetic {
                                "synthetic_capture:"
                            } else {
                                ""
                            },
                            audit.feed
                        ),
                        content_sha256: String::new(),
                    };
                    let path = write_dataset("datasets/market", &manifest, &bars)?;
                    println!(
                        "Imported {} PIT one-minute bars to {}",
                        bars.len(),
                        path.display()
                    );
                }
                return Ok(());
            }
            if journal.is_some() {
                anyhow::bail!("--journal requires provider: alpaca_journal");
            }

            if cfg.data.provider == "twelve_data" {
                anyhow::ensure!(
                    target_symbols == cfg.data.symbols,
                    "Global ingest must use the configured storage symbol"
                );
                let market = cfg.data.market.clone().expect("validated market config");
                let interval = cfg.data.bar_interval.clone();
                // reqwest's blocking runtime must be created outside the CLI Tokio runtime.
                let snapshot = std::thread::spawn(move || {
                    quant_data::global::fetch(&market, &interval, Some((start, end)), 5000)
                })
                .join()
                .map_err(|_| anyhow::anyhow!("Market data worker failed"))??;
                let manifest = DatasetManifest {
                    market: cfg.data.market.clone(),
                    bar_interval: cfg.data.bar_interval.clone(),
                    capture_sha256: None,
                    dataset_version: cfg.data.dataset_version.clone(),
                    symbol: target_symbols[0].clone(),
                    start_date: start.to_string(),
                    end_date: end.to_string(),
                    bar_count: snapshot.bars.len(),
                    source: "twelve_data:unadjusted:rest_snapshot".into(),
                    content_sha256: String::new(),
                };
                let path = write_dataset("datasets/market", &manifest, &snapshot.bars)?;
                println!("Imported {} global market bars to {}. Historical availability is estimated; revised REST data is not a first-publication capture.", snapshot.bars.len(), path.display());
                return Ok(());
            }

            // Synthetic data is opt-in for development and tests; production ingestion must fail
            // instead of silently replacing missing market data.
            let provider: Box<dyn MarketDataProvider> = if cfg.data.provider == "csv" {
                Box::new(LocalCsvProvider::new(&cfg.data.input_dir))
            } else if cfg.data.provider == "yfinance" {
                Box::new(YfinanceAdapter::default_paths())
            } else if cfg.data.provider == "synthetic" {
                Box::new(SyntheticDataProvider::default())
            } else {
                anyhow::bail!("Unknown data provider: {}", cfg.data.provider);
            };

            for sym in &target_symbols {
                quant_data::validate_storage_id(sym)?;
                println!(
                    "Ingesting historical bars for '{}' [{} -> {}]...",
                    sym, start, end
                );
                let bars = provider.fetch_ohlcv(sym, start, end)?;
                validate_bars_monotonic_and_sound(&bars)?;
                check_duplicate_timestamps(&bars)?;
                let manifest = DatasetManifest {
                    market: None,
                    bar_interval: "1d".into(),
                    capture_sha256: None,
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
        DataSubcommands::VerifyCapture { journal } => {
            let audit = quant_data::capture::verify(journal)?;
            println!("{}", serde_json::to_string_pretty(&audit)?);
            if !audit.clean_shutdown {
                anyhow::bail!("Capture is not cleanly closed; not eligible for research import");
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
