//! Validation rules for configuration schemas.

use thiserror::Error;
use crate::schema::AppConfig;

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
}

impl AppConfig {
    /// Validates all configuration constraints across subsystems.
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        // Data constraints
        if self.data.symbols.is_empty() {
            return Err(ConfigValidationError::Data("symbols list cannot be empty".into()));
        }
        for s in &self.data.symbols {
            if s.trim().is_empty() {
                return Err(ConfigValidationError::Data("symbol cannot be blank".into()));
            }
        }
        if self.data.start_date >= self.data.end_date {
            return Err(ConfigValidationError::Data(format!(
                "start_date ({}) must be strictly before end_date ({})",
                self.data.start_date, self.data.end_date
            )));
        }

        // Features constraints
        if self.features.lookback < 2 {
            return Err(ConfigValidationError::Features("lookback must be at least 2".into()));
        }
        if self.features.target_horizon == 0 {
            return Err(ConfigValidationError::Features("target_horizon must be at least 1".into()));
        }

        // Training constraints
        if self.training.batch_size == 0 {
            return Err(ConfigValidationError::Training("batch_size must be greater than 0".into()));
        }
        if self.training.epochs == 0 {
            return Err(ConfigValidationError::Training("epochs must be greater than 0".into()));
        }
        if self.training.learning_rate <= 0.0 {
            return Err(ConfigValidationError::Training("learning_rate must be positive".into()));
        }
        if self.training.dropout < 0.0 || self.training.dropout >= 1.0 {
            return Err(ConfigValidationError::Training("dropout must be in [0.0, 1.0)".into()));
        }

        // Portfolio constraints
        if self.portfolio.max_gross_exposure <= 0.0 {
            return Err(ConfigValidationError::Portfolio("max_gross_exposure must be positive".into()));
        }
        if self.portfolio.max_net_exposure < 0.0 || self.portfolio.max_net_exposure > self.portfolio.max_gross_exposure {
            return Err(ConfigValidationError::Portfolio(
                "max_net_exposure must be non-negative and <= max_gross_exposure".into(),
            ));
        }
        if self.portfolio.max_position_pct <= 0.0 || self.portfolio.max_position_pct > 1.0 {
            return Err(ConfigValidationError::Portfolio("max_position_pct must be in (0.0, 1.0]".into()));
        }
        if let Some(vt) = self.portfolio.volatility_target {
            if vt <= 0.0 {
                return Err(ConfigValidationError::Portfolio("volatility_target must be positive if specified".into()));
            }
        }

        // Execution constraints
        if self.execution.fixed_commission < 0.0 {
            return Err(ConfigValidationError::Execution("fixed_commission cannot be negative".into()));
        }
        if self.execution.half_spread_bps < 0.0 {
            return Err(ConfigValidationError::Execution("half_spread_bps cannot be negative".into()));
        }
        if self.execution.participation_cap <= 0.0 || self.execution.participation_cap > 1.0 {
            return Err(ConfigValidationError::Execution("participation_cap must be in (0.0, 1.0]".into()));
        }

        Ok(())
    }
}
