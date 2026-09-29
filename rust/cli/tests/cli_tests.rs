//! Integration tests for quantctl binary and CLI subcommands.

use std::process::Command;

#[test]
fn captured_minutes_train_fresh_model_backtest_and_simulation() {
    use chrono::{Duration, TimeZone, Utc};
    use serde_json::json;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let capture = temp.path().join("capture");
    std::fs::create_dir(&capture).unwrap();
    let start = Utc.with_ymd_and_hms(2024, 1, 2, 14, 30, 0).unwrap();
    let mut records = vec![
        json!({"type":"session","feed":"iex","symbols":["AAPL"],"synthetic":true,"received_at_ms":start.timestamp_millis()}),
    ];
    // Three full regular sessions, explicitly marked synthetic protocol data.
    let mut close: f64 = 100.0;
    for day in 0..3 {
        for minute in 0..390 {
            let t = start + Duration::days(day) + Duration::minutes(minute);
            let open = close;
            close *= 1.0 + if minute % 3 == 0 { 0.0003 } else { -0.0001 };
            records.push(json!({"type":"market","feed":"iex","received_at_ms":(t+Duration::minutes(1)).timestamp_millis()+500,
                "event":{"T":"b","S":"AAPL","t":t.to_rfc3339(),"o":open,"h":open.max(close)+0.02,"l":open.min(close)-0.02,"c":close,"v":50000}}));
        }
    }
    let last = records.last().unwrap()["received_at_ms"].as_i64().unwrap();
    records.push(json!({"type":"stop","reason":"shutdown","received_at_ms":last+1}));
    let mut chain = String::new();
    let mut journal = String::new();
    for (i, mut record) in records.into_iter().enumerate() {
        record["schema_version"] = json!(1);
        record["seq"] = json!(i + 1);
        record["session_id"] = json!("synthetic_protocol_fixture");
        record["previous_checksum"] = json!(chain);
        chain = quant_data::capture::checksum(&record);
        record["checksum"] = json!(chain);
        journal.push_str(&record.to_string());
        journal.push('\n');
    }
    std::fs::write(capture.join("000001.ndjson"), journal).unwrap();
    // Scale the shipped intraday config down to this three-session fixture. The
    // production lookback and embargo need roughly a month of minute history; the
    // first walk-forward fold would otherwise leave a validation split below
    // lookback + 1 rows. Cadence, provenance and mismatched-bar-interval checks
    // are unaffected.
    // A repeated test must train a distinct package: production intentionally
    // prevents evaluating the exact same ONNX bytes on a test split twice.
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        % u32::MAX as u128;
    let cfg = std::fs::read_to_string(root.join("configs/intraday.yaml"))
        .unwrap()
        .replace("2026-09-01", "2024-01-01")
        .replace("2026-10-01", "2024-01-05")
        .replace("epochs: 10", "epochs: 1")
        .replace("hidden_size: 32", "hidden_size: 8")
        .replace("lookback: 30", "lookback: 5")
        .replace("random_seed: 42", &format!("random_seed: {seed}"))
        .replace("embargo_gap: 30", "embargo_gap: 3");
    let config = temp.path().join("config.yaml");
    std::fs::write(&config, cfg).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_quantctl"))
        .current_dir(&root)
        .arg("--config")
        .arg(&config)
        .args(["research", "--journal"])
        .arg(&capture)
        .args(["--paths", "20"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{}\n{}",
        stdout,
        String::from_utf8_lossy(&output.stderr)
    );
    let result = stdout
        .lines()
        .find_map(|line| line.strip_prefix("Research complete: "))
        .unwrap();
    let result: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join(result)).unwrap()).unwrap();
    let id = result["id"].as_str().unwrap();
    assert_eq!(result["status"], "complete");
    assert_eq!(result["capture"]["synthetic"], true);
    let model = root.join("models").join(id);
    let metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(model.join("metadata.json")).unwrap()).unwrap();
    assert_eq!(metadata["bar_interval"], "1m");
    assert_eq!(metadata["periods_per_year"], 98280);
    // Exercise the same inference service used by the browser with a genuinely
    // trained and sealed ONNX package, not a mocked prediction response.
    let query = quant_api::forecast::ForecastQuery {
        model: id.into(),
        dataset: id.into(),
        symbol: "AAPL".into(),
    };
    let forecast = quant_api::forecast::run(&root, &query).unwrap();
    assert_eq!(forecast.interval, "1m");
    assert_eq!(forecast.horizon_bars, 1);
    assert_eq!(forecast.as_of_ms, (last - 500));
    assert_eq!(forecast.available_at_ms, last);
    assert!(forecast.implied_close.is_finite() && forecast.implied_close > 0.0);
    assert!(
        (forecast.implied_close / forecast.last_close - 1.0 - forecast.predicted_return).abs()
            < 1e-12
    );
    assert_eq!(forecast.dataset_sha256.len(), 64);
    assert!(forecast.warnings.iter().any(|w| w.contains("Synthetic")));
    assert!(forecast.warnings.iter().any(|w| w.contains("Historical")));
    let dataset_manifest = root
        .join("datasets/market")
        .join(id)
        .join("AAPL.manifest.json");
    let original = std::fs::read(&dataset_manifest).unwrap();
    let mut mismatched: serde_json::Value = serde_json::from_slice(&original).unwrap();
    mismatched["bar_interval"] = json!("1d");
    std::fs::write(&dataset_manifest, serde_json::to_vec(&mismatched).unwrap()).unwrap();
    assert!(quant_api::forecast::run(&root, &query)
        .unwrap_err()
        .to_string()
        .contains("intervals differ"));
    std::fs::write(&dataset_manifest, &original).unwrap();
    let validation: serde_json::Value =
        serde_json::from_slice(&std::fs::read(model.join("validation.json")).unwrap()).unwrap();
    assert_eq!(validation["folds"].as_array().unwrap().len(), 3);
    for run in result["runs"].as_array().unwrap() {
        assert_eq!(run["metrics"]["periods_per_year"], 98280.0);
        assert!(run["provenance"]["source"]
            .as_str()
            .unwrap()
            .starts_with("synthetic_capture:"));
        assert_eq!(run["provenance"]["data_version"], id);
    }
    // A daily configuration cannot silently run this minute-trained model.
    let mismatch = Command::new(env!("CARGO_BIN_EXE_quantctl"))
        .current_dir(&root)
        .args([
            "--config",
            "configs/demo.yaml",
            "predict",
            "--model",
            id,
            "--symbol",
            "AAPL",
        ])
        .output()
        .unwrap();
    assert!(!mismatch.status.success());
    assert!(String::from_utf8_lossy(&mismatch.stderr).contains("bar interval differs"));
}

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
            // The simulation inherits this provenance, so select the backtest run.
            (value["kind"] == "backtest" && value["provenance"]["model_artifact_id"] == unique)
                .then_some(value)
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
