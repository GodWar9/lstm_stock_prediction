//! Almgren-Chriss Nonlinear Market Impact Model.
//!
//! Implements the classic Almgren-Chriss (2000) execution framework separating market impact
//! into permanent and temporary components:
//!
//! Permanent Impact (persists in order book):
//!   ΔP_perm = γ * σ * (V_order / ADV)^α
//!
//! Temporary Impact (dissipates immediately, affects only current order):
//!   ΔP_temp = η * σ * (V_order / V_bar)^β
//!
//! Typically α = 1.0 and β = 0.5 (the empirical square-root law of price impact).

use crate::fees::VariableFeeSchedule;
use crate::fill::Fill;
use crate::model::ExecutionModel;
use crate::order::{Order, OrderType};
use quant_data::types::Bar;
use serde::{Deserialize, Serialize};

/// Almgren-Chriss nonlinear market impact execution model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlmgrenChrissExecutionModel {
    /// Permanent impact coefficient γ (gamma), typically ~0.314.
    pub gamma: f64,
    /// Temporary impact coefficient η (eta), typically ~0.142.
    pub eta: f64,
    /// Permanent impact exponent α (alpha), typically 1.0.
    pub alpha: f64,
    /// Temporary impact exponent β (beta), typically 0.5 (square-root law).
    pub beta: f64,
    /// Baseline volatility σ (sigma) used if bar has no volatility estimate (e.g. 0.02 = 2%).
    pub baseline_volatility: f64,
    /// Default Average Daily Volume (ADV) estimate if not dynamically supplied.
    pub default_adv: f64,
    /// Half-spread cost in basis points (e.g. 2.0 = 2 bps).
    pub half_spread_bps: f64,
    /// Default participation cap (e.g. 0.05 = 5% of bar volume).
    pub default_participation_cap: f64,
    /// Integrated variable fee schedule.
    pub fee_schedule: VariableFeeSchedule,
    /// Cumulative trading volume tracker for volume-tiered fee schedules.
    pub cumulative_volume: f64,
}

impl Default for AlmgrenChrissExecutionModel {
    fn default() -> Self {
        Self {
            gamma: 0.314,
            eta: 0.142,
            alpha: 1.0,
            beta: 0.5,
            baseline_volatility: 0.02,
            default_adv: 1_000_000.0,
            half_spread_bps: 2.0,
            default_participation_cap: 0.05,
            fee_schedule: VariableFeeSchedule::default(),
            cumulative_volume: 0.0,
        }
    }
}

impl AlmgrenChrissExecutionModel {
    /// Create a new Almgren-Chriss execution model with custom parameters.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        gamma: f64,
        eta: f64,
        alpha: f64,
        beta: f64,
        baseline_volatility: f64,
        default_adv: f64,
        half_spread_bps: f64,
        default_participation_cap: f64,
        fee_schedule: VariableFeeSchedule,
    ) -> Self {
        Self {
            gamma,
            eta,
            alpha,
            beta,
            baseline_volatility,
            default_adv,
            half_spread_bps,
            default_participation_cap,
            fee_schedule,
            cumulative_volume: 0.0,
        }
    }

    /// Calculate permanent impact shift: ΔP_perm = γ * σ * P * (V_order / ADV)^α
    pub fn permanent_impact(&self, order_qty: f64, price: f64, sigma: f64, adv: f64) -> f64 {
        let adv_eff = adv.max(1.0);
        let ratio = (order_qty.abs() / adv_eff).clamp(0.0, 1.0);
        self.gamma * sigma * price * ratio.powf(self.alpha)
    }

    /// Calculate temporary impact shift: ΔP_temp = η * σ * P * (V_order / V_bar)^β
    pub fn temporary_impact(&self, order_qty: f64, price: f64, sigma: f64, bar_volume: f64) -> f64 {
        let bar_vol_eff = bar_volume.max(1.0);
        let participation = (order_qty.abs() / bar_vol_eff).clamp(0.0, 1.0);
        self.eta * sigma * price * participation.powf(self.beta)
    }
}

