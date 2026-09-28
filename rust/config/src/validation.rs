//! Validation rules for configuration schemas.

use crate::schema::AppConfig;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigValidationError {
    #[error("Data validation error: {0}")]
    Data(String),

    #[error("Feature validation error: {0}")]
    Features(String),

    #[error("Training validation error: {0}")]
    Training(String),

    #[error("Portfolio validation error: {0}")]
    Portfolio(String),

    #[error("Execution validation error: {0}")]
    Execution(String),

    #[error("Backtest validation error: {0}")]
    Backtest(String),
}

/// A portable single filename component, allowing ticker dots (e.g. BRK.B).
pub fn valid_storage_id(id: &str) -> bool {
    let stem = id.split('.').next().unwrap_or("").to_ascii_uppercase();
    !id.is_empty()
        && id.len() <= 128
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && !id.ends_with('.')
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
        && !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        && !(stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
}

impl AppConfig {
    /// Validates all configuration constraints across subsystems.
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        // Data constraints
        if self.data.symbols.is_empty() {
            return Err(ConfigValidationError::Data(
                "symbols list cannot be empty".into(),
            ));
        }
        for s in &self.data.symbols {
            if !valid_storage_id(s) {
                return Err(ConfigValidationError::Data(
                    "symbol must be a portable filename identifier".into(),
                ));
            }
        }
        if !valid_storage_id(&self.data.dataset_version) {
            return Err(ConfigValidationError::Data(
                "dataset_version must be a portable filename identifier".into(),
            ));
        }
        if !matches!(
            self.data.provider.as_str(),
            "csv" | "synthetic" | "yfinance" | "alpaca_journal"
        ) {
            return Err(ConfigValidationError::Data(
                "provider must be csv, synthetic, yfinance or alpaca_journal".into(),
            ));
        }
        if self
            .data
            .symbols
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != self.data.symbols.len()
        {
            return Err(ConfigValidationError::Data("symbols must be unique".into()));
        }
        let parse_date = |value: &str| {
            chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .map_err(|_| ConfigValidationError::Data(format!("Invalid date: {value}")))
        };
        if parse_date(&self.data.start_date)? >= parse_date(&self.data.end_date)? {
            return Err(ConfigValidationError::Data(format!(
                "start_date ({}) must be strictly before end_date ({})",
                self.data.start_date, self.data.end_date
            )));
        }

        if !matches!(self.data.bar_interval.as_str(), "1d" | "1m")
            || (self.data.bar_interval == "1m") != (self.data.provider == "alpaca_journal")
        {
            return Err(ConfigValidationError::Data(
                "One-minute research requires alpaca_journal; other providers require 1d".into(),
            ));
        }

        // Features constraints
        if self.features.lookback < 2 {
            return Err(ConfigValidationError::Features(
                "lookback must be at least 2".into(),
            ));
        }
        if self.features.target_horizon == 0 {
            return Err(ConfigValidationError::Features(
                "target_horizon must be at least 1".into(),
            ));
        }

        // Training constraints
        if !valid_storage_id(&self.training.model_id)
            || !valid_storage_id(&self.inference.model_version)
        {
            return Err(ConfigValidationError::Training(
                "model identifiers must be portable filename identifiers".into(),
            ));
        }
        if self.training.hidden_size == 0 || self.training.num_layers == 0 {
            return Err(ConfigValidationError::Training(
                "hidden_size and num_layers must be positive".into(),
            ));
        }
        if !self.training.weight_decay.is_finite() || self.training.weight_decay < 0.0 {
            return Err(ConfigValidationError::Training(
                "weight_decay must be finite and non-negative".into(),
            ));
        }
        if self.training.batch_size == 0 {
            return Err(ConfigValidationError::Training(
                "batch_size must be greater than 0".into(),
            ));
        }
        if self.training.epochs == 0 {
            return Err(ConfigValidationError::Training(
                "epochs must be greater than 0".into(),
            ));
        }
        if !self.training.learning_rate.is_finite() || self.training.learning_rate <= 0.0 {
            return Err(ConfigValidationError::Training(
                "learning_rate must be positive".into(),
            ));
        }
        if !self.training.dropout.is_finite()
            || self.training.dropout < 0.0
            || self.training.dropout >= 1.0
        {
            return Err(ConfigValidationError::Training(
                "dropout must be in [0.0, 1.0)".into(),
            ));
        }

        // Portfolio constraints
        if !self.portfolio.max_gross_exposure.is_finite()
            || self.portfolio.max_gross_exposure <= 0.0
        {
            return Err(ConfigValidationError::Portfolio(
                "max_gross_exposure must be positive".into(),
            ));
        }
        if !self.portfolio.max_net_exposure.is_finite()
            || self.portfolio.max_net_exposure < 0.0
            || self.portfolio.max_net_exposure > self.portfolio.max_gross_exposure
        {
            return Err(ConfigValidationError::Portfolio(
                "max_net_exposure must be non-negative and <= max_gross_exposure".into(),
            ));
        }
        if !self.portfolio.max_position_pct.is_finite()
            || self.portfolio.max_position_pct <= 0.0
            || self.portfolio.max_position_pct > 1.0
        {
            return Err(ConfigValidationError::Portfolio(
                "max_position_pct must be in (0.0, 1.0]".into(),
            ));
        }
        if let Some(vt) = self.portfolio.volatility_target {
            if !vt.is_finite() || vt <= 0.0 {
                return Err(ConfigValidationError::Portfolio(
                    "volatility_target must be positive if specified".into(),
                ));
            }
        }

        // Execution constraints
        if !matches!(
            self.portfolio.long_short_mode.as_str(),
            "LongOnly" | "LongShort" | "DollarNeutral"
        ) {
            return Err(ConfigValidationError::Portfolio(
                "Unknown long_short_mode".into(),
            ));
        }
        if !self.execution.slippage_factor.is_finite() || self.execution.slippage_factor < 0.0 {
            return Err(ConfigValidationError::Execution(
                "slippage_factor must be finite and non-negative".into(),
            ));
        }
        if !self.execution.fixed_commission.is_finite() || self.execution.fixed_commission < 0.0 {
            return Err(ConfigValidationError::Execution(
                "fixed_commission cannot be negative".into(),
            ));
        }
        if !self.execution.half_spread_bps.is_finite() || self.execution.half_spread_bps < 0.0 {
            return Err(ConfigValidationError::Execution(
                "half_spread_bps cannot be negative".into(),
            ));
        }
        if !self.execution.participation_cap.is_finite()
            || self.execution.participation_cap <= 0.0
            || self.execution.participation_cap > 1.0
        {
            return Err(ConfigValidationError::Execution(
                "participation_cap must be in (0.0, 1.0]".into(),
            ));
        }

        if !self.backtest.initial_cash.is_finite()
            || self.backtest.initial_cash <= 0.0
            || !self.backtest.risk_free_rate.is_finite()
            || self.backtest.risk_free_rate <= -1.0
        {
            return Err(ConfigValidationError::Backtest("initial_cash must be finite and positive; risk_free_rate must be finite and greater than -1".into()));
        }
        Ok(())
    }
}
