//! Regime-Switching Risk Guardrails.
//!
//! Enforces dynamic, regime-dependent risk constraints on portfolio leverage,
//! concentration limits, and position sizing across Bull, Bear, HighVol, and Crisis states.

use crate::regime::Regime;
use quant_portfolio::Portfolio;
use serde::{Deserialize, Serialize};

/// Risk limits and guardrail thresholds for a specific market regime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegimeRiskLimits {
    /// Maximum allowed gross leverage (e.g. 1.5 in Bull, 0.8 in Bear, 0.2 in Crisis).
    pub max_gross_leverage: f64,
    /// Maximum allowed net exposure as fraction of NAV (e.g. 1.0 in Bull, 0.3 in Bear, 0.0 in Crisis).
    pub max_net_exposure: f64,
    /// Maximum single-instrument portfolio weight (e.g. 0.25 in Bull, 0.10 in Bear, 0.05 in Crisis).
    pub max_single_position_weight: f64,
    /// Multiplier scaling target volatility or position sizing (e.g. 1.2 in Bull, 0.6 in Bear, 0.0 in Crisis).
    pub sizing_multiplier: f64,
    /// Maximum tolerable drawdown before full cash de-risking.
    pub max_drawdown_limit: f64,
}

/// Status and violations reported by guardrail check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegimeGuardrailStatus {
    pub active_regime: Regime,
    pub current_gross_leverage: f64,
    pub allowed_gross_leverage: f64,
    pub current_net_exposure: f64,
    pub allowed_net_exposure: f64,
    pub current_drawdown: f64,
    pub is_compliant: bool,
    pub breaches: Vec<String>,
}

/// Regime-switching risk guardrail orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegimeSwitchingGuardrails {
    pub bull_limits: RegimeRiskLimits,
    pub bear_limits: RegimeRiskLimits,
    pub crisis_limits: RegimeRiskLimits,
    pub high_vol_limits: RegimeRiskLimits,
    pub low_vol_limits: RegimeRiskLimits,
    pub mean_reverting_limits: RegimeRiskLimits,
}

impl Default for RegimeSwitchingGuardrails {
    fn default() -> Self {
        Self {
            bull_limits: RegimeRiskLimits {
                max_gross_leverage: 1.50,
                max_net_exposure: 1.00,
                max_single_position_weight: 0.30,
                sizing_multiplier: 1.00,
                max_drawdown_limit: 0.20,
            },
            bear_limits: RegimeRiskLimits {
                max_gross_leverage: 0.80,
                max_net_exposure: 0.30,
                max_single_position_weight: 0.15,
                sizing_multiplier: 0.60,
                max_drawdown_limit: 0.10,
            },
            crisis_limits: RegimeRiskLimits {
                max_gross_leverage: 0.10,
                max_net_exposure: 0.05,
                max_single_position_weight: 0.05,
                sizing_multiplier: 0.00, // Halts new positions completely
                max_drawdown_limit: 0.05,
            },
            high_vol_limits: RegimeRiskLimits {
                max_gross_leverage: 0.70,
                max_net_exposure: 0.40,
                max_single_position_weight: 0.15,
                sizing_multiplier: 0.50,
                max_drawdown_limit: 0.12,
            },
            low_vol_limits: RegimeRiskLimits {
                max_gross_leverage: 1.30,
                max_net_exposure: 0.90,
                max_single_position_weight: 0.25,
                sizing_multiplier: 1.10,
                max_drawdown_limit: 0.15,
            },
            mean_reverting_limits: RegimeRiskLimits {
                max_gross_leverage: 1.00,
                max_net_exposure: 0.50,
                max_single_position_weight: 0.20,
                sizing_multiplier: 0.80,
                max_drawdown_limit: 0.15,
            },
        }
    }
}

impl RegimeSwitchingGuardrails {
    /// Retrieve active limits for given market regime.
    pub fn limits_for(&self, regime: Regime) -> &RegimeRiskLimits {
        match regime {
            Regime::Bull => &self.bull_limits,
            Regime::Bear => &self.bear_limits,
            Regime::Crisis => &self.crisis_limits,
            Regime::HighVol => &self.high_vol_limits,
            Regime::LowVol => &self.low_vol_limits,
            Regime::MeanReverting | Regime::Trending => &self.mean_reverting_limits,
        }
    }

