//! Multi-dimensional risk report and stress test structures.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Impact of a specific market stress event on portfolio valuation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StressResult {
    pub scenario_name: String,
    pub estimated_pnl: f64,
    pub estimated_pnl_pct: f64,
}

/// Comprehensive point-in-time portfolio risk report.
///
/// Designed to satisfy both live portfolio construction and deterministic backtesting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskReport {
    /// Parametric/historical Value at Risk at 95% confidence (1-day).
    pub var_95: f64,
    /// Conditional Value at Risk / Expected Shortfall at 95% confidence (1-day).
    pub cvar_95: f64,
    /// Annualized realized return volatility.
    pub volatility: f64,
    /// Maximum historical peak-to-trough drawdown experienced so far.
    pub max_drawdown: f64,
    /// Portfolio market beta relative to benchmark.
    pub beta: f64,
    /// Current gross exposure ratio (gross market value / NAV).
    pub gross_exposure: f64,
    /// Current net exposure ratio (net market value / NAV).
    pub net_exposure: f64,
    /// Annualized / period turnover rate.
    pub turnover: f64,
    /// Systematic factor exposures (e.g. market, momentum, size, value).
    pub factor_exposure: HashMap<String, f64>,
    /// Scenario stress test results.
    pub stress_scenarios: Vec<StressResult>,
}

impl Default for RiskReport {
    fn default() -> Self {
        Self {
            var_95: 0.0,
            cvar_95: 0.0,
            volatility: 0.0,
            max_drawdown: 0.0,
            beta: 1.0,
            gross_exposure: 0.0,
            net_exposure: 0.0,
            turnover: 0.0,
            factor_exposure: HashMap::new(),
            stress_scenarios: Vec::new(),
        }
    }
}
