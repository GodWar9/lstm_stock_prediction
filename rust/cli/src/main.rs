//! quantctl: Quantitative Research & Trading Platform CLI

mod commands;

use clap::Parser;
use commands::{Cli, Commands};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Config(_) => {
            println!("Executing config command...");
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
            println!("quantctl version {}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}
