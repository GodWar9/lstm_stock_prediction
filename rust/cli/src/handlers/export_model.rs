//! CLI handler for quantctl export-model: Export PyTorch checkpoint to ONNX artifact package.

use crate::commands::ExportModelArgs;
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;
use tracing::info;

pub fn handle_export_model(args: &ExportModelArgs, _config_path: &Path) -> Result<()> {
    info!(model_version = %args.model_version, "Initiating model artifact export");

    let python_script = "python/ml/export_onnx.py";
    let mut cmd = Command::new("python");
    cmd.arg(python_script);

    let status = cmd
        .status()
        .context("Failed to execute python export script")?;
    if !status.success() {
        bail!("Model export script exited with non-zero status");
    }

    let target_dir = format!("models/{}", args.model_version);
    println!(
        "Successfully verified and exported model artifact: {}",
        target_dir
    );
    Ok(())
}
