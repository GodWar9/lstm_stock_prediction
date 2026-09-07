//! Core instrument abstractions for the quant platform.
//!
//! Provides the fundamental traits and types representing tradable assets.
//! Phase 1 implements `Equity`, while maintaining an extensible trait boundary
//! for `Future` and `Option` derivatives in future phases.

use std::fmt;
use serde::{Deserialize, Serialize};

pub mod equity;
pub use equity::Equity;

/// Unique identifier for an instrument within the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct InstrumentId(pub u32);

impl fmt::Display for InstrumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u32> for InstrumentId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

/// Asset classes supported across all platform lifecycle phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AssetClass {
    Equity,
    Etf,
    Future,
    Option,
    Index,
    Fx,
    Crypto,
}

impl fmt::Display for AssetClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Equity => write!(f, "Equity"),
            Self::Etf => write!(f, "Etf"),
            Self::Future => write!(f, "Future"),
            Self::Option => write!(f, "Option"),
            Self::Index => write!(f, "Index"),
            Self::Fx => write!(f, "Fx"),
            Self::Crypto => write!(f, "Crypto"),
        }
    }
}

/// Currency representations for financial instruments and cash accounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Currency {
    USD,
    EUR,
    GBP,
    JPY,
    CAD,
    AUD,
    CHF,
    INR,
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::USD => write!(f, "USD"),
            Self::EUR => write!(f, "EUR"),
            Self::GBP => write!(f, "GBP"),
            Self::JPY => write!(f, "JPY"),
            Self::CAD => write!(f, "CAD"),
            Self::AUD => write!(f, "AUD"),
            Self::CHF => write!(f, "CHF"),
            Self::INR => write!(f, "INR"),
        }
    }
}

/// Core trait representing any tradable financial instrument.
///
/// Designed to decouple portfolio, risk, execution, and backtesting layers
/// from asset-specific details such as lot sizes and contract multipliers.
pub trait Instrument: Send + Sync + fmt::Debug {
    /// Unique instrument identifier.
    fn id(&self) -> InstrumentId;

    /// Primary asset class.
    fn asset_class(&self) -> AssetClass;

    /// Denominated base currency.
    fn currency(&self) -> Currency;

    /// Primary listing exchange code (e.g. "NASDAQ", "NYSE").
    fn exchange(&self) -> &str;

    /// Contract multiplier (1.0 for equities, typically 100.0 for standard options).
    fn multiplier(&self) -> f64;

    /// Minimum price variation.
    fn tick_size(&self) -> f64;

    /// Human-readable ticker symbol or contract identifier.
    fn symbol(&self) -> &str;
}
