//! Bounded Twelve Data OHLCV snapshots. Credentials never enter errors or URLs.
use crate::{Bar, MarketIdentity, Timestamp};
use anyhow::{bail, ensure, Context, Result};
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde::Deserialize;
use std::io::Read;

const MAX_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone)]
pub struct Snapshot {
    pub bars: Vec<Bar>,
    pub fetched_at_ms: i64,
}

#[derive(Deserialize)]
struct Envelope {
    status: Option<String>,
    code: Option<u16>,
    meta: Option<Meta>,
    values: Option<Vec<Value>>,
}
#[derive(Deserialize)]
struct Meta {
    symbol: String,
    exchange: String,
    currency: String,
    exchange_timezone: String,
    interval: String,
    #[serde(rename = "type")]
    instrument_type: String,
}
#[derive(Deserialize)]
struct Value {
    datetime: String,
    open: String,
    high: String,
    low: String,
    close: String,
    volume: Option<String>,
}

pub fn key_configured() -> bool {
    std::env::var("TWELVE_DATA_API_KEY").is_ok_and(|key| !key.trim().is_empty())
}

/// End date is exclusive. History is capped at 5000 bars per immutable import.
pub fn fetch(
    market: &MarketIdentity,
    interval: &str,
    dates: Option<(NaiveDate, NaiveDate)>,
    count: usize,
) -> Result<Snapshot> {
    market.validate().map_err(anyhow::Error::msg)?;
    let provider_interval = match interval {
        "1m" => "1min",
        "1d" => "1day",
        _ => bail!("Unsupported global bar interval"),
    };
    ensure!(
        (2..=5000).contains(&count),
        "Global snapshot requires 2..5000 bars"
    );
    let key = std::env::var("TWELVE_DATA_API_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty())
        .context("Set TWELVE_DATA_API_KEY on the backend to enable fresh global market data")?;
    let mut authorization =
        reqwest::header::HeaderValue::from_str(&format!("apikey {}", key.trim()))
            .map_err(|_| anyhow::anyhow!("TWELVE_DATA_API_KEY contains invalid characters"))?;
    authorization.set_sensitive(true);
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .connect_timeout(std::time::Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| anyhow::anyhow!("Cannot initialize market data HTTPS client"))?;
    let mut request = client
        .get("https://api.twelvedata.com/time_series")
        .header(reqwest::header::AUTHORIZATION, authorization)
        .query(&[
            ("symbol", market.provider_symbol.clone()),
            ("exchange", market.exchange.clone()),
            ("interval", provider_interval.into()),
            ("timezone", "UTC".into()),
            ("order", "asc".into()),
            ("adjust", "none".into()),
            ("outputsize", count.to_string()),
        ]);
    if let Some((start, end)) = dates {
        ensure!(start < end, "Historical start must precede end");
        request = request.query(&[
            ("start_date", start.to_string()),
            ("end_date", end.to_string()),
        ]);
    }
    // Do not propagate reqwest errors or provider messages: they can echo credentials.
    let response = request
        .send()
        .map_err(|_| anyhow::anyhow!("Market data connection failed or timed out; retry later"))?;
    ensure!(
        response.status().is_success(),
        "Market data HTTP {}; check credentials, exchange access and quota",
        response.status().as_u16()
    );
    ensure!(
        response.content_length().unwrap_or(0) <= MAX_BYTES,
        "Market data response exceeds size limit"
    );
    let mut bytes = Vec::new();
    response
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| anyhow::anyhow!("Cannot read market data response"))?;
    ensure!(
        bytes.len() as u64 <= MAX_BYTES,
        "Market data response exceeds size limit"
    );
    parse(&bytes, market, interval, dates, count, Utc::now())
}

