//! Environment, system, and version reporting for quantctl.

use std::path::Path;
use quant_config::load_config;

pub fn handle_env(config_path: &Path) {
    println!("=== quantctl Environment & Build Metadata ===");
    println!("  CLI Version       : {}", env!("CARGO_PKG_VERSION"));
    println!("  Target OS         : {}", std::env::consts::OS);
    println!("  Target Arch       : {}", std::env::consts::ARCH);
    println!("  Host Family       : {}", std::env::consts::FAMILY);

    let cwd = std::env::current_dir().unwrap_or_default();
    println!("  Working Directory : {}", cwd.display());
    println!("  Target Config     : {}", config_path.display());

    match load_config(config_path) {
        Ok(cfg) => {
            println!("  Active Env Name   : {}", cfg.env);
            println!("  Active Provider   : {}", cfg.data.provider);
            println!("  Active Model ID   : {}", cfg.training.model_id);
            println!("  Config Status     : VALID");
        }
        Err(e) => {
            println!("  Config Status     : INVALID ({})", e);
        }
    }
}
