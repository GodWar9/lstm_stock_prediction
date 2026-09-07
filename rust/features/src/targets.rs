//! Target generation and labeling for ML model training.
//!
//! Includes forward returns, binary/ternary direction labels,
//! and point-in-time safety guards ensuring target values are computed strictly
//! from future bars and separated from feature generation.

use quant_data::{Bar, Timestamp};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum TargetError {
    #[error("Horizon must be greater than 0, got {0}")]
    InvalidHorizon(usize),
    #[error("Target index out of bounds")]
    OutOfBounds,
    #[error("Prices must be strictly positive")]
    NonPositivePrice,
}

/// Direction classification label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectionLabel {
    Down = -1,
    Neutral = 0,
    Up = 1,
}

/// Single target record associated with an observation timestamp.
#[derive(Debug, Clone, PartialEq)]
pub struct TargetRow {
    /// Timestamp when the observation was made (features available up to this point).
    pub timestamp: Timestamp,
    /// Future timestamp when the target materialized.
    pub target_timestamp: Timestamp,
    /// Forward log return over horizon: ln(Close_{t+h} / Close_t).
    pub forward_return: f64,
    /// Categorical classification label based on threshold.
    pub direction: DirectionLabel,
}

/// Target generator with configurable lookahead horizon and neutral threshold.
#[derive(Debug, Clone)]
pub struct TargetGenerator {
    pub horizon: usize,
    pub neutral_threshold: f64,
}

impl TargetGenerator {
    pub fn new(horizon: usize, neutral_threshold: f64) -> Result<Self, TargetError> {
        if horizon == 0 {
            return Err(TargetError::InvalidHorizon(0));
        }
        Ok(Self {
            horizon,
            neutral_threshold: neutral_threshold.abs(),
        })
    }

    /// Default 1-day forward target with 0.1% neutral threshold.
    pub fn daily_default() -> Self {
        Self::new(1, 0.001).unwrap()
    }

    /// Compute targets for a slice of chronologically ordered bars.
    /// Bars near the end that do not have `horizon` future bars will not have targets.
    pub fn compute_targets(&self, bars: &[Bar]) -> Result<Vec<TargetRow>, TargetError> {
        if bars.len() <= self.horizon {
            return Ok(Vec::new());
        }

        let mut targets = Vec::with_capacity(bars.len() - self.horizon);
        for i in 0..(bars.len() - self.horizon) {
            let current = &bars[i];
            let future = &bars[i + self.horizon];

            // Point-in-time guard: ensure target timestamp is strictly in the future
            assert!(
                future.timestamp > current.timestamp,
                "Future bar timestamp must be > current bar timestamp"
            );

            if current.close <= 0.0 || future.close <= 0.0 {
                return Err(TargetError::NonPositivePrice);
            }

            let fwd_return = (future.close / current.close).ln();
            let direction = if fwd_return > self.neutral_threshold {
                DirectionLabel::Up
            } else if fwd_return < -self.neutral_threshold {
                DirectionLabel::Down
            } else {
                DirectionLabel::Neutral
            };

            targets.push(TargetRow {
                timestamp: current.timestamp,
                target_timestamp: future.timestamp,
                forward_return: fwd_return,
                direction,
            });
        }

        Ok(targets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bar(ts: i64, close: f64) -> Bar {
        Bar::same_bar(Timestamp(ts), close, close, close, close, 1000)
    }

    #[test]
    fn test_target_generation() {
        let bars = vec![
            make_bar(1, 100.0),
            make_bar(2, 105.0),
            make_bar(3, 102.0),
            make_bar(4, 110.0),
        ];

        let gen = TargetGenerator::new(1, 0.01).unwrap();
        let targets = gen.compute_targets(&bars).unwrap();

        // 4 bars with horizon 1 produces 3 targets
        assert_eq!(targets.len(), 3);

        // Bar 0 -> Bar 1: 100 -> 105 (+5%) -> Up
        assert_eq!(targets[0].timestamp, Timestamp(1));
        assert_eq!(targets[0].target_timestamp, Timestamp(2));
        assert_eq!(targets[0].direction, DirectionLabel::Up);

        // Bar 1 -> Bar 2: 105 -> 102 (-2.8%) -> Down
        assert_eq!(targets[1].direction, DirectionLabel::Down);
    }

    #[test]
    fn test_neutral_threshold() {
        let bars = vec![
            make_bar(1, 100.0),
            make_bar(2, 100.05), // +0.05%
        ];

        let gen = TargetGenerator::new(1, 0.001).unwrap(); // 0.1% threshold
        let targets = gen.compute_targets(&bars).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].direction, DirectionLabel::Neutral);
    }
}
