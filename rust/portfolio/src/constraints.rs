//! Portfolio risk constraints, position limits, and exposure modes.

use serde::{Deserialize, Serialize};

/// Trading exposure strategy style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LongShortMode {
    /// Long positions only; negative signals result in flat allocations.
    LongOnly,
    /// Both long and short positions permitted.
    LongShort,
    /// Long and short gross exposures equalized (sum of weights = 0).
    DollarNeutral,
    /// Benchmark beta neutralized allocations.
    BetaNeutral,
}

impl Default for LongShortMode {
    fn default() -> Self {
        Self::LongOnly
    }
}

/// Portfolio construction constraints and de-risking thresholds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioConstraints {
    /// Maximum allowable gross exposure: sum(|weight_i|) <= max_gross_exposure (e.g. 1.0 = 100%).
    pub max_gross_exposure: f64,
    /// Maximum net exposure: |sum(weight_i)| <= max_net_exposure.
    pub max_net_exposure: f64,
    /// Maximum single-instrument portfolio weight limit (e.g. 0.20 = 20%).
    pub max_position_pct: f64,
    /// Maximum concentration in any single sector.
    pub max_sector_pct: f64,
    /// Maximum rebalance turnover rate per observation.
    pub max_turnover_pct: f64,
    /// Annualized portfolio volatility target (e.g. 0.15 = 15%).
    pub volatility_target: Option<f64>,
    /// Portfolio market exposure mode.
    pub long_short_mode: LongShortMode,
    /// Drawdown threshold that triggers risk de-risking (e.g. 0.10 = 10% drawdown).
    pub drawdown_derisk_threshold: f64,
    /// Exposure scalar applied when in drawdown de-risking state (e.g. 0.50 = 50% exposure reduction).
    pub drawdown_derisk_multiplier: f64,
}

impl Default for PortfolioConstraints {
    fn default() -> Self {
        Self {
            max_gross_exposure: 1.0,
            max_net_exposure: 0.5,
            max_position_pct: 0.20,
            max_sector_pct: 0.35,
            max_turnover_pct: 0.50,
            volatility_target: Some(0.15),
            long_short_mode: LongShortMode::LongOnly,
            drawdown_derisk_threshold: 0.10,
            drawdown_derisk_multiplier: 0.50,
        }
    }
}
