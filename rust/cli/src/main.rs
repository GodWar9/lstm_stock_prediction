//! quantctl: Quantitative Research & Trading Platform CLI

mod commands;
mod handlers;
mod telemetry;

use clap::Parser;
use commands::{Cli, Commands};
use tracing::info;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    telemetry::init_telemetry(&cli.log_format, cli.verbose)?;

    info!(version = env!("CARGO_PKG_VERSION"), "quantctl initialized");

    match &cli.command {
        Commands::Config(args) => {
            handlers::config::handle_config(&args.command, &cli.config)?;
        }
        Commands::Data(_) => {
            println!("Executing data command...");
        }
        Commands::Features(_) => {
            println!("Executing features command...");
        }
        Commands::Train(_) => {
            println!("Executing train command...");
        }
        Commands::ExportModel(_) => {
            println!("Executing export-model command...");
        }
        Commands::Predict(_) => {
            println!("Executing predict command...");
        }
        Commands::Backtest(_) => {
            println!("Executing backtest command...");
        }
        Commands::Simulate(_) => {
            println!("Executing simulate command...");
        }
        Commands::Report(_) => {
            println!("Executing report command...");
        }
        Commands::Benchmark(_) => {
            println!("Executing benchmark command...");
        }
        Commands::Env => {
            handlers::env::handle_env(&cli.config);
        }
    }

    Ok(())
}
