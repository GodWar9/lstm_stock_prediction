//! Integration tests for quantctl binary and CLI subcommands.

use std::process::Command;

#[test]
fn test_cli_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_quantctl"))
        .arg("--help")
        .output()
        .expect("Failed to execute quantctl --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Quantitative Trading & Research Platform CLI"));
    assert!(stdout.contains("config"));
    assert!(stdout.contains("data"));
    assert!(stdout.contains("features"));
    assert!(stdout.contains("train"));
    assert!(stdout.contains("export-model"));
    assert!(stdout.contains("predict"));
    assert!(stdout.contains("backtest"));
    assert!(stdout.contains("simulate"));
    assert!(stdout.contains("report"));
    assert!(stdout.contains("benchmark"));
    assert!(stdout.contains("env"));
}

#[test]
fn test_cli_config_validate() {
    let output = Command::new(env!("CARGO_BIN_EXE_quantctl"))
        .args([
            "config",
            "validate",
            "--config",
            "../../configs/default.yaml",
        ])
        .output()
        .expect("Failed to execute quantctl config validate");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("SUCCESS: Configuration at '../../configs/default.yaml' is valid."));
    assert!(stdout.contains("Environment      : development"));
}

#[test]
fn test_cli_config_show() {
    let output = Command::new(env!("CARGO_BIN_EXE_quantctl"))
        .args(["config", "show", "--config", "../../configs/default.yaml"])
        .output()
        .expect("Failed to execute quantctl config show");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("symbols:"));
    assert!(stdout.contains("AAPL"));
}

#[test]
fn test_cli_env() {
    let output = Command::new(env!("CARGO_BIN_EXE_quantctl"))
        .args(["env", "--config", "../../configs/default.yaml"])
        .output()
        .expect("Failed to execute quantctl env");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("quantctl Environment & Build Metadata"));
    assert!(stdout.contains("CLI Version"));
    assert!(stdout.contains("Config Status     : VALID"));
}

#[test]
fn test_cli_invalid_config_path() {
    let output = Command::new(env!("CARGO_BIN_EXE_quantctl"))
        .args(["config", "validate", "--config", "non_existent_config.yaml"])
        .output()
        .expect("Failed to execute quantctl");

    assert!(!output.status.success());
}

#[test]
fn persisted_pipeline_training_prediction_backtest_simulation() {
    full_pipeline(false);
}

#[test]
fn local_csv_pipeline_training_prediction_backtest_simulation() {
    full_pipeline(true);
}

