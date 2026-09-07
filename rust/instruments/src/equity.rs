//! Equity instrument implementation.

use serde::{Deserialize, Serialize};
use crate::{AssetClass, Currency, Instrument, InstrumentId};

/// Represents a cash equity instrument (common or preferred stock).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Equity {
    pub id: InstrumentId,
    pub symbol: String,
    pub exchange: String,
    pub currency: Currency,
    pub tick_size: f64,
    pub lot_size: u32,
}

impl Equity {
    /// Constructs a custom equity instrument.
    pub fn new(
        id: InstrumentId,
        symbol: impl Into<String>,
        exchange: impl Into<String>,
        currency: Currency,
        tick_size: f64,
        lot_size: u32,
    ) -> Self {
        Self {
            id,
            symbol: symbol.into(),
            exchange: exchange.into(),
            currency,
            tick_size,
            lot_size,
        }
    }

    /// Factory method for standard US common equities (USD denominated, 1 cent tick, 1 lot).
    pub fn us_common_stock(id: InstrumentId, symbol: impl Into<String>, exchange: impl Into<String>) -> Self {
        Self::new(id, symbol, exchange, Currency::USD, 0.01, 1)
    }
}

impl Instrument for Equity {
    fn id(&self) -> InstrumentId {
        self.id
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::Equity
    }

    fn currency(&self) -> Currency {
        self.currency
    }

    fn exchange(&self) -> &str {
        &self.exchange
    }

    fn multiplier(&self) -> f64 {
        1.0
    }

    fn tick_size(&self) -> f64 {
        self.tick_size
    }

    fn symbol(&self) -> &str {
        &self.symbol
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equity_creation_and_instrument_trait() {
        let equity = Equity::us_common_stock(InstrumentId(1), "AAPL", "NASDAQ");
        assert_eq!(equity.id(), InstrumentId(1));
        assert_eq!(equity.symbol(), "AAPL");
        assert_eq!(equity.exchange(), "NASDAQ");
        assert_eq!(equity.currency(), Currency::USD);
        assert_eq!(equity.multiplier(), 1.0);
        assert_eq!(equity.tick_size(), 0.01);
        assert_eq!(equity.asset_class(), AssetClass::Equity);
    }

    #[test]
    fn test_equity_serde_roundtrip() {
        let equity = Equity::us_common_stock(InstrumentId(42), "MSFT", "NASDAQ");
        let serialized = serde_json::to_string(&equity).expect("serialization failed");
        let deserialized: Equity = serde_json::from_str(&serialized).expect("deserialization failed");
        assert_eq!(equity, deserialized);
    }
}
