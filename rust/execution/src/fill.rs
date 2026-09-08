//! Executed trade fill structures.

use quant_instruments::InstrumentId;
use serde::{Deserialize, Serialize};

/// Executed trade fill record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fill {
    pub instrument: InstrumentId,
    pub symbol: String,
    /// Effective average fill execution price.
    pub fill_price: f64,
    /// Signed filled quantity (+ for BUY, - for SELL).
    pub fill_quantity: f64,
    /// Total commission/brokerage fees charged.
    pub commission: f64,
    /// Price impact and slippage cost incurred per share.
    pub slippage: f64,
    /// Execution timestamp.
    pub as_of: i64,
}

impl Fill {
    pub fn is_filled(&self) -> bool {
        self.fill_quantity.abs() > 1e-8
    }

    pub fn notional(&self) -> f64 {
        self.fill_quantity.abs() * self.fill_price
    }
}
