//! Core market data types: Bar, Timestamp, and CorporateAction.

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};

/// Point-in-time timestamp represented as nanoseconds since Unix Epoch (UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Timestamp(pub i64);

impl Timestamp {
    pub const ZERO: Self = Self(0);

    pub fn from_unix_nanos(nanos: i64) -> Self {
        Self(nanos)
    }

    pub fn from_datetime(dt: DateTime<Utc>) -> Self {
        Self(dt.timestamp_nanos_opt().unwrap_or(0))
    }

    pub fn to_datetime(&self) -> DateTime<Utc> {
        Utc.timestamp_opt(self.0 / 1_000_000_000, (self.0 % 1_000_000_000) as u32)
            .single()
            .unwrap_or_else(|| Utc.timestamp_opt(0, 0).unwrap())
    }

    pub fn as_nanos(&self) -> i64 {
        self.0
    }
}

impl From<DateTime<Utc>> for Timestamp {
    fn from(dt: DateTime<Utc>) -> Self {
        Self::from_datetime(dt)
    }
}

impl From<Timestamp> for DateTime<Utc> {
    fn from(ts: Timestamp) -> Self {
        ts.to_datetime()
    }
}

/// A standard OHLCV price bar with explicit point-in-time observation and availability timestamps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bar {
    /// Timestamp when this bar was observed (e.g. bar close).
    pub timestamp: Timestamp,
    /// Availability timestamp: when this bar's data became known to the trading engine.
    /// Used for strict point-in-time leakage prevention.
    pub availability_timestamp: Timestamp,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
    /// Indicates whether OHLC values were adjusted for splits/dividends.
    pub adjusted: bool,
}

impl Bar {
    pub fn new(
        timestamp: Timestamp,
        availability_timestamp: Timestamp,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: u64,
        adjusted: bool,
    ) -> Self {
        Self {
            timestamp,
            availability_timestamp,
            open,
            high,
            low,
            close,
            volume,
            adjusted,
        }
    }

    /// Convenience constructor when availability is immediate at bar close.
    pub fn same_bar(timestamp: Timestamp, open: f64, high: f64, low: f64, close: f64, volume: u64) -> Self {
        Self::new(timestamp, timestamp, open, high, low, close, volume, false)
    }
}

/// Corporate action events (stock splits and cash/stock dividends).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CorporateActionKind {
    /// Forward or reverse split with ratio = (new_shares / old_shares).
    /// E.g., 4:1 split has ratio 4.0.
    Split { ratio: f64 },
    /// Cash dividend per share amount in denominated currency.
    Dividend { amount: f64 },
}

/// Point-in-time corporate action record with effective date.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorporateAction {
    pub symbol: String,
    pub effective_date: NaiveDate,
    pub action: CorporateActionKind,
}

impl CorporateAction {
    pub fn split(symbol: impl Into<String>, effective_date: NaiveDate, ratio: f64) -> Self {
        Self {
            symbol: symbol.into(),
            effective_date,
            action: CorporateActionKind::Split { ratio },
        }
    }

    pub fn dividend(symbol: impl Into<String>, effective_date: NaiveDate, amount: f64) -> Self {
        Self {
            symbol: symbol.into(),
            effective_date,
            action: CorporateActionKind::Dividend { amount },
        }
    }
}
