//! A fresh, recorded CLI workflow; never reuses a saved backtest or model ID.
use anyhow::{ensure, Context, Result};
use serde_json::json;
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
};

pub fn run(journal: &Path, paths: usize, config: &Path) -> Result<()> {
    let mut cfg = quant_config::load_config(config)?;
    ensure!(
        cfg.data.provider == "alpaca_journal" && cfg.data.bar_interval == "1m",
        "Research capture workflow requires alpaca_journal with bar_interval: 1m"
    );
    ensure!(
        cfg.data.symbols.len() == 1,
        "Use one symbol per research model"
    );
    ensure!((1..=100_000).contains(&paths), "paths must be 1..100000");
    let journal = fs::canonicalize(journal)?;
    let start = chrono::NaiveDate::parse_from_str(&cfg.data.start_date, "%Y-%m-%d")?;
    let end = chrono::NaiveDate::parse_from_str(&cfg.data.end_date, "%Y-%m-%d")?;
    // Preflight the data and simulation budget before creating any new artifacts.
    let (audit, bars) =
        quant_data::capture::import_minutes(&journal, &cfg.data.symbols[0], start, end)?;
    ensure!(
        bars.len().saturating_mul(paths) <= 5_000_000,
        "Requested simulation exceeds the 5-million-step budget; reduce paths"
    );
    let id = format!("research_{}", uuid::Uuid::new_v4().simple());
    cfg.data.dataset_version = id.clone();
    cfg.training.model_id = id.clone();
    cfg.inference.model_version = id.clone();
    let directory = Path::new("reports/research").join(&id);
    fs::create_dir_all(&directory)?;
    let resolved = directory.join("config.yaml");
    fs::write(&resolved, serde_yaml::to_string(&cfg)?)?;
    let log = fs::File::create(directory.join("pipeline.log"))?;
    let report = format!("reports/backtest_{id}_test.json");
    let journal_arg = journal.to_string_lossy().into_owned();
    let paths_arg = paths.to_string();
    let steps: [(&str, Vec<&str>); 6] = [
        ("ingest", vec!["data", "ingest", "--journal", &journal_arg]),
        ("validate", vec!["data", "validate"]),
        ("features", vec!["features", "build"]),
        ("train", vec!["train"]),
        ("backtest", vec!["backtest", "run", "--model", &id]),
        (
            "simulate",
            vec!["simulate", "--report", &report, "--paths", &paths_arg],
        ),
    ];
    let mut completed = Vec::new();
    for (name, arguments) in steps {
        println!(
            "Research {id}: {name} (log: {})",
            directory.join("pipeline.log").display()
        );
        fs::write(
            directory.join("status.json"),
            serde_json::to_vec_pretty(
                &json!({"id":id,"status":"running","stage":name,"completed":completed,"capture":audit}),
            )?,
        )?;
        let mut child = Command::new(std::env::current_exe()?);
        for key in [
            "QUANTCTL_SYMBOLS",
            "QUANTCTL_DATASET_VERSION",
            "QUANTCTL_MODEL_ID",
            "QUANTCTL_EPOCHS",
            "QUANTCTL_BATCH_SIZE",
        ] {
            child.env_remove(key);
        }
        let result = child
            .arg("--config")
            .arg(&resolved)
            .args(arguments)
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log.try_clone()?))
            .status();
        match result {
            Ok(status) if status.success() => completed.push(name),
            _ => {
                fs::write(
                    directory.join("status.json"),
                    serde_json::to_vec_pretty(
                        &json!({"id":id,"status":"failed","stage":name,"completed":completed}),
                    )?,
                )?;
                anyhow::bail!("Research stage {name} failed; inspect {}. Partial versions are preserved; rerun creates a new ID.", directory.join("pipeline.log").display());
            }
        }
    }
    let runs: Vec<serde_json::Value> = fs::read_dir("reports/runs")?
        .filter_map(Result::ok)
        .filter_map(|e| fs::read(e.path().join("manifest.json")).ok())
        .filter_map(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .filter(|m| m["provenance"]["model_artifact_id"] == id)
        .collect();
    ensure!(
        runs.iter().any(|m| m["kind"] == "backtest")
            && runs.iter().any(|m| m["kind"] == "simulation"),
        "Missing newly published research runs"
    );
    fs::write(
        directory.join("status.json"),
        serde_json::to_vec_pretty(
            &json!({"id":id,"status":"complete","completed":completed,"capture":audit,"runs":runs}),
        )?,
    )
    .context("Persist research result")?;
    println!(
        "Research complete: {}",
        directory.join("status.json").display()
    );
    Ok(())
}