impl ExecutionModel for AlmgrenChrissExecutionModel {
    fn simulate_fill(&self, order: &Order, bar: &Bar) -> Fill {
        if order.quantity.abs() < 1e-8 {
            return Fill {
                instrument: order.instrument,
                symbol: order.symbol.clone(),
                fill_price: bar.close,
                fill_quantity: 0.0,
                commission: 0.0,
                slippage: 0.0,
                as_of: order.as_of,
            };
        }

        // 1. Enforce volume participation cap
        let cap_ratio = order
            .participation_cap
            .unwrap_or(self.default_participation_cap)
            .clamp(0.001, 1.0);
        let bar_vol = bar.volume as f64;
        let max_shares = bar_vol * cap_ratio;
        let eligible_qty = order.quantity.clamp(-max_shares, max_shares);

        if eligible_qty.abs() < 1e-8 {
            return Fill {
                instrument: order.instrument,
                symbol: order.symbol.clone(),
                fill_price: bar.close,
                fill_quantity: 0.0,
                commission: 0.0,
                slippage: 0.0,
                as_of: order.as_of,
            };
        }

        // 2. Limit order check
        if let Some(limit_price) = order.limit_price {
            match order.order_type {
                OrderType::Limit if order.is_buy() && bar.low > limit_price => {
                    return Fill {
                        instrument: order.instrument,
                        symbol: order.symbol.clone(),
                        fill_price: limit_price,
                        fill_quantity: 0.0,
                        commission: 0.0,
                        slippage: 0.0,
                        as_of: order.as_of,
                    };
                }
                OrderType::Limit if order.is_sell() && bar.high < limit_price => {
                    return Fill {
                        instrument: order.instrument,
                        symbol: order.symbol.clone(),
                        fill_price: limit_price,
                        fill_quantity: 0.0,
                        commission: 0.0,
                        slippage: 0.0,
                        as_of: order.as_of,
                    };
                }
                _ => {}
            }
        }

        let base_price = bar.close;
        let half_spread_cost = base_price * (self.half_spread_bps * 1e-4);

        // Volatility estimation from High-Low range or baseline
        let bar_volatility = if bar.high > bar.low && bar.close > 0.0 {
            ((bar.high - bar.low) / bar.close).max(self.baseline_volatility)
        } else {
            self.baseline_volatility
        };

        // Almgren-Chriss Permanent and Temporary impact calculations
        let perm_impact =
            self.permanent_impact(eligible_qty, base_price, bar_volatility, self.default_adv);
        let temp_impact = self.temporary_impact(eligible_qty, base_price, bar_volatility, bar_vol);

        let total_impact_cost = perm_impact + temp_impact;
        let total_slippage_cost = half_spread_cost + total_impact_cost;

        let fill_price = if eligible_qty > 0.0 {
            // Buyer pays upward half-spread and impact
            base_price + total_slippage_cost
        } else {
            // Seller pays downward half-spread and impact
            base_price - total_slippage_cost
        };

        // Fee schedule application (passive limit = maker, market = taker)
        let is_maker = order.order_type == OrderType::Limit;
        let commission = self.fee_schedule.calculate_trade_fee(
            eligible_qty,
            fill_price,
            is_maker,
            self.cumulative_volume,
        );

        Fill {
            instrument: order.instrument,
            symbol: order.symbol.clone(),
            fill_price,
            fill_quantity: eligible_qty,
            commission,
            slippage: total_slippage_cost,
            as_of: order.as_of,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quant_data::types::{Bar, Timestamp};
    use quant_instruments::InstrumentId;

    fn make_test_bar(open: f64, close: f64, low: f64, high: f64, volume: u64) -> Bar {
        Bar {
            timestamp: Timestamp(1000),
            availability_timestamp: Timestamp(1000),
            open,
            high,
            low,
            close,
            volume,
            adjusted: false,
        }
    }

    #[test]
    fn test_almgren_chriss_square_root_impact() {
        let model = AlmgrenChrissExecutionModel::default();
        let bar = make_test_bar(100.0, 100.0, 99.0, 101.0, 100_000);

        // 1,000 shares vs 4,000 shares (4x volume -> 2x temporary impact under sqrt law)
        let order_1k = Order::market(InstrumentId(1), "AAPL", 1_000.0, 1000);
        let _order_4k = Order::market(InstrumentId(1), "AAPL", 4_000.0, 1000);

        let temp_1k = model.temporary_impact(1_000.0, 100.0, 0.02, 100_000.0);
        let temp_4k = model.temporary_impact(4_000.0, 100.0, 0.02, 100_000.0);

        // With beta = 0.5: (4000/100000)^0.5 / (1000/100000)^0.5 = sqrt(4) = 2.0
        let ratio = temp_4k / temp_1k;
        assert!(
            (ratio - 2.0).abs() < 1e-6,
            "Expected sqrt scaling 2.0, got {}",
            ratio
        );

        let fill = model.simulate_fill(&order_1k, &bar);
        assert_eq!(fill.fill_quantity, 1_000.0);
        assert!(fill.fill_price > 100.0);
        assert!(fill.slippage > 0.0);
    }

    #[test]
    fn test_almgren_chriss_limit_order_maker_rebate() {
        let model = AlmgrenChrissExecutionModel::default();
        let bar = make_test_bar(100.0, 100.0, 95.0, 105.0, 100_000);

        // Buy limit order at 98 (reached because low is 95)
        let order = Order::limit(InstrumentId(1), "AAPL", 1_000.0, 98.0, 1000);
        let fill = model.simulate_fill(&order, &bar);

        assert_eq!(fill.fill_quantity, 1_000.0);
        // Maker order earns rebate -> negative commission
        assert!(
            fill.commission < 0.0,
            "Expected maker rebate, got commission {}",
            fill.commission
        );
    }
}
