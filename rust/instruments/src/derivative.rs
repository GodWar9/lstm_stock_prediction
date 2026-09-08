//! Derivative instrument abstractions (Future and Option) for roadmap readiness.

use crate::{AssetClass, Currency, Instrument, InstrumentId};
use serde::{Deserialize, Serialize};

/// Option style / exercise type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OptionType {
    Call,
    Put,
}

/// Future contract representation (Phase 2 readiness).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Future {
    pub id: InstrumentId,
    pub symbol: String,
    pub underlying_symbol: String,
    pub currency: Currency,
    pub exchange: String,
    pub multiplier: f64,
    pub tick_size: f64,
    pub expiry_date: String,
}

impl Instrument for Future {
    fn id(&self) -> InstrumentId {
        self.id
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::Future
    }

    fn currency(&self) -> Currency {
        self.currency
    }

    fn exchange(&self) -> &str {
        &self.exchange
    }

    fn multiplier(&self) -> f64 {
        self.multiplier
    }

    fn tick_size(&self) -> f64 {
        self.tick_size
    }

    fn symbol(&self) -> &str {
        &self.symbol
    }
}

/// Standardized equity/index option representation (Phase 3 readiness).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Option_ {
    pub id: InstrumentId,
    pub symbol: String,
    pub underlying_id: InstrumentId,
    pub strike: f64,
    pub option_type: OptionType,
    pub expiry_date: String,
    pub currency: Currency,
    pub exchange: String,
    pub multiplier: f64,
    pub tick_size: f64,
}

impl Instrument for Option_ {
    fn id(&self) -> InstrumentId {
        self.id
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::Option
    }

    fn currency(&self) -> Currency {
        self.currency
    }

    fn exchange(&self) -> &str {
        &self.exchange
    }

    fn multiplier(&self) -> f64 {
        self.multiplier
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
    fn test_future_instrument_implementation() {
        let fut = Future {
            id: InstrumentId(101),
            symbol: "ESZ4".to_string(),
            underlying_symbol: "SPX".to_string(),
            currency: Currency::USD,
            exchange: "CME".to_string(),
            multiplier: 50.0,
            tick_size: 0.25,
            expiry_date: "2024-12-20".to_string(),
        };

        assert_eq!(fut.id(), InstrumentId(101));
        assert_eq!(fut.asset_class(), AssetClass::Future);
        assert_eq!(fut.multiplier(), 50.0);
    }

    #[test]
    fn test_option_instrument_implementation() {
        let opt = Option_ {
            id: InstrumentId(201),
            symbol: "AAPL241220C00200000".to_string(),
            underlying_id: InstrumentId(1),
            strike: 200.0,
            option_type: OptionType::Call,
            expiry_date: "2024-12-20".to_string(),
            currency: Currency::USD,
            exchange: "OPRA".to_string(),
            multiplier: 100.0,
            tick_size: 0.01,
        };

        assert_eq!(opt.id(), InstrumentId(201));
        assert_eq!(opt.asset_class(), AssetClass::Option);
        assert_eq!(opt.multiplier(), 100.0);
    }
}
