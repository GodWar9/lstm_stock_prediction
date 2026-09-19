//! Portfolio state tracking, mark-to-market valuation, and trade application.

use crate::position::Position;
use quant_instruments::InstrumentId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Core portfolio state representing cash, open holdings, and performance history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portfolio {
    pub cash: f64,
    pub positions: HashMap<InstrumentId, Position>,
    pub initial_cash: f64,
    pub peak_nav: f64,
    pub realized_pnl: f64,
    pub total_commissions: f64,
}

impl Portfolio {
    /// Initialize a new portfolio with a specified starting cash balance.
    pub fn new(initial_cash: f64) -> Self {
        Self {
            cash: initial_cash,
            positions: HashMap::new(),
            initial_cash,
            peak_nav: initial_cash,
            realized_pnl: 0.0,
            total_commissions: 0.0,
        }
    }

    /// Total portfolio Net Asset Value (NAV): Cash + sum(market value of all positions).
    pub fn nav(&self) -> f64 {
        let positions_value: f64 = self.positions.values().map(|p| p.market_value()).sum();
        self.cash + positions_value
    }

    /// Total portfolio gross market value: sum(|market value|).
    pub fn gross_market_value(&self) -> f64 {
        self.positions
            .values()
            .map(|p| p.market_value().abs())
            .sum()
    }

    /// Total portfolio net market value: sum(market value).
    pub fn net_market_value(&self) -> f64 {
        self.positions.values().map(|p| p.market_value()).sum()
    }

    /// Gross exposure ratio: gross_market_value / NAV.
    pub fn gross_exposure(&self) -> f64 {
        let nav = self.nav();
        if nav > 1e-8 {
            self.gross_market_value() / nav
        } else {
            0.0
        }
    }

    /// Net exposure ratio: net_market_value / NAV.
    pub fn net_exposure(&self) -> f64 {
        let nav = self.nav();
        if nav > 1e-8 {
            self.net_market_value() / nav
        } else {
            0.0
        }
    }

    /// Current peak-to-trough drawdown ratio: (peak_nav - current_nav) / peak_nav.
    pub fn current_drawdown(&self) -> f64 {
        let nav = self.nav();
        if self.peak_nav > 1e-8 {
            ((self.peak_nav - nav) / self.peak_nav).max(0.0)
        } else {
            0.0
        }
    }

    /// Update mark-to-market prices for active positions and recalculate peak NAV.
    pub fn update_market_prices(&mut self, prices: &HashMap<InstrumentId, f64>) {
        for (id, pos) in self.positions.iter_mut() {
            if let Some(&price) = prices.get(id) {
                pos.current_price = price;
            }
        }
        let nav = self.nav();
        if nav > self.peak_nav {
            self.peak_nav = nav;
        }
    }

    /// Apply an executed trade fill to the portfolio.
    ///
    /// - `fill_qty > 0`: buying shares (increases position, deducts cash)
    /// - `fill_qty < 0`: selling shares (decreases position, adds cash, records realized PnL)
    pub fn apply_trade(
        &mut self,
        instrument: InstrumentId,
        symbol: &str,
        fill_price: f64,
        fill_qty: f64,
        commission: f64,
    ) {
        if fill_qty.abs() < 1e-8 {
            return;
        }

        let trade_cost = fill_qty * fill_price;
        self.cash -= trade_cost + commission;
        self.total_commissions += commission;

        let pos = self
            .positions
            .entry(instrument)
            .or_insert_with(|| Position::new(instrument, symbol, 0.0, fill_price));

        pos.current_price = fill_price;

        let prev_qty = pos.quantity;
        let new_qty = prev_qty + fill_qty;

        if prev_qty.signum() == fill_qty.signum() || prev_qty.abs() < 1e-8 {
            // Increasing existing position in same direction
            if new_qty.abs() > 1e-8 {
                pos.avg_entry_price =
                    (prev_qty * pos.avg_entry_price + fill_qty * fill_price) / new_qty;
            }
            pos.quantity = new_qty;
        } else {
            // Closing or reducing position (opposite direction) -> realize PnL
            let closed_qty = prev_qty.abs().min(fill_qty.abs()) * prev_qty.signum();
            let pnl = closed_qty * (fill_price - pos.avg_entry_price);
            self.realized_pnl += pnl;

            pos.quantity = new_qty;
            if prev_qty.signum() != new_qty.signum() && new_qty.abs() > 1e-8 {
                // Position reversed direction
                pos.avg_entry_price = fill_price;
            }
        }

        // Update peak NAV
        let nav = self.nav();
        if nav > self.peak_nav {
            self.peak_nav = nav;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_portfolio_initial_state() {
        let p = Portfolio::new(100_000.0);
        assert_eq!(p.nav(), 100_000.0);
        assert_eq!(p.cash, 100_000.0);
        assert_eq!(p.gross_exposure(), 0.0);
        assert_eq!(p.current_drawdown(), 0.0);
    }

    #[test]
    fn test_portfolio_buy_and_sell_cycle() {
        let mut p = Portfolio::new(100_000.0);
        let id = InstrumentId(1);

        // Buy 100 shares @ $150 with $1 commission
        p.apply_trade(id, "AAPL", 150.0, 100.0, 1.0);
        assert_eq!(p.cash, 100_000.0 - 15_000.0 - 1.0);
        assert_eq!(p.positions.get(&id).unwrap().quantity, 100.0);
        assert_eq!(p.total_commissions, 1.0);

        // Price rises to $160
        let mut prices = HashMap::new();
        prices.insert(id, 160.0);
        p.update_market_prices(&prices);
        assert_eq!(p.nav(), 84_999.0 + 16_000.0); // 100,999.0
        assert_eq!(p.peak_nav, 100_999.0);

        // Sell 100 shares @ $160 with $1 commission
        p.apply_trade(id, "AAPL", 160.0, -100.0, 1.0);
        assert_eq!(p.positions.get(&id).unwrap().quantity, 0.0);
        assert_eq!(p.realized_pnl, 1_000.0); // (160 - 150) * 100
        assert_eq!(p.cash, 100_998.0);
    }
}
