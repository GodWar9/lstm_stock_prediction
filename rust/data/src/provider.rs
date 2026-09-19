//! MarketDataProvider trait defining interface for historical and live feeds.

use crate::types::{Bar, CorporateAction};
use chrono::NaiveDate;
use quant_calendar::TradingCalendar;
use thiserror::Error;

/// Data engine errors across fetching, normalization, and validation.
#[derive(Debug, Error)]
pub enum DataError {
    #[error("Unknown or invalid symbol: {0}")]
    UnknownSymbol(String),

    #[error("Failed to fetch market data: {0}")]
    FetchError(String),

    #[error("Data parsing error: {0}")]
    ParseError(String),

    #[error("Data validation failure: {0}")]
    ValidationError(String),

    #[error("Point-in-time leakage violation: {0}")]
    LeakageViolation(String),

    #[error("Exchange calendar error: {0}")]
    CalendarError(String),
}

/// Abstract data provider interface.
///
/// Implementations include `YfinanceAdapter`, synthetic test generators,
/// and future commercial vendor feeds. Downstream consumers never reference
/// vendor adapters directly.
pub trait MarketDataProvider: Send + Sync {
    /// Ingests historical OHLCV bars for `symbol` between `start` and `end` inclusive.
    fn fetch_ohlcv(
        &self,
        symbol: &str,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<Bar>, DataError>;

    /// Ingests all point-in-time corporate actions (splits, dividends) between `start` and `end`.
    fn corporate_actions(
        &self,
        symbol: &str,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<CorporateAction>, DataError>;

    /// Returns the trading calendar associated with an exchange.
    fn trading_calendar(&self, exchange: &str) -> Result<Box<dyn TradingCalendar>, DataError>;
}
