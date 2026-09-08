//! CLI handler for quantctl simulate: Monte Carlo resampling and parameter sensitivity.

use crate::commands::SimulateArgs;
use anyhow::{bail, Context, Result};
use quant_backtest::BacktestReport;
use quant_simulation::{MonteCarloResampler, SimulationStrategy};
use std::path::Path;
use tracing::info;

pub fn handle_simulate(args: &SimulateArgs, _config_path: &Path) -> Result<()> {
    info!(report = %args.report.display(), paths = args.paths, "Running Monte Carlo simulation");

    if !args.report.exists() {
        bail!("Backtest report file not found at: {}", args.report.display());
    }

    let report_content = std::fs::read_to_string(&args.report)
        .with_context(|| format!("Failed to read report at {}", args.report.display()))?;

    let report: BacktestReport = serde_json::from_str(&report_content)
        .with_context(|| "Failed to parse backtest report JSON")?;

    let resampler = MonteCarloResampler::new(42);
    let sim_result = resampler.generate_paths(&report, args.paths);

    println!("==================================================");
    println!("        QUANTCTL MONTE CARLO SIMULATION           ");
    println!("==================================================");
    println!("Simulated Paths:    {}", sim_result.num_paths);
    println!("Probability of Ruin:{:.2}%", sim_result.prob_of_ruin * 100.0);
    println!("--------------------------------------------------");
    println!("Metric              p5        p50       p95");
    println!("--------------------------------------------------");
    println!(
        "Sharpe Ratio        {:+0.2}     {:+0.2}     {:+0.2}",
        sim_result.sharpe_distribution.p5,
        sim_result.sharpe_distribution.p50,
        sim_result.sharpe_distribution.p95
    );
    println!(
        "Max Drawdown        {:.2}%     {:.2}%     {:.2}%",
        sim_result.drawdown_distribution.p5 * 100.0,
        sim_result.drawdown_distribution.p50 * 100.0,
        sim_result.drawdown_distribution.p95 * 100.0
    );
    println!(
        "CAGR                {:+0.2}%    {:+0.2}%    {:+0.2}%",
        sim_result.cagr_distribution.p5 * 100.0,
        sim_result.cagr_distribution.p50 * 100.0,
        sim_result.cagr_distribution.p95 * 100.0
    );
    println!("==================================================");

    Ok(())
}
