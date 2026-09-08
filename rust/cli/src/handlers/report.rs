//! CLI handler for quantctl report: Comprehensive reporting across backtest and simulation.

use crate::commands::ReportArgs;
use anyhow::{bail, Context, Result};
use quant_backtest::BacktestReport;
use std::path::Path;
use tracing::info;

pub fn handle_report(args: &ReportArgs, _config_path: &Path) -> Result<()> {
    info!(report_file = %args.backtest.display(), "Generating detailed strategy analytics report");

    if !args.backtest.exists() {
        bail!("Backtest report file not found at: {}", args.backtest.display());
    }

    let content = std::fs::read_to_string(&args.backtest)
        .with_context(|| format!("Failed to read report at {}", args.backtest.display()))?;

    let report: BacktestReport = serde_json::from_str(&content)
        .with_context(|| "Failed to parse backtest report JSON")?;

    let is_successful = report.sharpe > 0.5 && report.max_drawdown < 0.35 && report.total_return_pct > 0.0;

    println!("==================================================");
    println!("        QUANTITATIVE STRATEGY AUDIT REPORT        ");
    println!("==================================================");
    println!("Validation Status:  {}", if is_successful { "VALIDATED / PASS" } else { "REJECTED / UNFAVORABLE" });
    println!("Initial Capital:    ${:.2}", report.initial_cash);
    println!("Final NAV:          ${:.2}", report.final_nav);
    println!("Cumulative Return:  {:+0.2}%", report.total_return_pct * 100.0);
    println!("Annualized CAGR:    {:+0.2}%", report.cagr * 100.0);
    println!("Annualized Sharpe:  {:.2}", report.sharpe);
    println!("Deflated Sharpe:    {:.2}", report.deflated_sharpe);
    println!("Sortino Ratio:      {:.2}", report.sortino);
    println!("Calmar Ratio:       {:.2}", report.calmar);
    println!("Max Drawdown:       {:.2}%", report.max_drawdown * 100.0);
    println!("Profit Factor:      {:.2}", report.profit_factor);
    println!("Hit Rate:           {:.2}%", report.hit_rate * 100.0);
    println!("Avg Win / Avg Loss: {:.2}% / {:.2}%", report.avg_win * 100.0, report.avg_loss * 100.0);
    println!("Total Trades:       {}", report.total_trades);
    println!("Winning Trades:     {}", report.winning_trades);
    println!("Losing Trades:      {}", report.losing_trades);
    println!("==================================================");

    if !is_successful {
        println!("NOTE: Strategy does not meet production thresholds despite ML loss convergence.");
    }

    Ok(())
}