fn full_pipeline(local_csv: bool) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let unique = format!(
        "test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let mut config = std::fs::read_to_string(root.join("configs/demo.yaml"))
        .unwrap()
        .replace("inspector_demo", &unique)
        .replace("weight_decay: 0.0001", "weight_decay: 0.02")
        .replace(
            &format!("model_id: {unique}"),
            &format!("model_id: unused_{unique}"),
        );
    if local_csv {
        use quant_data::MarketDataProvider;
        let bars = quant_data::SyntheticDataProvider::default()
            .fetch_ohlcv(
                "AAPL",
                chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
                chrono::NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            )
            .unwrap();
        let import_dir = format!("datasets/import/{unique}");
        std::fs::create_dir_all(root.join(&import_dir)).unwrap();
        let mut csv = String::from("timestamp,open,high,low,close,volume\n");
        for bar in bars {
            csv.push_str(&format!(
                "{},{},{},{},{},{}\n",
                bar.timestamp.to_datetime().to_rfc3339(),
                bar.open,
                bar.high,
                bar.low,
                bar.close,
                bar.volume
            ));
        }
        std::fs::write(root.join(&import_dir).join("AAPL.csv"), csv).unwrap();
        config = config.replace(
            "provider: synthetic",
            &format!("provider: csv\n  input_dir: {import_dir}"),
        );
    }
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.yaml");
    std::fs::write(&path, config).unwrap();
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_quantctl"))
            .current_dir(&root)
            .env("QUANTCTL_MODEL_ID", &unique)
            .env("QUANTCTL_EPOCHS", "1")
            .arg("--config")
            .arg(&path)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{:?}\n{}\n{}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    };
    let missing = Command::new(env!("CARGO_BIN_EXE_quantctl"))
        .current_dir(&root)
        .arg("--config")
        .arg(&path)
        .args(["features", "build"])
        .output()
        .unwrap();
    assert!(
        !missing.status.success(),
        "No synthetic fallback for missing data"
    );
    run(&["data", "ingest"]);
    run(&["data", "validate"]);
    run(&["features", "build"]);
    run(&["train"]);
    let model_dir = root.join("models").join(&unique);
    let metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(model_dir.join("metadata.json")).unwrap()).unwrap();
    assert_eq!(metadata["hyperparameters"]["weight_decay"], 0.02);
    let log: serde_json::Value =
        serde_json::from_slice(&std::fs::read(model_dir.join("training_log.json")).unwrap())
            .unwrap();
    assert_eq!(
        log["train_loss"].as_array().unwrap().len(),
        1,
        "Environment epoch override reaches Python"
    );
    for name in [
        "model.onnx",
        "model.pt",
        "scaler.json",
        "metadata.json",
        "training_log.json",
        "validation.json",
        "integrity.json",
    ] {
        assert!(model_dir.join(name).is_file(), "Missing published {name}");
    }
    assert!(std::fs::read_dir(root.join("models/.staging"))
        .unwrap()
        .flatten()
        .all(|entry| !entry.file_name().to_string_lossy().starts_with(&unique)));
    let duplicate = Command::new(env!("CARGO_BIN_EXE_quantctl"))
        .current_dir(&root)
        .env("QUANTCTL_MODEL_ID", &unique)
        .arg("--config")
        .arg(&path)
        .arg("train")
        .output()
        .unwrap();
    assert!(!duplicate.status.success());
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("already exists"));
    run(&["predict", "--model", &unique, "--symbol", "AAPL"]);
    run(&["backtest", "run", "--model", &unique]);
    let reused = Command::new(env!("CARGO_BIN_EXE_quantctl"))
        .current_dir(&root)
        .arg("--config")
        .arg(&path)
        .args(["backtest", "run", "--model", &unique])
        .output()
        .unwrap();
    assert!(!reused.status.success());
    let report = format!("reports/backtest_{unique}_test.json");
    run(&["simulate", "--report", &report, "--paths", "50"]);
    run(&["report", "--backtest", &report]);
    let entries = std::fs::read_dir(root.join("reports/runs")).unwrap();
    let manifest = entries
        .flatten()
        .find_map(|entry| {
            let value: serde_json::Value =
                serde_json::from_slice(&std::fs::read(entry.path().join("manifest.json")).ok()?)
                    .ok()?;
            (value["provenance"]["model_artifact_id"] == unique).then_some(value)
        })
        .expect("published run manifest");
    assert_eq!(manifest["split"], "test");
    assert_eq!(
        manifest["provenance"]["source_snapshot"]["content_sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    assert_eq!(
        manifest["provenance"]["source"],
        if local_csv { "csv" } else { "synthetic" }
    );
    let report_json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join(&report)).unwrap()).unwrap();
    assert_eq!(
        report_json["returns"].as_array().unwrap().len(),
        report_json["benchmark_returns"].as_array().unwrap().len()
    );
    assert_eq!(report_json["benchmark_returns"][0], 0.0);
    let run_dir = root
        .join("reports/runs")
        .join(manifest["run_id"].as_str().unwrap());
    for capability in ["benchmark", "positions", "signal_outcomes", "risk"] {
        assert!(manifest["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c == capability));
        assert!(run_dir
            .join(manifest["artifacts"][capability].as_str().unwrap())
            .is_file());
    }
    let risk: serde_json::Value =
        serde_json::from_slice(&std::fs::read(run_dir.join("risk.json")).unwrap()).unwrap();
    assert_eq!(risk["max_drawdown"], report_json["max_drawdown"]);
    assert_eq!(risk["turnover"], report_json["turnover"]);
    // A byte-level change must fail before model loading, even if JSON remains valid.
    let scaler_path = model_dir.join("scaler.json");
    let mut scaler_bytes = std::fs::read(&scaler_path).unwrap();
    scaler_bytes.push(b' ');
    std::fs::write(&scaler_path, scaler_bytes).unwrap();
    let changed = Command::new(env!("CARGO_BIN_EXE_quantctl"))
        .current_dir(&root)
        .arg("--config")
        .arg(&path)
        .args(["predict", "--model", &unique, "--symbol", "AAPL"])
        .output()
        .unwrap();
    assert!(!changed.status.success());
    assert!(String::from_utf8_lossy(&changed.stderr).contains("integrity verification failed"));
}

#[test]
fn offline_csv_ingest_and_network_denial() {
    let temp = tempfile::tempdir().unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = std::fs::read_to_string(root.join("configs/demo.yaml")).unwrap();
    std::fs::create_dir_all(temp.path().join("datasets/import")).unwrap();
    std::fs::write(temp.path().join("datasets/import/AAPL.csv"), "timestamp,open,high,low,close,volume\n2020-01-02T21:00:00Z,10,12,9,11,100\n2020-01-03T21:00:00Z,11,12,10,11,200\n").unwrap();
    std::fs::write(
        temp.path().join("csv.yaml"),
        config.replace("provider: synthetic", "provider: csv"),
    )
    .unwrap();
    std::fs::write(
        temp.path().join("online.yaml"),
        config.replace("provider: synthetic", "provider: yfinance"),
    )
    .unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_quantctl"))
            .current_dir(temp.path())
            .env_remove("QUANTCTL_ALLOW_NETWORK")
            .args(args)
            .output()
            .unwrap()
    };
    let imported = run(&["--config", "csv.yaml", "data", "ingest"]);
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    assert!(run(&["--config", "csv.yaml", "data", "validate"])
        .status
        .success());
    assert!(
        !run(&["--config", "csv.yaml", "data", "ingest"])
            .status
            .success(),
        "Never overwrite a dataset version"
    );
    let denied = run(&["--config", "online.yaml", "data", "ingest"]);
    let short = run(&["--config", "csv.yaml", "features", "build"]);
    assert!(!short.status.success());
    assert!(String::from_utf8_lossy(&short.stderr).contains("Insufficient bars"));
    assert!(!denied.status.success());
    assert!(String::from_utf8_lossy(&denied.stderr).contains("Network acquisition is disabled"));
    assert!(!run(&[
        "--config",
        "csv.yaml",
        "data",
        "ingest",
        "--symbol",
        "../escape"
    ])
    .status
    .success());
}
