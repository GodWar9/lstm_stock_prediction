//! Read-only verification and conservative PIT import of captured Alpaca minute bars.
use crate::{Bar, Timestamp};
use anyhow::{bail, ensure, Context, Result};
use chrono::{DateTime, Duration, NaiveDate};
use quant_calendar::{TradingCalendar, UsEquityCalendar};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Read},
    path::Path,
};

pub const SCHEMA_VERSION: u64 = 1;
pub const MAX_RECORD_BYTES: u64 = 1024 * 1024 + 4096;

#[derive(Debug, Serialize)]
pub struct CaptureAudit {
    pub synthetic: bool,
    pub session_id: String,
    pub feed: String,
    pub symbols: Vec<String>,
    pub records: u64,
    pub gaps: u64,
    pub clean_shutdown: bool,
    pub content_sha256: String,
}

/// Validate the checksum before interpreting any data. This detects accidental
/// corruption, not an adversary who can rewrite the entire journal and its hashes.
pub fn checksum(record: &Value) -> String {
    let mut payload = record.clone();
    payload.as_object_mut().unwrap().remove("checksum");
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&payload).unwrap())
    )
}

pub fn verify(path: &Path) -> Result<CaptureAudit> {
    scan(path, |_| Ok(()))
}

fn scan(path: &Path, mut accept: impl FnMut(&Value) -> Result<()>) -> Result<CaptureAudit> {
    ensure!(
        path.is_dir() && !fs::symlink_metadata(path)?.file_type().is_symlink(),
        "Expected a captured session directory"
    );
    let mut files = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        ensure!(files.len() < 4096, "Too many journal segments");
        ensure!(
            entry.file_type()?.is_file(),
            "Unexpected journal directory or symlink"
        );
        ensure!(
            entry.file_name().to_string_lossy().ends_with(".ndjson"),
            "Unexpected journal file"
        );
        files.push(entry.path());
    }
    files.sort();
    ensure!(!files.is_empty(), "Empty capture");
    let mut audit = CaptureAudit {
        synthetic: false,
        session_id: String::new(),
        feed: String::new(),
        symbols: vec![],
        records: 0,
        gaps: 0,
        clean_shutdown: false,
        content_sha256: String::new(),
    };
    let mut chain = String::new();
    let mut last_received = 0;
    let mut ended = false;
    for (index, path) in files.iter().enumerate() {
        ensure!(
            path.file_name().unwrap().to_string_lossy() == format!("{:06}.ndjson", index + 1),
            "Missing or misnamed journal segment"
        );
        let mut reader = BufReader::new(fs::File::open(path)?);
        loop {
            let mut line = Vec::new();
            let n = reader
                .by_ref()
                .take(MAX_RECORD_BYTES + 1)
                .read_until(b'\n', &mut line)?;
            if n == 0 {
                break;
            }
            ensure!(
                n as u64 <= MAX_RECORD_BYTES && line.ends_with(b"\n"),
                "Truncated or oversized journal record"
            );
            let record: Value = serde_json::from_slice(&line).context("Malformed journal JSON")?;
            ensure!(record.is_object(), "Journal record must be an object");
            ensure!(!ended, "Records after stop marker");
            ensure!(
                record["schema_version"] == SCHEMA_VERSION,
                "Unsupported journal schema; legacy captures must not be silently imported"
            );
            ensure!(
                record["seq"].as_u64() == Some(audit.records + 1),
                "Journal sequence discontinuity"
            );
            ensure!(
                record["previous_checksum"] == chain,
                "Journal checksum chain discontinuity"
            );
            chain = checksum(&record);
            ensure!(record["checksum"] == chain, "Journal checksum mismatch");
            let received = record["received_at_ms"]
                .as_i64()
                .context("Missing receipt timestamp")?;
            ensure!(
                received >= last_received && received > 0,
                "Receipt clock moved backwards"
            );
            last_received = received;
            if audit.records == 0 {
                ensure!(record["type"] == "session", "Missing session header");
                audit.session_id = record["session_id"]
                    .as_str()
                    .context("Missing session ID")?
                    .into();
                audit.feed = record["feed"].as_str().context("Missing feed")?.into();
                audit.synthetic = record["synthetic"].as_bool().unwrap_or(false);
                audit.symbols = serde_json::from_value(record["symbols"].clone())?;
                ensure!(
                    !audit.symbols.is_empty() && !audit.session_id.is_empty(),
                    "Empty session metadata"
                );
            } else {
                ensure!(
                    record["session_id"] == audit.session_id,
                    "Mixed capture sessions"
                );
                match record["type"].as_str() {
                    Some("market") => {
                        ensure!(record["feed"] == audit.feed, "Mixed capture feeds");
                        ensure!(
                            audit.symbols.iter().any(|s| record["event"]["S"] == *s),
                            "Unsubscribed symbol in journal"
                        );
                        validate_event(&record["event"], received)?;
                    }
                    Some("gap") => audit.gaps += 1,
                    Some("stop") => {
                        ended = true;
                        audit.clean_shutdown = record["reason"] == "shutdown";
                    }
                    _ => bail!("Unknown journal record type"),
                }
            }
            audit.records += 1;
            accept(&record)?;
        }
    }
    audit.content_sha256 = chain;
    Ok(audit)
}

