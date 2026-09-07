//! Data validation passes: monotonic timestamps and bar price integrity.

use crate::provider::DataError;
use crate::types::Bar;

/// Validates that a sequence of bars has strictly monotonically increasing timestamps
/// and physically valid prices (positive, non-NaN, high >= max(open, close), low <= min(open, close)).
pub fn validate_bars_monotonic_and_sound(bars: &[Bar]) -> Result<(), DataError> {
    if bars.is_empty() {
        return Ok(());
    }

    let mut prev_ts = None;

    for (idx, bar) in bars.iter().enumerate() {
        // 1. Monotonic timestamp check
        if let Some(prev) = prev_ts {
            if bar.timestamp <= prev {
                return Err(DataError::ValidationError(format!(
                    "Timestamp monotonicity violation at index {}: current {:?} <= previous {:?}",
                    idx, bar.timestamp, prev
                )));
            }
        }
        prev_ts = Some(bar.timestamp);

        // 2. Numerical validity checks
        for (field, val) in [
            ("open", bar.open),
            ("high", bar.high),
            ("low", bar.low),
            ("close", bar.close),
        ] {
            if val.is_nan() || val.is_infinite() {
                return Err(DataError::ValidationError(format!(
                    "Non-finite price detected for {} at index {}",
                    field, idx
                )));
            }
            if val <= 0.0 {
                return Err(DataError::ValidationError(format!(
                    "Non-positive price ({:.4}) for {} at index {}",
                    val, field, idx
                )));
            }
        }

        // 3. High/Low boundary checks
        let max_oc = bar.open.max(bar.close);
        let min_oc = bar.open.min(bar.close);

        if bar.high < bar.low {
            return Err(DataError::ValidationError(format!(
                "High price ({:.4}) < Low price ({:.4}) at index {}",
                bar.high, bar.low, idx
            )));
        }

        if bar.high < max_oc {
            return Err(DataError::ValidationError(format!(
                "High price ({:.4}) < max(open, close) ({:.4}) at index {}",
                bar.high, max_oc, idx
            )));
        }

        if bar.low > min_oc {
            return Err(DataError::ValidationError(format!(
                "Low price ({:.4}) > min(open, close) ({:.4}) at index {}",
                bar.low, min_oc, idx
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Timestamp;

    #[test]
    fn test_valid_bars_pass() {
        let bars = vec![
            Bar::same_bar(Timestamp(100), 10.0, 12.0, 9.0, 11.0, 100),
            Bar::same_bar(Timestamp(200), 11.0, 13.0, 10.5, 12.5, 200),
        ];
        assert!(validate_bars_monotonic_and_sound(&bars).is_ok());
    }

    #[test]
    fn test_non_monotonic_timestamp_fails() {
        let bars = vec![
            Bar::same_bar(Timestamp(200), 10.0, 12.0, 9.0, 11.0, 100),
            Bar::same_bar(Timestamp(100), 11.0, 13.0, 10.5, 12.5, 200),
        ];
        let err = validate_bars_monotonic_and_sound(&bars);
        assert!(matches!(err, Err(DataError::ValidationError(_))));
    }

    #[test]
    fn test_equal_timestamp_fails() {
        let bars = vec![
            Bar::same_bar(Timestamp(100), 10.0, 12.0, 9.0, 11.0, 100),
            Bar::same_bar(Timestamp(100), 11.0, 13.0, 10.5, 12.5, 200),
        ];
        let err = validate_bars_monotonic_and_sound(&bars);
        assert!(matches!(err, Err(DataError::ValidationError(_))));
    }

    #[test]
    fn test_high_less_than_low_fails() {
        let bars = vec![Bar::same_bar(Timestamp(100), 10.0, 8.0, 9.0, 8.5, 100)];
        let err = validate_bars_monotonic_and_sound(&bars);
        assert!(matches!(err, Err(DataError::ValidationError(_))));
    }

    #[test]
    fn test_negative_price_fails() {
        let bars = vec![Bar::same_bar(Timestamp(100), -10.0, 12.0, 9.0, 11.0, 100)];
        let err = validate_bars_monotonic_and_sound(&bars);
        assert!(matches!(err, Err(DataError::ValidationError(_))));
    }
}
