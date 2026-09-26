//! Reproducible wall-clock measurements over the configured dataset.
use crate::commands::BenchmarkArgs;
use anyhow::{bail, Context, Result};
use quant_inference::{OnnxLstmProvider, PredictionProvider};
use std::{path::Path, time::Instant};

pub fn handle_benchmark(args: &BenchmarkArgs, config_path: &Path) -> Result<()> {
    if !matches!(args.suite.as_str(), "features" | "inference" | "backtest") {
        bail!("Unknown benchmark suite; choose features, inference, or backtest");
    }
    let cfg = quant_config::load_config(config_path)?;
    let symbol = &cfg.data.symbols[0];
    let bars = super::pipeline::bars(config_path, symbol)?;
    let provider = if args.suite == "features" {
        None
    } else {
        Some(OnnxLstmProvider::load(
            Path::new("models").join(
                args.model
                    .as_ref()
                    .context("--model is required for inference/backtest benchmarks")?,
            ),
        )?)
    };
    let mut input = Vec::new();
    if let Some(model) = &provider {
        let rows = super::pipeline::graph().compute_batch(&bars);
        let ordered = super::pipeline::ordered(&rows, model.feature_schema())?;
        if ordered.len() < model.lookback() {
            bail!("Dataset is shorter than model lookback");
        }
        input = ordered[ordered.len() - model.lookback()..]
            .iter()
            .flat_map(|(_, v)| v.iter().copied())
            .collect();
    }
    let mut times = Vec::new();
    for iteration in 0..=args.iterations {
        let start = Instant::now();
        match args.suite.as_str() {
            "features" => {
                std::hint::black_box(super::pipeline::graph().compute_batch(&bars));
            }
            "inference" => {
                let model = provider.as_ref().unwrap();
                std::hint::black_box(model.predict(
                    &input,
                    model.lookback(),
                    model.feature_schema().len(),
                )?);
            }
            _ => {
                let prediction = provider.as_ref().unwrap().predict(
                    &input,
                    provider.as_ref().unwrap().lookback(),
                    provider.as_ref().unwrap().feature_schema().len(),
                )?;
                // A fixed signal workload measures engine execution, not strategy quality.
                let calibrator =
                    quant_signals::SignalCalibrator::new(quant_signals::SignalConfig::default());
                let signals = bars
                    .iter()
                    .map(|b| {
                        calibrator.calibrate(
                            &prediction,
                            quant_instruments::InstrumentId(1),
                            symbol,
                            b.timestamp.as_nanos(),
                        )
                    })
                    .collect();
                let report = quant_backtest::BacktestEngine::new(Default::default()).run(
                    quant_backtest::stream::ManualSignalStream::from_signals(signals),
                    &quant_portfolio::VolatilityTargetedConstructor::new(0.2),
                    &quant_execution::CompositeExecutionModel::new(
                        cfg.execution.fixed_commission,
                        1.0,
                        cfg.execution.half_spread_bps,
                        cfg.execution.slippage_factor,
                        cfg.execution.participation_cap,
                    ),
                    &bars,
                    quant_instruments::InstrumentId(1),
                    symbol,
                    &quant_portfolio::PortfolioConstraints::default(),
                    "benchmark",
                )?;
                std::hint::black_box(report);
            }
        }
        if iteration > 0 {
            times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
    }
    times.sort_by(f64::total_cmp);
    let report = serde_json::json!({"suite": args.suite, "iterations": args.iterations, "warmup_iterations": 1,
        "bars": bars.len(), "model": args.model, "dataset_version": cfg.data.dataset_version,
        "build_profile": if cfg!(debug_assertions) {"debug"} else {"release"},
        "os": std::env::consts::OS, "architecture": std::env::consts::ARCH,
        "logical_cpus": std::thread::available_parallelism().map(|v| v.get()).unwrap_or(1),
        "median_ms": times[times.len()/2], "p95_ms": times[((times.len() as f64 * 0.95).ceil() as usize).saturating_sub(1)],
        "samples_ms": times, "workload": "local CPU; backtest uses fixed signals for engine timing, not performance evaluation"});
    std::fs::create_dir_all("reports/benchmarks")?;
    let path = format!("reports/benchmarks/{}.json", uuid::Uuid::new_v4());
    std::fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::json!({"report": report, "path": path}));
    Ok(())
}
