//! Re-export an existing trained checkpoint under a new model ID.
use crate::commands::ExportModelArgs;
use anyhow::{bail, Context, Result};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub fn handle_export_model(args: &ExportModelArgs, _config_path: &Path) -> Result<()> {
    let script = [
        "python/ml/export_checkpoint.py",
        "../python/ml/export_checkpoint.py",
        "../../python/ml/export_checkpoint.py",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|p| p.is_file())
    .context("Cannot locate checkpoint exporter")?
    .canonicalize()?;
    let root = script.parent().unwrap().parent().unwrap().parent().unwrap();
    let python = std::env::var_os("QUANTCTL_PYTHON")
        .map(PathBuf::from)
        .or_else(|| {
            [
                root.join("python/.venv/Scripts/python.exe"),
                root.join("python/.venv/bin/python"),
            ]
            .into_iter()
            .find(|p| p.is_file())
        })
        .unwrap_or_else(|| "python".into());
    let source = root.join("models").join(&args.model_version);
    quant_inference::integrity::verify_package(&source)?;
    let output = Command::new(python)
        .arg(&script)
        .arg("--source")
        .arg(source)
        .arg("--model-id")
        .arg(&args.output_model)
        .env("QUANTCTL_EXECUTABLE", std::env::current_exe()?)
        .current_dir(root)
        .output()
        .context("Failed to launch checkpoint exporter")?;
    if !output.status.success() {
        bail!(
            "Checkpoint export failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    print!("{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}
