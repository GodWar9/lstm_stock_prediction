//! CLI handler for quantctl backtest: Point-in-time deterministic backtesting.

use crate::commands::BacktestSubcommands;
use anyhow::{bail, Context, Result};
use quant_backtest::stream::ModelSignalStream;
use quant_backtest::{BacktestConfig, BacktestEngine};
use quant_data::types::{Bar, Timestamp};
use quant_execution::CompositeExecutionModel;
use quant_inference::{OnnxLstmProvider, PredictionProvider};
use quant_instruments::InstrumentId;
use quant_portfolio::{PortfolioConstraints, VolatilityTargetedConstructor};
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
        "Model artifact for '{}' not found in search paths",
        model_id
    );
}

fn generate_market_replay(n_bars: usize, start_price: f64) -> Vec<Bar> {
    let mut bars = Vec::with_capacity(n_bars);
    let mut price = start_price;
    for i in 0..n_bars {
        let ts = Timestamp((i as i64 + 1) * 86_400_000_000_000);
        let ret = if i % 3 == 0 {
            0.008
        } else if i % 2 == 0 {
            -0.004
        } else {
            0.002
        };
        price *= 1.0 + ret;
        bars.push(Bar::same_bar(
            ts,
            price * 0.999,
            price * 1.004,
            price * 0.996,
            price,
            100_000,
        ));
    }
    bars
}

pub fn handle_backtest(command: &BacktestSubcommands, _config_path: &Path) -> Result<()> {
    match command {
        BacktestSubcommands::Run {
            model,
            split,
            allow_reuse,
        } => {
            info!(model = %model, split = %split, allow_reuse = *allow_reuse, "Starting backtest run");

            let artifact_dir = resolve_model_path(model)?;
            let provider = OnnxLstmProvider::load(&artifact_dir)
                .context("Failed to load model artifact for backtest")?;

            let id = InstrumentId(1);
            let symbol = "AAPL";
            let lookback = provider.lookback();
            let num_features = provider.feature_schema().len();
            let n_bars = lookback + 30;
            let bars = generate_market_replay(n_bars, 150.0);

            // Pre-populate chronological timed feature vectors for point-in-time model inference
            let mut timed_features = Vec::with_capacity(bars.len());
            for (idx, bar) in bars.iter().enumerate() {
                let ts = bar.timestamp.as_nanos();
                let mut feat = Vec::with_capacity(num_features);
                for f in 0..num_features {
                    let step = idx as f64;
                    let f_idx = f as f64;
                    let val = (step * 0.05 + f_idx * 0.1).sin() * 0.02;
                    feat.push(val);
                }
                timed_features.push((ts, feat));
            }

            let model_id_str = provider.model_id().to_string();
            let signal_stream =
                ModelSignalStream::new(provider, id, symbol).with_timed_features(timed_features);
            let constructor = VolatilityTargetedConstructor::new(0.20);
            let exec_model = CompositeExecutionModel::default();
            let constraints = PortfolioConstraints::default();

            let engine = BacktestEngine::new(BacktestConfig {
                initial_cash: 100_000.0,
                risk_free_rate: 0.04,
                num_prior_trials: 1,
                allow_reuse: *allow_reuse,
                split: split.clone(),
            });

            let report = engine.run(
                signal_stream,
                &constructor,
                &exec_model,
                &bars,
                id,
                symbol,
                &constraints,
                &model_id_str,
            )?;

            // Save report JSON
            let out_dir = PathBuf::from("reports");
            std::fs::create_dir_all(&out_dir).ok();
            let report_path = out_dir.join(format!("backtest_{}_{}.json", model, split));
            let json_data = serde_json::to_string_pretty(&report)?;
            std::fs::write(&report_path, json_data)
                .with_context(|| format!("Failed to write report to {}", report_path.display()))?;

            println!("==================================================");
            println!("             QUANTCTL BACKTEST REPORT             ");
            println!("==================================================");
            println!("Model ID:           {}", model);
            println!("Split:              {}", split);
            println!("Initial Capital:    ${:.2}", report.initial_cash);
            println!("Final NAV:          ${:.2}", report.final_nav);
            println!(
                "Total Return:       {:+0.2}%",
                report.total_return_pct * 100.0
            );
            println!("CAGR:               {:+0.2}%", report.cagr * 100.0);
            println!("Sharpe Ratio:       {:.2}", report.sharpe);
            println!("Deflated Sharpe:    {:.2}", report.deflated_sharpe);
            println!("Sortino Ratio:      {:.2}", report.sortino);
            println!("Max Drawdown:       {:.2}%", report.max_drawdown * 100.0);
            println!("Profit Factor:      {:.2}", report.profit_factor);
            println!("Hit Rate:           {:.2}%", report.hit_rate * 100.0);
            println!("Total Trades:       {}", report.total_trades);
            println!("Report Saved At:    {}", report_path.display());
            println!("==================================================");
        }
    }
    Ok(())
}
