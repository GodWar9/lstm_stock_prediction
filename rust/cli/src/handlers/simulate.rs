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
        bail!(
            "Backtest report file not found at: {}",
            args.report.display()
        );
    }

    let report_content = std::fs::read_to_string(&args.report)
        .with_context(|| format!("Failed to read report at {}", args.report.display()))?;

    let report: BacktestReport = serde_json::from_str(&report_content)
        .with_context(|| "Failed to parse backtest report JSON")?;

    if args.paths == 0
        || args.paths > 100_000
        || report.returns.len().saturating_mul(args.paths) > 5_000_000
    {
        bail!("Simulation requires 1..100000 paths and at most 5 million path steps");
    }
    if report.returns.is_empty() {
        bail!("Report has no returns to resample");
    }
    let resampler = MonteCarloResampler::new(42);
    let sim_result = resampler.generate_paths(&report, args.paths);

    let bands = resampler.nav_bands(&report, args.paths);
    let root = std::path::Path::new("reports/runs");
    let source_dir = args.report.parent().unwrap_or(std::path::Path::new("."));
    let provenance = if source_dir.join("manifest.json").exists() {
        let source: quant_api::contract::RunManifest =
            serde_json::from_slice(&std::fs::read(source_dir.join("manifest.json"))?)?;
        source.provenance
    } else {
        quant_api::contract::Provenance {
            git_commit: "unknown".into(),
            source_snapshot: None,
            config_hash: "unknown".into(),
            data_version: "unknown".into(),
            model_artifact_id: "unknown".into(),
            source: "unverified report".into(),
        }
    };
    let mut manifest =
        quant_api::artifacts::publish_backtest(root, &report, provenance, "unknown", "unverified")?;
    manifest.kind = "simulation".into();
    manifest.capabilities.push("monte_carlo".into());
    let dir = root.join(&manifest.run_id);
    std::fs::write(
        dir.join("simulation.json"),
        serde_json::to_vec_pretty(&sim_result)?,
    )?;
    std::fs::write(
        dir.join("bands.arrow"),
        quant_api::artifacts::arrow_bytes(&[
            ("step", bands.iter().map(|(i, _)| *i as f64).collect()),
            ("p5", bands.iter().map(|(_, d)| d.p5).collect()),
            ("p50", bands.iter().map(|(_, d)| d.p50).collect()),
            ("p95", bands.iter().map(|(_, d)| d.p95).collect()),
        ])?,
    )?;
    manifest
        .artifacts
        .insert("simulation".into(), "simulation.json".into());
    manifest
        .artifacts
        .insert("simulation_bands".into(), "bands.arrow".into());
    manifest.warnings.push("Pointwise bootstrap bands are scenarios, not forecast confidence intervals. Expected block length: 5 bars; seed: 42.".into());
    quant_api::artifacts::write_manifest(&dir, &manifest)?;
    println!("Simulation run: {}", manifest.run_id);
    println!("==================================================");
    println!("        QUANTCTL MONTE CARLO SIMULATION           ");
    println!("==================================================");
    println!("Simulated Paths:    {}", sim_result.num_paths);
    println!(
        "Probability of Ruin:{:.2}%",
        sim_result.prob_of_ruin * 100.0
    );
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
