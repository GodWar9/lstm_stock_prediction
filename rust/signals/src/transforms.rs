//! Signal transformation traits and standard pipeline filters.

use crate::signal::{Direction, Signal};

/// Trait for single-signal transformations (filtering, confidence-weighting, scaling).
pub trait SignalTransform: Send + Sync {
    /// Apply the transformation to a raw signal, producing a refined signal.
    fn apply(&self, raw: &Signal) -> Signal;
}

/// Trait for multi-asset cross-sectional signal transformations.
pub trait BatchSignalTransform: Send + Sync {
    /// Apply batch transformation across a universe of signals at a single observation timestamp.
    fn apply_batch(&self, signals: &[Signal]) -> Vec<Signal>;
}

/// Filter that zeros out signals failing return or confidence thresholds.
#[derive(Debug, Clone)]
pub struct ThresholdFilter {
    pub min_expected_return: f64,
    pub min_confidence: f64,
}

impl ThresholdFilter {
    pub fn new(min_expected_return: f64, min_confidence: f64) -> Self {
        Self {
            min_expected_return,
            min_confidence,
        }
    }
}

impl SignalTransform for ThresholdFilter {
    fn apply(&self, raw: &Signal) -> Signal {
        if raw.expected_return.abs() < self.min_expected_return || raw.confidence < self.min_confidence {
            Signal {
                direction: Direction::Flat,
                expected_return: 0.0,
                confidence: 0.0,
                ..raw.clone()
            }
        } else {
            raw.clone()
        }
    }
}

/// Weights expected return by calibrated model confidence: `expected_return * confidence^exponent`.
#[derive(Debug, Clone)]
pub struct ConfidenceWeighting {
    pub exponent: f64,
}

impl ConfidenceWeighting {
    pub fn new(exponent: f64) -> Self {
        Self { exponent }
    }
}

impl Default for ConfidenceWeighting {
    fn default() -> Self {
        Self { exponent: 1.0 }
    }
}

impl SignalTransform for ConfidenceWeighting {
    fn apply(&self, raw: &Signal) -> Signal {
        let weighted_return = raw.expected_return * raw.confidence.powf(self.exponent);
        Signal {
            expected_return: weighted_return,
            ..raw.clone()
        }
    }
}

/// Sequential composition of multiple `SignalTransform` stages.
pub struct CompositeTransform {
    transforms: Vec<Box<dyn SignalTransform>>,
}

impl CompositeTransform {
    pub fn new(transforms: Vec<Box<dyn SignalTransform>>) -> Self {
        Self { transforms }
    }
}

impl SignalTransform for CompositeTransform {
    fn apply(&self, raw: &Signal) -> Signal {
        let mut current = raw.clone();
        for t in &self.transforms {
            current = t.apply(&current);
        }
        current
    }
}

/// Cross-sectional percentile ranking across an investment universe.
/// Longs top N performers, Shorts bottom N performers, sets middle to Flat.
#[derive(Debug, Clone)]
pub struct CrossSectionalRank {
    pub top_n: usize,
    pub bottom_n: usize,
}

impl CrossSectionalRank {
    pub fn new(top_n: usize, bottom_n: usize) -> Self {
        Self { top_n, bottom_n }
    }
}

impl BatchSignalTransform for CrossSectionalRank {
    fn apply_batch(&self, signals: &[Signal]) -> Vec<Signal> {
        if signals.is_empty() {
            return Vec::new();
        }

        let mut indexed: Vec<(usize, &Signal)> = signals.iter().enumerate().collect();
        // Sort descending by expected return
        indexed.sort_by(|a, b| {
            b.1.expected_return
                .partial_cmp(&a.1.expected_return)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let total = indexed.len();
        let top_cutoff = self.top_n.min(total);
        let bottom_cutoff = total.saturating_sub(self.bottom_n);

        let mut results = signals.to_vec();

        for (rank, &(orig_idx, orig_sig)) in indexed.iter().enumerate() {
            if rank < top_cutoff && orig_sig.expected_return > 0.0 {
                results[orig_idx] = Signal {
                    direction: Direction::Long,
                    ..orig_sig.clone()
                };
            } else if rank >= bottom_cutoff && orig_sig.expected_return < 0.0 {
                results[orig_idx] = Signal {
                    direction: Direction::Short,
                    ..orig_sig.clone()
                };
            } else {
                results[orig_idx] = Signal {
                    direction: Direction::Flat,
                    expected_return: 0.0,
                    confidence: 0.0,
                    ..orig_sig.clone()
                };
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quant_instruments::InstrumentId;

    fn make_test_signal(id: u32, ret: f64, conf: f64) -> Signal {
        Signal {
            direction: if ret > 0.0 {
                Direction::Long
            } else if ret < 0.0 {
                Direction::Short
            } else {
                Direction::Flat
            },
            expected_return: ret,
            confidence: conf,
            horizon_bars: 1,
            instrument: InstrumentId(id),
            symbol: format!("SYM{}", id),
            model_id: "lstm_v1".to_string(),
            as_of: 1000,
        }
    }

    #[test]
    fn test_threshold_filter() {
        let filter = ThresholdFilter::new(0.01, 0.5);

        let strong_sig = make_test_signal(1, 0.02, 0.8);
        assert_eq!(filter.apply(&strong_sig).direction, Direction::Long);

        let low_ret_sig = make_test_signal(2, 0.005, 0.8);
        assert_eq!(filter.apply(&low_ret_sig).direction, Direction::Flat);

        let low_conf_sig = make_test_signal(3, 0.03, 0.4);
        assert_eq!(filter.apply(&low_conf_sig).direction, Direction::Flat);
    }

    #[test]
    fn test_confidence_weighting() {
        let weighting = ConfidenceWeighting::new(1.0);
        let sig = make_test_signal(1, 0.02, 0.5);
        let weighted = weighting.apply(&sig);
        assert_eq!(weighted.expected_return, 0.01);
    }

    #[test]
    fn test_composite_transform() {
        let filter = ThresholdFilter::new(0.005, 0.4);
        let weighting = ConfidenceWeighting::new(2.0);
        let composite = CompositeTransform::new(vec![Box::new(filter), Box::new(weighting)]);

        let sig = make_test_signal(1, 0.02, 0.5);
        let result = composite.apply(&sig);
        assert_eq!(result.direction, Direction::Long);
        assert_eq!(result.expected_return, 0.02 * 0.25);
    }

    #[test]
    fn test_cross_sectional_rank() {
        let ranker = CrossSectionalRank::new(1, 1);
        let universe = vec![
            make_test_signal(1, 0.05, 0.8),  // Top 1 -> Long
            make_test_signal(2, 0.01, 0.8),  // Middle -> Flat
            make_test_signal(3, -0.04, 0.8), // Bottom 1 -> Short
        ];

        let ranked = ranker.apply_batch(&universe);
        assert_eq!(ranked[0].direction, Direction::Long);
        assert_eq!(ranked[1].direction, Direction::Flat);
        assert_eq!(ranked[2].direction, Direction::Short);
    }
}
