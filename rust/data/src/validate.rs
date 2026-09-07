//! Data validation passes: monotonic timestamps, bar price integrity, and leakage prevention.

use std::collections::HashSet;
use crate::provider::DataError;
use crate::types::{Bar, Timestamp};

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

/// Checks that no duplicate timestamps exist within the bar sequence.
pub fn check_duplicate_timestamps(bars: &[Bar]) -> Result<(), DataError> {
    let mut seen = HashSet::with_capacity(bars.len());
    for (idx, bar) in bars.iter().enumerate() {
        if !seen.insert(bar.timestamp) {
            return Err(DataError::LeakageViolation(format!(
                "Duplicate timestamp detected at index {}: {:?}",
                idx, bar.timestamp
            )));
        }
    }
    Ok(())
}

/// Verifies that feature availability occurs strictly before or at prediction time T,
/// and strictly before target observation start.
pub fn check_feature_target_leakage(
    feature_availability: Timestamp,
    target_observation_start: Timestamp,
) -> Result<(), DataError> {
    if feature_availability > target_observation_start {
        return Err(DataError::LeakageViolation(format!(
            "PIT LEAKAGE: Feature availability timestamp ({:?}) is strictly after target observation start ({:?})",
            feature_availability, target_observation_start
        )));
    }
    Ok(())
}

/// Verifies that Train, Validation, and Test index sets have zero overlap (purge and embargo integrity).
pub fn check_split_overlap(
    train_indices: &[usize],
    val_indices: &[usize],
    test_indices: &[usize],
) -> Result<(), DataError> {
    let mut train_set = HashSet::with_capacity(train_indices.len());
    for &idx in train_indices {
        train_set.insert(idx);
    }

    for &idx in val_indices {
        if train_set.contains(&idx) {
            return Err(DataError::LeakageViolation(format!(
                "SPLIT LEAKAGE: Index {} is present in both train and validation splits",
                idx
            )));
        }
    }

    let mut val_set = HashSet::with_capacity(val_indices.len());
    for &idx in val_indices {
        val_set.insert(idx);
    }

    for &idx in test_indices {
        if train_set.contains(&idx) {
            return Err(DataError::LeakageViolation(format!(
                "SPLIT LEAKAGE: Index {} is present in both train and test splits",
                idx
            )));
        }
        if val_set.contains(&idx) {
            return Err(DataError::LeakageViolation(format!(
                "SPLIT LEAKAGE: Index {} is present in both validation and test splits",
                idx
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
    fn test_duplicate_timestamps_fails() {
        let bars = vec![
            Bar::same_bar(Timestamp(100), 10.0, 12.0, 9.0, 11.0, 100),
            Bar::same_bar(Timestamp(100), 11.0, 13.0, 10.5, 12.5, 200),
        ];
        assert!(check_duplicate_timestamps(&bars).is_err());
    }

    #[test]
    fn test_pit_leakage_rejected() {
        let feat_avail = Timestamp(1000);
        let target_start = Timestamp(900); // target started in the past relative to feature availability!
        let err = check_feature_target_leakage(feat_avail, target_start);
        assert!(matches!(err, Err(DataError::LeakageViolation(_))));

        let valid_err = check_feature_target_leakage(Timestamp(900), Timestamp(1000));
        assert!(valid_err.is_ok());
    }

    #[test]
    fn test_split_overlap_rejected() {
        let train = vec![0, 1, 2, 3];
        let val = vec![3, 4, 5]; // index 3 overlap!
        let test = vec![6, 7];

        assert!(check_split_overlap(&train, &val, &test).is_err());

        let val_clean = vec![4, 5];
        assert!(check_split_overlap(&train, &val_clean, &test).is_ok());
    }
}
