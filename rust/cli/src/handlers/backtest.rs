//! Model-backed, held-out replay over persisted market data.
use crate::commands::BacktestSubcommands;
use anyhow::{bail, Context, Result};
use quant_api::{
    artifacts::{arrow_bytes, publish_backtest, write_manifest},
    contract::Provenance,
};
use quant_backtest::{stream::ManualSignalStream, BacktestConfig, BacktestEngine};
use quant_execution::CompositeExecutionModel;
use quant_inference::{OnnxLstmProvider, PredictionProvider};
use quant_instruments::InstrumentId;
use quant_portfolio::{LongShortMode, PortfolioConstraints, VolatilityTargetedConstructor};
use quant_signals::{SignalCalibrator, SignalConfig};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

pub fn handle_backtest(command: &BacktestSubcommands, config_path: &Path) -> Result<()> {
    let BacktestSubcommands::Run {
        model,
        split,
        allow_reuse,
    } = command;
    if !matches!(split.as_str(), "test" | "validation" | "train") {
        bail!("split must be test, validation, or train");
    }
    let cfg = quant_config::load_config(config_path)?;
    let symbol = &cfg.data.symbols[0];
    let artifact_dir = Path::new("models").join(model);
    let provider = OnnxLstmProvider::load(&artifact_dir)?;
    let validation: serde_json::Value = serde_json::from_slice(
        &fs::read(artifact_dir.join("validation.json"))
            .context("Retrain to produce explicit split and provenance records")?,
    )?;
    let period = &validation["periods"][split];
    let start: i64 = period[0].as_str().context("Missing split start")?.parse()?;
    let end: i64 = period[1].as_str().context("Missing split end")?.parse()?;
    let all_bars = super::pipeline::bars(config_path, symbol)?;
    let data_manifest =
        quant_data::read_manifest("datasets/market", &cfg.data.dataset_version, symbol)?;
    if validation["data_version"].as_str() != Some(cfg.data.dataset_version.as_str()) {
        bail!("Model and market dataset versions differ");
    }
    let rows = super::pipeline::graph().compute_batch(&all_bars);
    let ordered = super::pipeline::ordered(&rows, provider.feature_schema())?;
    let bars: Vec<_> = all_bars
        .iter()
        .filter(|b| b.timestamp.as_nanos() >= start && b.timestamp.as_nanos() <= end)
        .cloned()
        .collect();
    if bars.len() < 2 {
        bail!("Requested split has fewer than two market bars");
    }
    let model_hash = format!(
        "{:x}",
        Sha256::digest(fs::read(artifact_dir.join("model.onnx"))?)
    );
    fs::create_dir_all("reports/evaluations")?;
    let guard = Path::new("reports/evaluations").join(format!("{model_hash}_{split}.lock"));
    let claimed = if split == "test" && !allow_reuse {
        Some(fs::OpenOptions::new().write(true).create_new(true).open(&guard).context("Test split already evaluated (or evaluation in progress); use --allow-reuse deliberately")?)
    } else {
        None
    };
    let result = (|| -> Result<()> {
        let lookback = provider.lookback();
        let calibrator = SignalCalibrator::new(SignalConfig::default());
        let mut signals = Vec::new();
        for window in ordered.windows(lookback) {
            let ts = window.last().unwrap().0;
            if ts < start || ts > end {
                continue;
            }
            let flat: Vec<_> = window.iter().flat_map(|(_, v)| v.iter().copied()).collect();
            let prediction = provider.predict(&flat, lookback, provider.feature_schema().len())?;
            signals.push(calibrator.calibrate(&prediction, InstrumentId(1), symbol, ts));
        }
        if signals.is_empty() {
            bail!("No model predictions were produced for the split");
        }
        let constraints = PortfolioConstraints {
            max_gross_exposure: cfg.portfolio.max_gross_exposure,
            max_net_exposure: cfg.portfolio.max_net_exposure,
            max_position_pct: cfg.portfolio.max_position_pct,
            volatility_target: cfg.portfolio.volatility_target,
            long_short_mode: match cfg.portfolio.long_short_mode.as_str() {
                "LongOnly" => LongShortMode::LongOnly,
                "LongShort" => LongShortMode::LongShort,
                "DollarNeutral" => LongShortMode::DollarNeutral,
                _ => bail!("Unknown portfolio mode"),
            },
            ..Default::default()
        };
        let report = BacktestEngine::new(BacktestConfig {
            initial_cash: cfg.backtest.initial_cash,
            risk_free_rate: cfg.backtest.risk_free_rate,
            num_prior_trials: 1,
            allow_reuse: *allow_reuse,
            split: split.clone(),
        })
        .run(
            ManualSignalStream::from_signals(signals.clone()),
            &VolatilityTargetedConstructor::new(0.20),
            &CompositeExecutionModel::new(
                cfg.execution.fixed_commission,
                1.0,
                cfg.execution.half_spread_bps,
                cfg.execution.slippage_factor,
                cfg.execution.participation_cap,
            ),
            &bars,
            InstrumentId(1),
            symbol,
            &constraints,
            model,
        )?;
        let git = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|| "unknown".into());
        let provenance = Provenance {
            git_commit: git,
            config_hash: format!("{:x}", Sha256::digest(serde_json::to_vec(&cfg)?)),
            data_version: cfg.data.dataset_version.clone(),
            model_artifact_id: model.clone(),
            source: data_manifest.source,
        };
        let mut manifest = publish_backtest(
            Path::new("reports/runs"),
            &report,
            provenance,
            symbol,
            split,
        )?;
        let dir = Path::new("reports/runs").join(&manifest.run_id);
        fs::write(
            dir.join("signals.arrow"),
            arrow_bytes(&[
                (
                    "timestamp_ms",
                    signals
                        .iter()
                        .map(|s| (s.as_of / 1_000_000) as f64)
                        .collect(),
                ),
                (
                    "expected_return",
                    signals.iter().map(|s| s.expected_return).collect(),
                ),
                ("confidence", signals.iter().map(|s| s.confidence).collect()),
            ])?,
        )?;
        fs::write(
            dir.join("validation.json"),
            serde_json::to_vec_pretty(&validation)?,
        )?;
        manifest
            .capabilities
            .extend(["signals".into(), "validation".into()]);
        manifest
            .artifacts
            .insert("signals".into(), "signals.arrow".into());
        manifest
            .artifacts
            .insert("validation".into(), "validation.json".into());
        if *allow_reuse {
            manifest.warnings.push(
                "Test reuse explicitly allowed; repeated selection can bias performance.".into(),
            );
        }
        write_manifest(&dir, &manifest)?;
        let report_path = format!("reports/backtest_{model}_{split}.json");
        fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
        println!(
            "QUANTCTL BACKTEST REPORT\nRun ID: {}\nSharpe Ratio: {:.4}\nReport Saved At: {}",
            manifest.run_id, report.sharpe, report_path
        );
        Ok(())
    })();
    drop(claimed);
    if result.is_err() && !allow_reuse && split == "test" {
        let _ = fs::remove_file(guard);
    }
    result
}
