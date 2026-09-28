//! Standalone risk evaluation engine for live portfolios and backtesting.

use crate::risk_report::{RiskReport, StressResult};
use quant_portfolio::Portfolio;
use std::collections::HashMap;

/// High-performance quantitative risk engine.
#[derive(Debug, Clone, Default)]
pub struct RiskEngine;

impl RiskEngine {
    pub fn new() -> Self {
        Self
    }

    /// Evaluate comprehensive risk metrics on a portfolio given return series.
    pub fn evaluate(
        &self,
        portfolio: &Portfolio,
        daily_returns: &[f64],
        benchmark_returns: &[f64],
    ) -> RiskReport {
        self.evaluate_with_periods(portfolio, daily_returns, benchmark_returns, 252.0)
    }
    pub fn evaluate_with_periods(
        &self,
        portfolio: &Portfolio,
        daily_returns: &[f64],
        benchmark_returns: &[f64],
        periods_per_year: f64,
    ) -> RiskReport {
        let nav = portfolio.nav();
        let gross_exp = portfolio.gross_exposure();
        let net_exp = portfolio.net_exposure();
        let max_dd = portfolio.current_drawdown();

        // 1. Return volatility
        let (mean_ret, daily_vol) = if daily_returns.len() >= 2 {
            let n = daily_returns.len() as f64;
            let mean = daily_returns.iter().sum::<f64>() / n;
            let var = daily_returns
                .iter()
                .map(|r| (r - mean).powi(2))
                .sum::<f64>()
                / (n - 1.0);
            (mean, var.sqrt())
        } else {
            (0.0, 0.0)
        };
        let annualized_vol = daily_vol * periods_per_year.sqrt();

        // 2. Parametric VaR and CVaR at 95% (Z_0.95 = 1.6449)
        let z_95 = 1.6448536269514722;
        let var_95_daily = (z_95 * daily_vol - mean_ret).max(0.0) * nav;

        // Historical CVaR calculation if returns available
        let cvar_95_daily = if daily_returns.len() >= 5 {
            let mut sorted = daily_returns.to_vec();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let cutoff_idx = ((sorted.len() as f64) * 0.05).ceil() as usize;
            let tail = &sorted[..cutoff_idx.max(1)];
            let mean_tail_loss = -(tail.iter().sum::<f64>() / tail.len() as f64);
            (mean_tail_loss * nav).max(var_95_daily)
        } else {
            var_95_daily * 1.25 // Conservative approximation when history is short
        };

        // 3. Market Beta: cov(Rp, Rb) / var(Rb)
        let beta = if daily_returns.len() >= 5 && daily_returns.len() == benchmark_returns.len() {
            let n = daily_returns.len() as f64;
            let mean_p = mean_ret;
            let mean_b = benchmark_returns.iter().sum::<f64>() / n;

            let mut cov = 0.0;
            let mut var_b = 0.0;
            for (p, b) in daily_returns.iter().zip(benchmark_returns) {
                cov += (p - mean_p) * (b - mean_b);
                var_b += (b - mean_b).powi(2);
            }
            if var_b > 1e-10 {
                cov / var_b
            } else {
                1.0
            }
        } else {
            1.0
        };

        // 4. Factor exposure
        let mut factor_exposure = HashMap::new();
        factor_exposure.insert("market".to_string(), beta);
        factor_exposure.insert("gross_leverage".to_string(), gross_exp);

        // 5. Stress testing scenarios
        let net_value = portfolio.net_market_value();
        let stress_scenarios = vec![
            StressResult {
                scenario_name: "MarketCrash_10Pct".to_string(),
                estimated_pnl: -0.10 * net_value,
                estimated_pnl_pct: if nav > 1e-8 {
                    (-0.10 * net_value) / nav
                } else {
                    0.0
                },
            },
            StressResult {
                scenario_name: "MarketRally_10Pct".to_string(),
                estimated_pnl: 0.10 * net_value,
                estimated_pnl_pct: if nav > 1e-8 {
                    (0.10 * net_value) / nav
                } else {
                    0.0
                },
            },
            StressResult {
                scenario_name: "SevereCrisis_20Pct".to_string(),
                estimated_pnl: -0.20 * net_value,
                estimated_pnl_pct: if nav > 1e-8 {
                    (-0.20 * net_value) / nav
                } else {
                    0.0
                },
            },
        ];

        RiskReport {
            var_95: var_95_daily,
            cvar_95: cvar_95_daily,
            volatility: annualized_vol,
            max_drawdown: max_dd,
            beta,
            gross_exposure: gross_exp,
            net_exposure: net_exp,
            turnover: 0.0,
            factor_exposure,
            stress_scenarios,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quant_instruments::InstrumentId;

    #[test]
    fn test_risk_engine_evaluation() {
        let engine = RiskEngine::new();
        let mut portfolio = Portfolio::new(100_000.0);
        portfolio.apply_trade(InstrumentId(1), "AAPL", 150.0, 200.0, 0.0); // $30k in AAPL

        let returns = vec![0.01, -0.015, 0.02, -0.005, 0.01, -0.02, 0.005];
        let bench = vec![0.008, -0.012, 0.018, -0.004, 0.009, -0.018, 0.004];

        let report = engine.evaluate(&portfolio, &returns, &bench);

        assert!(report.volatility > 0.0);
        assert!(report.var_95 > 0.0);
        assert!(report.cvar_95 >= report.var_95);
        assert!((report.beta - 1.0).abs() < 0.5);
        assert_eq!(report.stress_scenarios.len(), 3);
        assert_eq!(
            report.stress_scenarios[0].scenario_name,
            "MarketCrash_10Pct"
        );
    }
}
