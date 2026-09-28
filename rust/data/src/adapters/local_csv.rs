//! Strict local OHLCV imports. No downloads or synthetic fallback.
use crate::{Bar, CorporateAction, DataError, MarketDataProvider, Timestamp};
use chrono::{DateTime, NaiveDate, Utc};
use quant_calendar::{TradingCalendar, UsEquityCalendar};
use std::path::PathBuf;

pub struct LocalCsvProvider {
    root: PathBuf,
}

impl LocalCsvProvider {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

/// The timestamp is the actual bar availability time, including an explicit UTC offset.
pub fn parse_local_csv(
    content: &str,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<Bar>, DataError> {
    if start >= end {
        return Err(DataError::ValidationError(
            "CSV start must precede end".into(),
        ));
    }
    let mut lines = content.trim_start_matches('\u{feff}').lines();
    if lines.next().map(str::trim) != Some("timestamp,open,high,low,close,volume") {
        return Err(DataError::ParseError(
            "CSV header must be timestamp,open,high,low,close,volume".into(),
        ));
    }
    let mut bars = Vec::new();
    // Validate the entire file before filtering so bad rows are never silently hidden.
    for (index, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<_> = line.split(',').map(str::trim).collect();
        let invalid = || DataError::ParseError(format!("Invalid OHLCV CSV row {}", index + 2));
        if fields.len() != 6 {
            return Err(invalid());
        }
        let timestamp = DateTime::parse_from_rfc3339(fields[0])
            .map_err(|_| {
                DataError::ParseError(format!(
                    "Row {} requires an RFC3339 timestamp with timezone",
                    index + 2
                ))
            })?
            .with_timezone(&Utc);
        // Timestamp uses nanoseconds: reject dates outside its representable range.
        let nanos = timestamp.timestamp_nanos_opt().ok_or_else(invalid)?;
        bars.push(Bar::same_bar(
            Timestamp(nanos),
            fields[1].parse().map_err(|_| invalid())?,
            fields[2].parse().map_err(|_| invalid())?,
            fields[3].parse().map_err(|_| invalid())?,
            fields[4].parse().map_err(|_| invalid())?,
            fields[5].parse().map_err(|_| invalid())?,
        ));
    }
    crate::validate_bars_monotonic_and_sound(&bars)?;
    bars.retain(|bar| {
        let date = bar.timestamp.to_datetime().date_naive();
        date >= start && date < end
    });
    if bars.is_empty() {
        return Err(DataError::ValidationError(
            "No CSV bars in the configured [start_date, end_date) UTC range".into(),
        ));
    }
    Ok(bars)
}

impl MarketDataProvider for LocalCsvProvider {
    fn fetch_ohlcv(
        &self,
        symbol: &str,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<Bar>, DataError> {
        crate::validate_storage_id(symbol)?;
        let path = self.root.join(format!("{symbol}.csv"));
        let content = std::fs::read_to_string(&path).map_err(|e| DataError::FetchError(format!(
            "Cannot read local CSV {}: {e}. Supply your OHLCV data; offline ingestion never downloads it.", path.display()
        )))?;
        parse_local_csv(&content, start, end)
    }

    fn corporate_actions(
        &self,
        _: &str,
        _: NaiveDate,
        _: NaiveDate,
    ) -> Result<Vec<CorporateAction>, DataError> {
        Err(DataError::FetchError(
            "CSV corporate actions are not implemented; supply consistently adjusted OHLCV data"
                .into(),
        ))
    }

    fn trading_calendar(&self, exchange: &str) -> Result<Box<dyn TradingCalendar>, DataError> {
        Ok(Box::new(UsEquityCalendar::new(exchange)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const HEADER: &str = "timestamp,open,high,low,close,volume\n";
    fn parse(rows: &str) -> Result<Vec<Bar>, DataError> {
        parse_local_csv(
            &format!("{HEADER}{rows}"),
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 3).unwrap(),
        )
    }
    #[test]
    fn imports_timezone_and_exclusive_end() {
        let bars = parse(
            "2024-01-01T16:00:00-05:00,10,12,9,11,100\n2024-01-03T21:00:00Z,11,12,9,10,200\n",
        )
        .unwrap();
        assert_eq!(bars.len(), 1);
        assert_eq!(
            bars[0].timestamp.to_datetime().to_rfc3339(),
            "2024-01-01T21:00:00+00:00"
        );
    }
    #[test]
    fn rejects_corrupt_rows_instead_of_dropping_or_coercing_them() {
        for row in [
            "2024-01-01,10,12,9,11,100",
            "2024-01-01T21:00:00Z,10,12,9,11,bad",
            "2024-01-01T21:00:00Z,10,12,9,11,-1",
            "2024-01-01T21:00:00Z,NaN,12,9,11,100",
            "2024-01-01T21:00:00Z,10,8,9,11,100",
            "2024-01-01T21:00:00Z,10,12",
            "",
            "2500-01-01T21:00:00Z,10,12,9,11,100",
        ] {
            assert!(parse(row).is_err(), "{row}");
        }
        let row = "2024-01-01T21:00:00Z,10,12,9,11,100\n";
        assert!(parse(&row.repeat(2)).is_err());
    }
}
