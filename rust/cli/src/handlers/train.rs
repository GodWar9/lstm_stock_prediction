//! Handler for `quantctl train` command invoking Python PyTorch training loop.

use crate::commands::TrainArgs;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{error, info};

pub fn handle_train(args: &TrainArgs, global_config_path: &Path) -> Result<()> {
    let config_file = args.train_config.as_deref().unwrap_or(global_config_path);

    // Resolve script path relative to current execution context
    let candidates = [
        PathBuf::from("python/ml/train_orchestrator.py"),
        PathBuf::from("../python/ml/train_orchestrator.py"),
        PathBuf::from("../../python/ml/train_orchestrator.py"),
    ];

    let (script_path, work_dir) = candidates
        .iter()
        .find_map(|p| {
            if p.exists() {
                let abs = std::fs::canonicalize(p).ok()?;
                let root = abs.parent()?.parent()?.parent()?.to_path_buf();
                Some((abs, root))
            } else {
                None
            }
        })
        .unwrap_or_else(|| {
            (
                PathBuf::from("python/ml/train_orchestrator.py"),
                PathBuf::from("."),
            )
        });

    info!(
        script = %script_path.display(),
        config = %config_file.display(),
        "Launching PyTorch training orchestrator subprocess"
    );

    let abs_config =
        std::fs::canonicalize(config_file).unwrap_or_else(|_| config_file.to_path_buf());

    let mut cmd = Command::new("python");
    cmd.arg(&script_path).arg("--config").arg(&abs_config);
    if let Some(dataset) = &args.dataset {
        cmd.arg("--dataset").arg(dataset);
    }
    if let Some(manifest) = &args.manifest {
        cmd.arg("--manifest").arg(manifest);
    }
    if args.synthetic {
        cmd.arg("--synthetic");
    }

    if work_dir.exists() && work_dir != Path::new("") {
        cmd.current_dir(&work_dir);
    }

    let output = cmd
        .output()
        .with_context(|| "Failed to execute python training orchestrator")?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        print!("{}", stdout);
        info!("PyTorch training pipeline finished successfully.");
        println!("Training completed successfully. ONNX model exported to models/ directory.");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        error!(stderr = %stderr, stdout = %stdout, "Python training orchestrator failed");
        eprintln!(
            "Python training error:\nSTDOUT:\n{}\nSTDERR:\n{}",
            stdout, stderr
        );
        Err(anyhow::anyhow!(
            "Training failed with exit status {:?}",
            output.status.code()
        ))
    }
}
