//! Position and TargetPosition structures.

use quant_instruments::InstrumentId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An existing active portfolio position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub instrument: InstrumentId,
    pub symbol: String,
    /// Number of shares/units held (positive for Long, negative for Short, 0 for Flat).
    pub quantity: f64,
    /// Volume-weighted average entry purchase price.
    pub avg_entry_price: f64,
    /// Most recent mark-to-market bar price.
    pub current_price: f64,
}

impl Position {
    pub fn new(
        instrument: InstrumentId,
        symbol: impl Into<String>,
        quantity: f64,
        price: f64,
    ) -> Self {
        Self {
            instrument,
            symbol: symbol.into(),
            quantity,
            avg_entry_price: price,
            current_price: price,
        }
    }

    /// Mark-to-market position dollar value.
    pub fn market_value(&self) -> f64 {
        self.quantity * self.current_price
    }

    /// Unrealized mark-to-market profit or loss.
    pub fn unrealized_pnl(&self) -> f64 {
        self.quantity * (self.current_price - self.avg_entry_price)
    }

    pub fn is_long(&self) -> bool {
        self.quantity > 1e-8
    }

    pub fn is_short(&self) -> bool {
        self.quantity < -1e-8
    }

    pub fn is_flat(&self) -> bool {
        self.quantity.abs() <= 1e-8
    }
}

/// Target desired allocation for an individual instrument.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetPosition {
    pub instrument: InstrumentId,
    pub symbol: String,
    /// Target portfolio weight in [-1.0, 1.0].
    pub target_weight: f64,
    /// Target allocated dollar value.
    pub target_value: f64,
    /// Target number of shares/contracts.
    pub target_quantity: f64,
}

/// Collection of target positions output by the PortfolioConstructor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetPositions {
    pub targets: HashMap<InstrumentId, TargetPosition>,
    pub gross_weight: f64,
    pub net_weight: f64,
    pub as_of: i64,
}

impl TargetPositions {
    pub fn new(as_of: i64) -> Self {
        Self {
            targets: HashMap::new(),
            gross_weight: 0.0,
            net_weight: 0.0,
            as_of,
        }
    }

    pub fn add(&mut self, target: TargetPosition) {
        self.gross_weight += target.target_weight.abs();
        self.net_weight += target.target_weight;
        self.targets.insert(target.instrument, target);
    }

    pub fn get(&self, id: &InstrumentId) -> Option<&TargetPosition> {
        self.targets.get(id)
    }
}
