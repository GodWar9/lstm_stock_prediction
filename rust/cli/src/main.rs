//! quantctl: Quantitative Research & Trading Platform CLI

mod commands;
mod handlers;
mod telemetry;

use clap::Parser;
use commands::{Cli, Commands};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    telemetry::init_telemetry(&cli.log_format, cli.verbose)?;

    info!(version = env!("CARGO_PKG_VERSION"), "quantctl initialized");

    match &cli.command {
        Commands::SourceSnapshot => println!(
            "{}",
            serde_json::to_string(&quant_api::provenance::snapshot(std::path::Path::new("."))?)?
        ),
        Commands::VerifyModel { artifact_dir } => println!(
            "{}",
            quant_inference::parity::verify_runtime_parity(artifact_dir)?
        ),
        Commands::Serve { port, root } => quant_api::serve(root.clone(), *port).await?,
        Commands::Openapi => println!("{}", quant_api::openapi()),
        Commands::Config(args) => {
            handlers::config::handle_config(&args.command, &cli.config)?;
        }
        Commands::Data(args) => {
            handlers::data::handle_data(&args.command, &cli.config)?;
        }
        Commands::Research { journal, paths } => {
            handlers::research::run(journal, *paths, &cli.config)?
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
        Commands::Backtest(args) => {
            handlers::backtest::handle_backtest(&args.command, &cli.config)?;
        }
        Commands::Simulate(args) => {
            handlers::simulate::handle_simulate(args, &cli.config)?;
        }
        Commands::Report(args) => {
            handlers::report::handle_report(args, &cli.config)?;
        }
        Commands::Benchmark(args) => handlers::benchmark::handle_benchmark(args, &cli.config)?,
        Commands::Env => {
            handlers::env::handle_env(&cli.config);
        }
    }

    Ok(())
}
