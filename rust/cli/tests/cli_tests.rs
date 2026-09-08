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
        .args(["config", "validate", "--config", "../../configs/default.yaml"])
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
fn test_cli_features_build() {
    let output = Command::new(env!("CARGO_BIN_EXE_quantctl"))
        .args(["features", "build", "--config", "../../configs/default.yaml", "--feature-set", "test_v1"])
        .output()
        .expect("Failed to execute quantctl features build");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Successfully built feature set 'test_v1'"));
}

#[test]
fn test_cli_train() {
    let output = Command::new(env!("CARGO_BIN_EXE_quantctl"))
        .args(["train", "--config", "../../configs/default.yaml"])
        .output()
        .expect("Failed to execute quantctl train");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        eprintln!("test_cli_train FAILED:\nSTDOUT:\n{}\nSTDERR:\n{}", stdout, stderr);
    }
    assert!(output.status.success(), "Status not success");
    assert!(stdout.contains("Training completed successfully"));
}

#[test]
fn test_cli_predict() {
    let output = Command::new(env!("CARGO_BIN_EXE_quantctl"))
        .args(["predict", "--model", "lstm_v1", "--symbol", "AAPL", "--config", "../../configs/default.yaml"])
        .output()
        .expect("Failed to execute quantctl predict");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        eprintln!("test_cli_predict FAILED:\nSTDOUT:\n{}\nSTDERR:\n{}", stdout, stderr);
    }
    assert!(output.status.success());
    assert!(stdout.contains("QUANTCTL MODEL PREDICTION & SIGNAL"));
    assert!(stdout.contains("Symbol:             AAPL"));
    assert!(stdout.contains("Model:              lstm_v1"));
    assert!(stdout.contains("Signal Direction:"));
}

#[test]
fn test_cli_backtest_and_simulate_and_report() {
    // 1. Backtest Run
    let backtest_output = Command::new(env!("CARGO_BIN_EXE_quantctl"))
        .args([
            "backtest",
            "run",
            "--model",
            "lstm_v1",
            "--split",
            "test",
            "--allow-reuse",
            "--config",
            "../../configs/default.yaml",
        ])
        .output()
        .expect("Failed to execute quantctl backtest run");

    assert!(backtest_output.status.success());
    let bt_stdout = String::from_utf8_lossy(&backtest_output.stdout);
    assert!(bt_stdout.contains("QUANTCTL BACKTEST REPORT"));
    assert!(bt_stdout.contains("Sharpe Ratio:"));

    // Find generated report file (either relative to rust/cli or workspace)
    let report_candidates = [
        std::path::PathBuf::from("reports/backtest_lstm_v1_test.json"),
        std::path::PathBuf::from("../../reports/backtest_lstm_v1_test.json"),
    ];
    let report_path = report_candidates.iter().find(|p| p.exists()).cloned()
        .unwrap_or_else(|| report_candidates[0].clone());

    // 2. Simulate Run
    let sim_output = Command::new(env!("CARGO_BIN_EXE_quantctl"))
        .args([
            "simulate",
            "--report",
            report_path.to_str().unwrap(),
            "--paths",
            "100",
            "--config",
            "../../configs/default.yaml",
        ])
        .output()
        .expect("Failed to execute quantctl simulate");

    assert!(sim_output.status.success());
    let sim_stdout = String::from_utf8_lossy(&sim_output.stdout);
    assert!(sim_stdout.contains("QUANTCTL MONTE CARLO SIMULATION"));
    assert!(sim_stdout.contains("Simulated Paths:    100"));

    // 3. Report Run
    let rep_output = Command::new(env!("CARGO_BIN_EXE_quantctl"))
        .args([
            "report",
            "--backtest",
            report_path.to_str().unwrap(),
            "--config",
            "../../configs/default.yaml",
        ])
        .output()
        .expect("Failed to execute quantctl report");

    assert!(rep_output.status.success());
    let rep_stdout = String::from_utf8_lossy(&rep_output.stdout);
    assert!(rep_stdout.contains("QUANTITATIVE STRATEGY AUDIT REPORT"));
    assert!(rep_stdout.contains("Cumulative Return:"));
}
