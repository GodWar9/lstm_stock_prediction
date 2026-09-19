//! Handlers for `quantctl config` subcommands.

use crate::commands::ConfigSubcommands;
use quant_config::load_config;
use std::path::Path;
use tracing::info;

pub fn handle_config(command: &ConfigSubcommands, config_path: &Path) -> anyhow::Result<()> {
    match command {
        ConfigSubcommands::Validate => {
            info!(path = %config_path.display(), "Validating configuration file");
            match load_config(config_path) {
                Ok(cfg) => {
                    println!(
                        "SUCCESS: Configuration at '{}' is valid.",
                        config_path.display()
                    );
                    println!("  Environment      : {}", cfg.env);
                    println!("  Symbols          : {:?}", cfg.data.symbols);
                    println!(
                        "  Feature Set      : {} (v{})",
                        cfg.features.feature_set, cfg.features.feature_set_version
                    );
                    println!("  Lookback Window  : {} bars", cfg.features.lookback);
                    println!(
                        "  Target Horizon   : {} day(s) [{}]",
                        cfg.features.target_horizon, cfg.features.target_transformation
                    );
                    println!("  Model ID         : {}", cfg.training.model_id);
                    println!("  Initial Cash     : ${:.2}", cfg.backtest.initial_cash);
                    Ok(())
                }
                Err(e) => {
                    eprintln!(
                        "ERROR: Configuration validation failed for '{}': {}",
                        config_path.display(),
                        e
                    );
                    Err(anyhow::anyhow!("Configuration validation failed: {}", e))
                }
            }
        }
        ConfigSubcommands::Show => {
            let cfg = load_config(config_path)?;
            let serialized = serde_yaml::to_string(&cfg)?;
            println!("{}", serialized);
            Ok(())
        }
    }
}
