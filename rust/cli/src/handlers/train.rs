//! Handler for `quantctl train` command invoking Python PyTorch training loop.

use crate::commands::TrainArgs;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{error, info};

pub fn handle_train(args: &TrainArgs, global_config_path: &Path) -> Result<()> {
    let config_file = args.train_config.as_deref().unwrap_or(global_config_path);
    let resolved = quant_config::load_config(config_file)?;
    if resolved.data.symbols.len() != 1 {
        anyhow::bail!("Training currently supports one symbol per model; configure exactly one data.symbols entry");
    }

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

    // Resolve target Python interpreter: QUANTCTL_PYTHON > VIRTUAL_ENV > python/.venv > PATH
    let python_bin = std::env::var("QUANTCTL_PYTHON")
        .map(PathBuf::from)
        .ok()
        .or_else(|| {
            if let Ok(venv) = std::env::var("VIRTUAL_ENV") {
                let venv_dir = PathBuf::from(venv);
                let candidates = [
                    venv_dir.join("bin/python"),
                    venv_dir.join("Scripts/python.exe"),
                ];
                if let Some(p) = candidates.iter().find(|p| p.exists()) {
                    return Some(p.clone());
                }
            }
            None
        })
        .or_else(|| {
            let venv_candidates = [
                work_dir.join("python/.venv/Scripts/python.exe"),
                work_dir.join("python/.venv/bin/python"),
                PathBuf::from("python/.venv/Scripts/python.exe"),
                PathBuf::from("python/.venv/bin/python"),
                PathBuf::from("../python/.venv/Scripts/python.exe"),
                PathBuf::from("../python/.venv/bin/python"),
                PathBuf::from("../../python/.venv/Scripts/python.exe"),
                PathBuf::from("../../python/.venv/bin/python"),
            ];
            venv_candidates.iter().find(|p| p.exists()).cloned()
        })
        .unwrap_or_else(|| PathBuf::from("python"));

    info!(
        python = %python_bin.display(),
        script = %script_path.display(),
        config = %config_file.display(),
        "Launching PyTorch training orchestrator subprocess"
    );

    // Pass precisely the validated configuration (including QUANTCTL_* overrides)
    // to Python instead of letting the two languages train with different settings.
    let resolved_dir = tempfile::tempdir()?;
    let abs_config = resolved_dir.path().join("resolved-config.yaml");
    std::fs::write(&abs_config, serde_yaml::to_string(&resolved)?)?;

    let mut cmd = Command::new(&python_bin);
    cmd.env("QUANTCTL_EXECUTABLE", std::env::current_exe()?);
    cmd.arg(&script_path).arg("--config").arg(&abs_config);
    if let Some(dataset) = &args.dataset {
        cmd.arg("--dataset")
            .arg(std::fs::canonicalize(dataset).context("Training dataset does not exist")?);
    }
    if let Some(manifest) = &args.manifest {
        cmd.arg("--manifest")
            .arg(std::fs::canonicalize(manifest).context("Training manifest does not exist")?);
    }
    if args.synthetic {
        cmd.arg("--synthetic");
    }
    if args.walk_forward {
        cmd.arg("--walk-forward")
            .arg("--folds")
            .arg(args.folds.to_string());
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
