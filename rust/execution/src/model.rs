//! Execution models for realistic order fill simulation.

use crate::fill::Fill;
use crate::order::{Order, OrderType};
use quant_data::types::Bar;

/// Trait defining the execution simulation interface.
pub trait ExecutionModel: Send + Sync {
    /// Simulate order fill against market bar liquidity, spread, and slippage.
    fn simulate_fill(&self, order: &Order, bar: &Bar) -> Fill;
}

/// Composite execution model implementing fixed commission, half-spread cost,
/// linear volume-participation slippage, and volume participation caps.
#[derive(Debug, Clone)]
pub struct CompositeExecutionModel {
    /// Commission rate as a fraction of trade notional (e.g. 0.0005 = 5 bps).
    pub commission_rate: f64,
    /// Minimum commission fee per trade (e.g. $1.00).
    pub min_commission: f64,
    /// Half-spread cost in basis points (e.g. 2.0 = 2 bps).
    pub half_spread_bps: f64,
    /// Slippage coefficient scaling with (order_shares / bar_volume).
    pub slippage_factor: f64,
    /// Default maximum allowed participation of bar volume (e.g. 0.05 = 5%).
    pub default_participation_cap: f64,
}

impl Default for CompositeExecutionModel {
    fn default() -> Self {
        Self {
            commission_rate: 0.0005, // 5 bps
            min_commission: 1.0,     // $1.00
            half_spread_bps: 2.0,    // 2 bps
            slippage_factor: 0.10,   // linear slippage
            default_participation_cap: 0.05, // 5% volume limit
        }
    }
}

impl CompositeExecutionModel {
    pub fn new(
        commission_rate: f64,
        min_commission: f64,
        half_spread_bps: f64,
        slippage_factor: f64,
        default_participation_cap: f64,
    ) -> Self {
        Self {
            commission_rate,
            min_commission,
            half_spread_bps,
            slippage_factor,
            default_participation_cap,
        }
    }
}

impl ExecutionModel for CompositeExecutionModel {
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
        let cap_ratio = order.participation_cap.unwrap_or(self.default_participation_cap).clamp(0.001, 1.0);
        let max_shares = (bar.volume as f64) * cap_ratio;
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

        // 2. Limit order condition check
        if let Some(limit_price) = order.limit_price {
            match order.order_type {
                OrderType::Limit if order.is_buy() => {
                    if bar.low > limit_price {
                        // Market low didn't reach limit price
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
                }
                OrderType::Limit if order.is_sell() => {
                    if bar.high < limit_price {
                        // Market high didn't reach limit price
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
                }
                _ => {}
            }
        }

        // 3. Execution price calculation (Base price is bar open/close)
        let base_price = bar.close;
        let half_spread_cost = base_price * (self.half_spread_bps * 1e-4);

        // Linear market impact slippage: factor * (shares / volume)
        let participation_rate = eligible_qty.abs() / (bar.volume as f64).max(1.0);
        let slippage_cost = base_price * (self.slippage_factor * participation_rate);

        let (fill_price, total_slippage_cost) = if eligible_qty > 0.0 {
            // Buyer pays half-spread and upward slippage
            (base_price + half_spread_cost + slippage_cost, half_spread_cost + slippage_cost)
        } else {
            // Seller pays half-spread and downward slippage
            (base_price - half_spread_cost - slippage_cost, half_spread_cost + slippage_cost)
        };

        // 4. Commission calculation
        let notional = eligible_qty.abs() * fill_price;
        let commission = (notional * self.commission_rate).max(self.min_commission);

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
    fn test_market_order_fill_with_slippage_and_spread() {
        let model = CompositeExecutionModel {
            commission_rate: 0.001,
            min_commission: 1.0,
            half_spread_bps: 5.0, // 5 bps
            slippage_factor: 0.1,
            default_participation_cap: 0.10,
        };

        let bar = make_test_bar(100.0, 100.0, 99.0, 101.0, 10_000);
        let order = Order::market(InstrumentId(1), "AAPL", 100.0, 1000);

        let fill = model.simulate_fill(&order, &bar);
        assert_eq!(fill.fill_quantity, 100.0);
        // Buyer pays above $100
        assert!(fill.fill_price > 100.0);
        assert!(fill.commission >= 1.0);
        assert!(fill.slippage > 0.0);
    }

    #[test]
    fn test_participation_cap_enforcement() {
        let model = CompositeExecutionModel {
            default_participation_cap: 0.05, // 5% cap of 1,000 volume = 50 shares
            ..Default::default()
        };

        let bar = make_test_bar(50.0, 50.0, 49.0, 51.0, 1_000);
        let order = Order::market(InstrumentId(1), "MSFT", 500.0, 1000); // 500 requested

        let fill = model.simulate_fill(&order, &bar);
        assert_eq!(fill.fill_quantity, 50.0); // Clamped to 50 shares
    }

    #[test]
    fn test_limit_order_unfilled_when_price_not_reached() {
        let model = CompositeExecutionModel::default();
        let bar = make_test_bar(100.0, 100.0, 95.0, 105.0, 10_000);

        // Buy limit at $90 (bar low was $95, so not reached)
        let order = Order::limit(InstrumentId(1), "AAPL", 100.0, 90.0, 1000);
        let fill = model.simulate_fill(&order, &bar);
        assert_eq!(fill.fill_quantity, 0.0);
        assert!(!fill.is_filled());
    }
}