pub fn parse(
    bytes: &[u8],
    market: &MarketIdentity,
    interval: &str,
    dates: Option<(NaiveDate, NaiveDate)>,
    count: usize,
    now: DateTime<Utc>,
) -> Result<Snapshot> {
    market.validate().map_err(anyhow::Error::msg)?;
    ensure!(
        bytes.len() as u64 <= MAX_BYTES,
        "Market response exceeds size limit"
    );
    let envelope: Envelope = serde_json::from_slice(bytes)
        .map_err(|_| anyhow::anyhow!("Malformed market data response"))?;
    if envelope.status.as_deref() == Some("error") {
        bail!("Market data provider rejected request (code {}); check key, exchange entitlement and quota", envelope.code.unwrap_or(0));
    }
    let meta = envelope
        .meta
        .context("Market response has no identity metadata")?;
    let expected_interval = match interval {
        "1m" => "1min",
        "1d" => "1day",
        _ => bail!("Unsupported global bar interval"),
    };
    ensure!(meta.symbol == market.provider_symbol && meta.exchange == market.exchange
        && meta.currency == market.currency && meta.exchange_timezone == market.timezone
        && meta.instrument_type == market.instrument_type && meta.interval == expected_interval,
        "Provider symbol, exchange, currency, timezone, type or interval does not match the configured market");
    let timezone: chrono_tz::Tz = market
        .timezone
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid exchange timezone"))?;
    let values = envelope
        .values
        .context("Market response has no OHLCV values")?;
    ensure!(
        !values.is_empty() && values.len() <= count,
        "Market response row count is invalid"
    );
    ensure!(dates.is_none() || values.len() < count, "Historical request reached its bar limit; narrow the date range to avoid silent truncation");
    let mut bars = Vec::new();
    for value in values {
        let (close_time, market_date) = if interval == "1m" {
            let start = NaiveDateTime::parse_from_str(&value.datetime, "%Y-%m-%d %H:%M:%S")
                .context("Invalid UTC minute timestamp")?
                .and_utc();
            (start + Duration::minutes(1), start.date_naive())
        } else {
            // Daily API timestamps remain in exchange local time. Conservatively wait
            // until next local midnight; never assume a US market close for global data.
            let date = NaiveDate::parse_from_str(&value.datetime, "%Y-%m-%d")
                .context("Invalid exchange date")?;
            let next = date
                .succ_opt()
                .context("Invalid next exchange date")?
                .and_hms_opt(0, 0, 0)
                .unwrap();
            (
                timezone
                    .from_local_datetime(&next)
                    .single()
                    .context("Ambiguous exchange day boundary")?
                    .with_timezone(&Utc),
                date,
            )
        };
        if close_time > now
            || dates.is_some_and(|(start, end)| market_date < start || market_date >= end)
        {
            continue;
        }
        let price = |s: &str| s.parse::<f64>().context("Invalid OHLC price");
        let volume = value
            .volume
            .context(
                "Volume is missing; this OHLCV model does not support volume-less instruments",
            )?
            .parse::<u64>()
            .context("Volume must be an unsigned integer")?;
        bars.push(Bar::new(
            Timestamp::from_datetime(close_time),
            Timestamp::from_datetime(close_time),
            price(&value.open)?,
            price(&value.high)?,
            price(&value.low)?,
            price(&value.close)?,
            volume,
            false,
        ));
    }
    bars.sort_by_key(|bar| bar.timestamp);
    ensure!(!bars.is_empty(), "No completed bars in the requested range");
    crate::validate_bars_monotonic_and_sound(&bars)?;
    ensure!(
        bars.windows(2).all(|w| w[0].timestamp < w[1].timestamp),
        "Duplicate market bars"
    );
    Ok(Snapshot {
        bars,
        fetched_at_ms: now.timestamp_millis(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn market() -> MarketIdentity {
        MarketIdentity {
            provider_symbol: "INFY".into(),
            exchange: "NSE".into(),
            currency: "INR".into(),
            timezone: "Asia/Kolkata".into(),
            instrument_type: "Common Stock".into(),
            periods_per_year: 252.0,
        }
    }
    fn response(interval: &str, dates: &[&str]) -> serde_json::Value {
        json!({"status":"ok","meta":{"symbol":"INFY","exchange":"NSE","currency":"INR","exchange_timezone":"Asia/Kolkata","type":"Common Stock","interval":interval},
            "values":dates.iter().map(|date| json!({"datetime":date,"open":"10","high":"12","low":"9","close":"11","volume":"123"})).collect::<Vec<_>>()})
    }
    fn parse_value(value: serde_json::Value, interval: &str) -> Result<Snapshot> {
        parse(
            &serde_json::to_vec(&value)?,
            &market(),
            interval,
            None,
            5000,
            "2026-10-13T10:01:00Z".parse().unwrap(),
        )
    }
    #[test]
    fn excludes_unfinished_minutes_and_orders_completed_bars() {
        let result = parse_value(
            response(
                "1min",
                &[
                    "2026-10-13 10:01:00",
                    "2026-10-13 10:00:00",
                    "2026-10-13 09:59:00",
                ],
            ),
            "1m",
        )
        .unwrap();
        assert_eq!(result.bars.len(), 2);
        assert_eq!(
            result.bars[1].timestamp.to_datetime().to_rfc3339(),
            "2026-10-13T10:01:00+00:00"
        );
    }
    #[test]
    fn daily_uses_exchange_timezone_and_excludes_today() {
        let result = parse_value(response("1day", &["2026-10-12", "2026-10-13"]), "1d").unwrap();
        assert_eq!(result.bars.len(), 1);
        assert_eq!(
            result.bars[0].timestamp.to_datetime().to_rfc3339(),
            "2026-10-12T18:30:00+00:00"
        );
    }
    #[test]
    fn rejects_identity_mismatch_duplicates_missing_volume_and_bad_prices() {
        for (field, bad) in [
            ("exchange", "BSE"),
            ("currency", "USD"),
            ("symbol", "OTHER"),
            ("interval", "1min"),
            ("exchange_timezone", "UTC"),
            ("type", "Physical Currency"),
        ] {
            let mut value = response("1day", &["2026-10-12"]);
            value["meta"][field] = json!(bad);
            assert!(parse_value(value, "1d").is_err());
        }
        assert!(parse_value(response("1day", &["2026-10-12", "2026-10-12"]), "1d").is_err());
        for (field, bad) in [
            ("volume", json!(null)),
            ("volume", json!("1.5")),
            ("close", json!("NaN")),
            ("high", json!("1")),
        ] {
            let mut value = response("1day", &["2026-10-12"]);
            value["values"][0][field] = bad;
            assert!(parse_value(value, "1d").is_err());
        }
    }
    #[test]
    fn provider_error_never_echoes_message_and_history_never_silently_truncates() {
        let error = parse_value(
            json!({"status":"error","code":429,"message":"secret-key"}),
            "1d",
        )
        .err()
        .unwrap()
        .to_string();
        assert!(error.contains("429") && !error.contains("secret-key"));
        let bytes = serde_json::to_vec(&response("1day", &["2026-10-12"])).unwrap();
        assert!(parse(
            &bytes,
            &market(),
            "1d",
            Some(("2026-10-01".parse().unwrap(), "2026-10-13".parse().unwrap())),
            1,
            Utc::now()
        )
        .is_err());
    }
}