pub fn validate_event(event: &Value, received: i64) -> Result<()> {
    let timestamp =
        DateTime::parse_from_rfc3339(event["t"].as_str().context("Missing event timestamp")?)?;
    ensure!(
        timestamp.timestamp_nanos_opt().is_some()
            && timestamp.timestamp_millis() > 0
            && timestamp.timestamp_millis() <= received.saturating_add(5000),
        "Invalid or future event timestamp"
    );
    let positive = |name: &str| -> Result<f64> {
        let value = event[name].as_f64().context("Missing price")?;
        ensure!(value.is_finite() && value > 0.0, "Invalid price");
        Ok(value)
    };
    let size = |name: &str| -> Result<()> {
        ensure!(
            event[name].as_u64().is_some(),
            "Invalid integer size/volume"
        );
        Ok(())
    };
    match event["T"].as_str() {
        Some("t") => {
            positive("p")?;
            size("s")?;
        }
        Some("q") => {
            // A zero side denotes an absent quote. Crossed quotes can legitimately occur.
            for name in ["bp", "ap"] {
                ensure!(
                    event[name]
                        .as_f64()
                        .is_some_and(|v| v.is_finite() && v >= 0.0),
                    "Invalid quote"
                );
            }
            size("bs")?;
            size("as")?;
        }
        Some("b" | "u") => {
            let o = positive("o")?;
            let h = positive("h")?;
            let l = positive("l")?;
            let c = positive("c")?;
            ensure!(
                h >= o.max(c) && l <= o.min(c) && h >= l,
                "Invalid OHLC bounds"
            );
            size("v")?;
            ensure!(
                timestamp.timestamp() % 60 == 0 && timestamp.timestamp_subsec_nanos() == 0,
                "Minute bar timestamp is not aligned"
            );
            ensure!(
                timestamp.timestamp_millis() + 60_000 <= received,
                "Minute bar arrived before its close"
            );
        }
        Some("c") => {
            positive("op")?;
            positive("cp")?;
            size("os")?;
            size("cs")?;
        }
        Some("x") => {
            positive("p")?;
            size("s")?;
            ensure!(
                matches!(event["a"].as_str(), Some("C" | "E")),
                "Invalid cancellation action"
            );
        }
        _ => bail!("Unsupported market event"),
    }
    Ok(())
}

