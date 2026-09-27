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
    let config = std::fs::read_to_string(root.join("configs/demo.yaml"))
        .unwrap()
        .replace("inspector_demo", &unique)
        .replace("epochs: 2", "epochs: 1");
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.yaml");
    std::fs::write(&path, config).unwrap();
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_quantctl"))
            .current_dir(&root)
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
    assert_eq!(manifest["provenance"]["source"], "synthetic");
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
