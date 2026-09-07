//! YAML config loading and environment variable override support.

use std::{env, fs, path::Path};
use thiserror::Error;
use crate::schema::AppConfig;
use crate::validation::ConfigValidationError;

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
        config.data.symbols = syms.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
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