/// Freeze the first published provider minute bar. Revisions stay in the capture,
/// never overwrite a value that a historical decision has already observed.
pub fn import_minutes(
    path: &Path,
    symbol: &str,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<(CaptureAudit, Vec<Bar>)> {
    let calendar = UsEquityCalendar::new("NYSE");
    let mut minutes = BTreeMap::new();
    let mut duplicates = 0;
    let audit = scan(path, |record| {
        let e = &record["event"];
        if record["type"] != "market" || e["T"] != "b" || e["S"] != symbol {
            return Ok(());
        }
        let t = DateTime::parse_from_rfc3339(e["t"].as_str().unwrap())?.to_utc();
        if t.date_naive() < start || t.date_naive() > end {
            return Ok(());
        }
        let Some(session) = calendar.session(t.date_naive()) else {
            return Ok(());
        };
        let close = t + Duration::minutes(1);
        if t < session.open_utc || close > session.close_utc {
            return Ok(());
        }
        let bar = Bar::new(
            Timestamp(close.timestamp_nanos_opt().context("Timestamp overflow")?),
            Timestamp(
                record["received_at_ms"]
                    .as_i64()
                    .unwrap()
                    .checked_mul(1_000_000)
                    .context("Receipt timestamp overflow")?,
            ),
            e["o"].as_f64().unwrap(),
            e["h"].as_f64().unwrap(),
            e["l"].as_f64().unwrap(),
            e["c"].as_f64().unwrap(),
            e["v"].as_u64().unwrap(),
            false,
        );
        if let std::collections::btree_map::Entry::Vacant(entry) = minutes.entry(t) {
            entry.insert(bar);
        } else {
            duplicates += 1;
        }
        ensure!(
            minutes.len() <= 2_000_000,
            "Capture exceeds import row limit"
        );
        Ok(())
    })?;
    ensure!(
        audit.clean_shutdown,
        "Capture was not cleanly closed; stop ingestion before import"
    );
    ensure!(audit.gaps == 0, "Capture contains reconnect gaps; collect a clean session instead of concealing missing data");
    ensure!(
        matches!(audit.feed.as_str(), "iex" | "sip"),
        "Research import requires an undelayed stock feed; test and delayed feeds are excluded"
    );
    ensure!(
        duplicates == 0,
        "Duplicate original minute bars; investigate capture before import"
    );
    ensure!(
        !minutes.is_empty(),
        "No regular-session minute bars in requested range"
    );
    let ordered: Vec<_> = minutes.into_iter().collect();
    for pair in ordered.windows(2) {
        let (previous, previous_bar) = &pair[0];
        let (current, current_bar) = &pair[1];
        let session = calendar.session(previous.date_naive()).unwrap();
        let next = if *previous + Duration::minutes(1) < session.close_utc {
            *previous + Duration::minutes(1)
        } else {
            calendar
                .session(calendar.next_trading_day(previous.date_naive()))
                .unwrap()
                .open_utc
        };
        ensure!(*current == next, "Missing regular-session minute at {next}; no interpolation or hidden backfill is allowed");
        ensure!(
            current_bar.availability_timestamp > previous_bar.availability_timestamp,
            "Out-of-order minute availability"
        );
        ensure!(previous_bar.availability_timestamp < current_bar.timestamp, "Late bar was unavailable before the next close; capture cannot support a one-bar target");
    }
    Ok((audit, ordered.into_iter().map(|(_, b)| b).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn checksums_survive_decimal_price_json_roundtrip() {
        let mut price = 100.0_f64;
        for index in 0..2000 {
            price *= if index % 3 == 0 { 1.0003 } else { 0.9999 };
            let record = json!({"event":{"p":price,"h":price+0.02},"seq":index});
            let restored: Value = serde_json::from_str(&record.to_string()).unwrap();
            assert_eq!(checksum(&record), checksum(&restored));
        }
    }

    fn fixture(extra: Option<Value>) -> Vec<Value> {
        let open = DateTime::parse_from_rfc3339("2024-01-02T14:30:00Z").unwrap();
        let mut records = vec![
            json!({"type":"session","feed":"iex","symbols":["AAPL"],"received_at_ms":open.timestamp_millis()}),
        ];
        for i in 0..3 {
            records.push(json!({"type":"market","feed":"iex","received_at_ms":(open + Duration::minutes(i+1)).timestamp_millis()+500,
                "event":{"T":"b","S":"AAPL","t":(open+Duration::minutes(i)).to_rfc3339(),"o":100.0,"h":101.0,"l":99.0,"c":100.5,"v":500}}));
        }
        if let Some(extra) = extra {
            records.push(extra);
        }
        records.push(json!({"type":"stop","reason":"shutdown","received_at_ms":(open+Duration::minutes(4)).timestamp_millis()}));
        records
    }
    fn write(path: &Path, records: Vec<Value>) {
        let mut chain = String::new();
        let mut output = String::new();
        for (index, mut record) in records.into_iter().enumerate() {
            record["schema_version"] = json!(1);
            record["seq"] = json!(index + 1);
            record["session_id"] = json!("fixture");
            record["previous_checksum"] = json!(chain);
            chain = checksum(&record);
            record["checksum"] = json!(chain);
            output.push_str(&record.to_string());
            output.push('\n');
        }
        fs::write(path.join("000001.ndjson"), output).unwrap();
    }
    fn import(path: &Path) -> Result<(CaptureAudit, Vec<Bar>)> {
        import_minutes(
            path,
            "AAPL",
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 3).unwrap(),
        )
    }
    #[test]
    fn first_publication_preserves_receipt_time_and_ignores_later_revision() {
        let tmp = tempfile::tempdir().unwrap();
        let mut revised = fixture(None)[1].clone();
        revised["event"]["T"] = json!("u");
        revised["event"]["c"] = json!(100.9);
        revised["received_at_ms"] = json!(DateTime::parse_from_rfc3339("2024-01-02T14:33:30Z")
            .unwrap()
            .timestamp_millis());
        write(tmp.path(), fixture(Some(revised)));
        let (audit, bars) = import(tmp.path()).unwrap();
        assert!(audit.clean_shutdown);
        assert_eq!(bars.len(), 3);
        assert_eq!(bars[0].close, 100.5);
        assert_eq!(
            bars[0].availability_timestamp.as_nanos() - bars[0].timestamp.as_nanos(),
            500_000_000
        );
    }
    #[test]
    fn rejects_missing_duplicate_gap_unclean_and_late_bars() {
        let tmp = tempfile::tempdir().unwrap();
        for kind in [
            "missing",
            "duplicate",
            "gap",
            "unclean",
            "late",
            "test_feed",
        ] {
            let mut records = fixture(None);
            match kind {
                "missing" => {
                    records.remove(2);
                }
                "duplicate" => records.insert(2, records[1].clone()),
                "gap" => records.insert(
                    1,
                    json!({"type":"gap","received_at_ms":records[0]["received_at_ms"]}),
                ),
                "unclean" => {
                    records.pop();
                }
                "late" => records[1]["received_at_ms"] = records[2]["received_at_ms"].clone(),
                _ => {
                    for r in &mut records {
                        if r.get("feed").is_some() {
                            r["feed"] = json!("test");
                        }
                    }
                }
            }
            write(tmp.path(), records);
            assert!(import(tmp.path()).is_err(), "must reject {kind}");
        }
    }
    #[test]
    fn verifier_detects_corruption_truncation_sequence_and_missing_segments() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), fixture(None));
        let path = tmp.path().join("000001.ndjson");
        let original = fs::read_to_string(&path).unwrap();
        for corrupt in [
            original.replace("100.5", "100.6"),
            original[..original.len() - 2].into(),
            original.replace("\"seq\":2", "\"seq\":9"),
        ] {
            fs::write(&path, corrupt).unwrap();
            assert!(verify(tmp.path()).is_err());
        }
        fs::write(&path, &original).unwrap();
        fs::rename(path, tmp.path().join("000002.ndjson")).unwrap();
        assert!(verify(tmp.path()).is_err());
    }
    #[test]
    fn rejects_future_and_malformed_market_events() {
        let event = fixture(None)[1].clone();
        let received = event["received_at_ms"].as_i64().unwrap();
        for (key, val) in [
            ("h", json!(98)),
            ("v", json!(-1)),
            ("t", json!("2099-01-01T14:30:00Z")),
            ("t", json!("2024-01-02T14:30:30Z")),
        ] {
            let mut bad = event["event"].clone();
            bad[key] = val;
            assert!(validate_event(&bad, received).is_err());
        }
    }
}