    /// Evaluate whether a portfolio complies with active regime guardrails.
    pub fn evaluate(&self, portfolio: &Portfolio, regime: Regime) -> RegimeGuardrailStatus {
        let limits = self.limits_for(regime);
        let gross_lev = portfolio.gross_exposure();
        let net_exp = portfolio.net_exposure();
        let drawdown = portfolio.current_drawdown();

        let mut breaches = Vec::new();

        if gross_lev > limits.max_gross_leverage + 1e-4 {
            breaches.push(format!(
                "Gross leverage {:.2} exceeds regime limit {:.2}",
                gross_lev, limits.max_gross_leverage
            ));
        }

        if net_exp.abs() > limits.max_net_exposure + 1e-4 {
            breaches.push(format!(
                "Net exposure {:.2} exceeds regime limit {:.2}",
                net_exp, limits.max_net_exposure
            ));
        }

        if drawdown > limits.max_drawdown_limit + 1e-4 {
            breaches.push(format!(
                "Current drawdown {:.2}% exceeds regime threshold {:.2}%",
                drawdown * 100.0,
                limits.max_drawdown_limit * 100.0
            ));
        }

        let is_compliant = breaches.is_empty();

        RegimeGuardrailStatus {
            active_regime: regime,
            current_gross_leverage: gross_lev,
            allowed_gross_leverage: limits.max_gross_leverage,
            current_net_exposure: net_exp,
            allowed_net_exposure: limits.max_net_exposure,
            current_drawdown: drawdown,
            is_compliant,
            breaches,
        }
    }

    /// Scale or clamp proposed order quantity to conform to regime risk guardrails.
    pub fn clamp_order_quantity(
        &self,
        desired_shares: f64,
        price: f64,
        nav: f64,
        regime: Regime,
    ) -> f64 {
        let limits = self.limits_for(regime);

        // Apply regime sizing multiplier (e.g. 0.0 in Crisis)
        let scaled_shares = desired_shares * limits.sizing_multiplier;
        if scaled_shares.abs() < 1e-6 {
            return 0.0;
        }

        // Single position maximum notional check
        let max_notional = nav * limits.max_single_position_weight;
        let max_shares = if price > 1e-6 {
            max_notional / price
        } else {
            0.0
        };

        scaled_shares.clamp(-max_shares, max_shares)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quant_portfolio::Portfolio;

    #[test]
    fn test_crisis_guardrail_prevents_new_orders() {
        let guardrails = RegimeSwitchingGuardrails::default();
        let nav = 100_000.0;
        let price = 50.0;

        // In crisis regime, sizing multiplier is 0.0
        let clamped = guardrails.clamp_order_quantity(100.0, price, nav, Regime::Crisis);
        assert_eq!(clamped, 0.0);
    }

    #[test]
    fn test_bull_and_bear_order_clamping() {
        let guardrails = RegimeSwitchingGuardrails::default();
        let nav = 100_000.0;
        let price = 100.0;

        // Desired: 500 shares ($50,000 notional = 50% of NAV)
        // Bull max weight is 30% -> clamped to $30,000 (300 shares)
        let bull_qty = guardrails.clamp_order_quantity(500.0, price, nav, Regime::Bull);
        assert_eq!(bull_qty, 300.0);

        // Bear multiplier is 0.60, max weight 15% -> 500 * 0.60 = 300, clamped to 150 shares
        let bear_qty = guardrails.clamp_order_quantity(500.0, price, nav, Regime::Bear);
        assert_eq!(bear_qty, 150.0);
    }

    #[test]
    fn test_evaluate_portfolio_compliance() {
        let guardrails = RegimeSwitchingGuardrails::default();
        let portfolio = Portfolio::new(100_000.0);

        let status_bull = guardrails.evaluate(&portfolio, Regime::Bull);
        assert!(status_bull.is_compliant);
        assert!(status_bull.breaches.is_empty());
    }
}
