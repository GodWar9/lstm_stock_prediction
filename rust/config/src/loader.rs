//! YAML config loading and environment variable override support.

use crate::schema::AppConfig;
use crate::validation::ConfigValidationError;
use std::{env, fs, path::Path};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read configuration file: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML deserialization error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Configuration validation failed: {0}")]
    Validation(#[from] ConfigValidationError),
}

/// Loads an `AppConfig` from a YAML file, applies environment overrides, and validates.
pub fn load_config(path: impl AsRef<Path>) -> Result<AppConfig, ConfigError> {
    let content = fs::read_to_string(path)?;
    load_config_from_str(&content)
}

/// Parses an `AppConfig` from a YAML string, applies environment overrides, and validates.
pub fn load_config_from_str(content: &str) -> Result<AppConfig, ConfigError> {
    let mut config: AppConfig = serde_yaml::from_str(content)?;
    apply_env_overrides(&mut config);
    config.validate()?;
    Ok(config)
}

/// Overrides configuration values using QUANTCTL_* environment variables.
pub fn apply_env_overrides(config: &mut AppConfig) {
    if let Ok(env_val) = env::var("QUANTCTL_ENV") {
        config.env = env_val;
    }
    if let Ok(syms) = env::var("QUANTCTL_SYMBOLS") {
        config.data.symbols = syms
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    if let Ok(dataset_ver) = env::var("QUANTCTL_DATASET_VERSION") {
        config.data.dataset_version = dataset_ver;
    }
    if let Ok(model_id) = env::var("QUANTCTL_MODEL_ID") {
        config.training.model_id = model_id.clone();
        config.inference.model_version = model_id;
    }
    if let Ok(epochs) = env::var("QUANTCTL_EPOCHS") {
        if let Ok(e) = epochs.parse::<usize>() {
            config.training.epochs = e;
        }
    }
    if let Ok(batch_size) = env::var("QUANTCTL_BATCH_SIZE") {
        if let Ok(b) = batch_size.parse::<usize>() {
            config.training.batch_size = b;
            config.inference.batch_size = b;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    const VALID_YAML: &str = r#"
env: production
data:
  symbols: ["AAPL", "MSFT"]
  start_date: "2020-01-01"
  end_date: "2023-01-01"
  provider: "yfinance"
  exchange: "NASDAQ"
  dataset_version: "ds_test_v1"
features:
  feature_set: "baseline_v1"
  feature_set_version: 1
  lookback: 30
  target_horizon: 1
  target_transformation: "log_return"
training:
  model_id: "lstm_test_v1"
  hidden_size: 64
  num_layers: 2
  dropout: 0.2
  learning_rate: 0.001
  weight_decay: 0.0001
  batch_size: 32
  epochs: 10
  random_seed: 123
  purge_gap: 5
  embargo_gap: 30
inference:
  model_version: "lstm_test_v1"
  intra_op_threads: 2
  inter_op_threads: 1
  batch_size: 32
portfolio:
  max_gross_exposure: 1.0
  max_net_exposure: 0.5
  max_position_pct: 0.25
  volatility_target: 0.12
  long_short_mode: "LongShort"
execution:
  fixed_commission: 0.0005
  half_spread_bps: 1.5
  slippage_factor: 0.05
  participation_cap: 0.02
backtest:
  initial_cash: 500000.0
  risk_free_rate: 0.045
"#;

    #[test]
    fn test_load_valid_yaml() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let cfg = load_config_from_str(VALID_YAML).expect("valid yaml should parse");
        assert_eq!(cfg.env, "production");
        assert_eq!(cfg.data.symbols, vec!["AAPL", "MSFT"]);
        assert_eq!(cfg.features.lookback, 30);
        assert_eq!(cfg.training.hidden_size, 64);
        assert_eq!(cfg.portfolio.volatility_target, Some(0.12));
        assert_eq!(cfg.backtest.initial_cash, 500_000.0);
    }

    #[test]
    fn test_invalid_date_ordering() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let invalid_yaml =
            VALID_YAML.replace("start_date: \"2020-01-01\"", "start_date: \"2025-01-01\"");
        let res = load_config_from_str(&invalid_yaml);
        assert!(matches!(
            res,
            Err(ConfigError::Validation(ConfigValidationError::Data(_)))
        ));
    }

    #[test]
    fn test_empty_symbols_rejected() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let invalid_yaml = VALID_YAML.replace("symbols: [\"AAPL\", \"MSFT\"]", "symbols: []");
        let res = load_config_from_str(&invalid_yaml);
        assert!(matches!(
            res,
            Err(ConfigError::Validation(ConfigValidationError::Data(_)))
        ));
    }

    #[test]
    fn test_invalid_lookback() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let invalid_yaml = VALID_YAML.replace("lookback: 30", "lookback: 1");
        let res = load_config_from_str(&invalid_yaml);
        assert!(matches!(
            res,
            Err(ConfigError::Validation(ConfigValidationError::Features(_)))
        ));
    }

    #[test]
    fn test_invalid_exposure() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let invalid_yaml = VALID_YAML.replace("max_net_exposure: 0.5", "max_net_exposure: 1.5");
        let res = load_config_from_str(&invalid_yaml);
        assert!(matches!(
            res,
            Err(ConfigError::Validation(ConfigValidationError::Portfolio(_)))
        ));
    }

    #[test]
    fn test_invalid_participation_cap() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let invalid_yaml = VALID_YAML.replace("participation_cap: 0.02", "participation_cap: 1.5");
        let res = load_config_from_str(&invalid_yaml);
        assert!(matches!(
            res,
            Err(ConfigError::Validation(ConfigValidationError::Execution(_)))
        ));
    }

    #[test]
    fn test_env_overrides() {
        let _guard = TEST_MUTEX.lock().unwrap();
        env::set_var("QUANTCTL_ENV", "staging");
        env::set_var("QUANTCTL_SYMBOLS", "NVDA,TSLA");
        env::set_var("QUANTCTL_EPOCHS", "100");

        let cfg = load_config_from_str(VALID_YAML).expect("should parse");
        assert_eq!(cfg.env, "staging");
        assert_eq!(cfg.data.symbols, vec!["NVDA", "TSLA"]);
        assert_eq!(cfg.training.epochs, 100);

        // Clean up env vars
        env::remove_var("QUANTCTL_ENV");
        env::remove_var("QUANTCTL_SYMBOLS");
        env::remove_var("QUANTCTL_EPOCHS");
    }

    #[test]
    fn test_default_config_file_loads() {
        let _guard = TEST_MUTEX.lock().unwrap();
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../configs/default.yaml");
        let cfg =
            load_config(path).expect("repository default config must load and validate cleanly");
        assert_eq!(cfg.data.symbols, vec!["AAPL"]);
        assert_eq!(cfg.features.lookback, 60);
    }

    #[test]
    fn rejects_invalid_numbers_dates_and_paths() {
        let _guard = TEST_MUTEX.lock().unwrap();
        for (from, to) in [
            ("learning_rate: 0.001", "learning_rate: .nan"),
            ("dropout: 0.2", "dropout: .nan"),
            ("max_gross_exposure: 1.0", "max_gross_exposure: .inf"),
            ("initial_cash: 500000.0", "initial_cash: 0"),
            ("risk_free_rate: 0.045", "risk_free_rate: .nan"),
            ("slippage_factor: 0.05", "slippage_factor: -1"),
            ("hidden_size: 64", "hidden_size: 0"),
            ("2020-01-01", "2020-02-31"),
            ("ds_test_v1", "../escape"),
            ("[\"AAPL\", \"MSFT\"]", "[\"AAPL\", \"AAPL\"]"),
            ("[\"AAPL\", \"MSFT\"]", "[\"../escape\"]"),
        ] {
            assert!(
                load_config_from_str(&VALID_YAML.replace(from, to)).is_err(),
                "{to}"
            );
        }
    }
}
