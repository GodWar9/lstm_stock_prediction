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
        Commands::Data(args) => {
            handlers::data::handle_data(&args.command, &cli.config)?;
        }
        Commands::Features(args) => {
            handlers::features::handle_features(&args.command, &cli.config)?;
        }
        Commands::Train(args) => {
            handlers::train::handle_train(args, &cli.config)?;
        }
        Commands::ExportModel(args) => {
            handlers::export_model::handle_export_model(args, &cli.config)?;
        }
        Commands::Predict(args) => {
            handlers::predict::handle_predict(args, &cli.config)?;
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
