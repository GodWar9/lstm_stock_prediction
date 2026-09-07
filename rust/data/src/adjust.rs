//! Corporate actions adjustment logic with strict point-in-time enforcement.

use chrono::NaiveDate;
use crate::provider::DataError;
use crate::types::{Bar, CorporateAction, CorporateActionKind};

/// Adjusts a historical series of price bars for corporate actions strictly known as of `as_of`.
///
/// Corporate actions with effective dates strictly after `as_of` are ignored or rejected,
/// guaranteeing that no future split or dividend announcements leak into past feature values.
pub fn adjust_bars_pit(
    bars: &[Bar],
    actions: &[CorporateAction],
    as_of: NaiveDate,
) -> Result<Vec<Bar>, DataError> {
    if bars.is_empty() {
        return Ok(Vec::new());
    }

    // Filter actions effective strictly on or before `as_of`
    let valid_actions: Vec<&CorporateAction> = actions
        .iter()
        .filter(|a| a.effective_date <= as_of)
        .collect();

    // Sort actions by effective date ascending
    let mut sorted_actions = valid_actions;
    sorted_actions.sort_by_key(|a| a.effective_date);

    let mut adjusted_bars = Vec::with_capacity(bars.len());

    for bar in bars {
        let bar_date = bar.timestamp.to_datetime().date_naive();
        let mut split_multiplier = 1.0;
        let mut dividend_offset = 0.0;

        // Apply any actions that took effect between bar_date and as_of
        for action in &sorted_actions {
            if action.effective_date > bar_date && action.effective_date <= as_of {
                match &action.action {
                    CorporateActionKind::Split { ratio } => {
                        if *ratio <= 0.0 {
                            return Err(DataError::ValidationError("Split ratio must be positive".into()));
                        }
                        split_multiplier /= ratio;
                    }
                    CorporateActionKind::Dividend { amount } => {
                        dividend_offset += amount;
                    }
                }
            }
        }

        let mut adjusted_bar = bar.clone();
        adjusted_bar.open = (bar.open * split_multiplier) - dividend_offset;
        adjusted_bar.high = (bar.high * split_multiplier) - dividend_offset;
        adjusted_bar.low = (bar.low * split_multiplier) - dividend_offset;
        adjusted_bar.close = (bar.close * split_multiplier) - dividend_offset;
        adjusted_bar.volume = (bar.volume as f64 / split_multiplier).round() as u64;
        adjusted_bar.adjusted = true;

        adjusted_bars.push(adjusted_bar);
    }

    Ok(adjusted_bars)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use crate::types::Timestamp;

    #[test]
    fn test_split_adjustment_pit() {
        let d1 = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
        let d2 = NaiveDate::from_ymd_opt(2023, 1, 2).unwrap();
        let d3 = NaiveDate::from_ymd_opt(2023, 1, 3).unwrap();

        let ts1 = Timestamp::from_datetime(chrono::Utc.from_utc_datetime(&d1.and_hms_opt(16, 0, 0).unwrap()));
        let ts2 = Timestamp::from_datetime(chrono::Utc.from_utc_datetime(&d2.and_hms_opt(16, 0, 0).unwrap()));

        let bars = vec![
            Bar::same_bar(ts1, 100.0, 105.0, 95.0, 100.0, 1000),
            Bar::same_bar(ts2, 102.0, 106.0, 98.0, 104.0, 1200),
        ];

        // 2:1 split on d2
        let actions = vec![CorporateAction::split("AAPL", d2, 2.0)];

        // As of d1 (before split became effective): no adjustment to bar 1
        let adj_d1 = adjust_bars_pit(&bars, &actions, d1).unwrap();
        assert_eq!(adj_d1[0].close, 100.0);
        assert_eq!(adj_d1[0].volume, 1000);

        // As of d3 (after split): bar 1 (from d1) is adjusted by 1/2
        let adj_d3 = adjust_bars_pit(&bars, &actions, d3).unwrap();
        assert_eq!(adj_d3[0].close, 50.0);
        assert_eq!(adj_d3[0].volume, 2000);
        assert_eq!(adj_d3[1].close, 104.0); // bar 2 happened on d2, so split is already in effect
    }
}
