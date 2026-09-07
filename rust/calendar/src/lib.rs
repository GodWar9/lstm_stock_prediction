//! Trading calendar abstractions and market session schedules.
//!
//! Provides exchange-specific calendar definitions, session open/close boundaries,
//! holiday schedules, and point-in-time market hours validation.

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Calendar-specific errors.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CalendarError {
    #[error("Unknown exchange: {0}")]
    UnknownExchange(String),

    #[error("Market closed on date: {0}")]
    MarketClosed(NaiveDate),

    #[error("Timestamp outside market hours: {0}")]
    OutsideMarketHours(DateTime<Utc>),
}

/// Regular daily market hours specification (local time).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketHours {
    pub open: NaiveTime,
    pub close: NaiveTime,
}

impl MarketHours {
    pub fn new(open: NaiveTime, close: NaiveTime) -> Self {
        Self { open, close }
    }
}

/// A specific trading session instance with UTC timestamps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub date: NaiveDate,
    pub open_utc: DateTime<Utc>,
    pub close_utc: DateTime<Utc>,
    pub is_early_close: bool,
}

impl Session {
    pub fn new(date: NaiveDate, open_utc: DateTime<Utc>, close_utc: DateTime<Utc>, is_early_close: bool) -> Self {
        Self {
            date,
            open_utc,
            close_utc,
            is_early_close,
        }
    }

    /// Checks if a UTC timestamp falls within this session's operational hours [open, close].
    pub fn contains(&self, dt: DateTime<Utc>) -> bool {
        dt >= self.open_utc && dt <= self.close_utc
    }
}

/// Core trait representing exchange trading schedules and holiday rules.
pub trait TradingCalendar: Send + Sync {
    /// Listing exchange identifier (e.g., "NYSE", "NASDAQ").
    fn exchange(&self) -> &str;

    /// Checks if the given date is an active trading day (non-weekend, non-holiday).
    fn is_trading_day(&self, date: NaiveDate) -> bool;

    /// Returns the session details for a given date, or None if closed.
    fn session(&self, date: NaiveDate) -> Option<Session>;

    /// Returns the next valid trading day strictly after `date`.
    fn next_trading_day(&self, date: NaiveDate) -> NaiveDate;

    /// Returns the previous valid trading day strictly before `date`.
    fn prev_trading_day(&self, date: NaiveDate) -> NaiveDate;

    /// Verifies if a given UTC timestamp is within open trading hours.
    fn is_market_open(&self, dt: DateTime<Utc>) -> bool;
}
