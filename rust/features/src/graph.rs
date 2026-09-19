//! Feature Graph orchestrator: manages a set of features, determines
//! maximum lookback requirements, and computes feature vectors for incoming bars.

use crate::traits::Feature;
use crate::window::BarWindow;
use quant_data::{Bar, Timestamp};
use std::collections::HashMap;

/// A row of computed features at a specific bar timestamp.
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureRow {
    pub timestamp: Timestamp,
    pub values: HashMap<String, f64>,
}

/// Orchestrator for evaluating a graph of features over incoming market bars.
#[derive(Debug)]
pub struct FeatureGraph {
    features: Vec<Box<dyn Feature>>,
    max_lookback: usize,
    window: BarWindow,
}

impl FeatureGraph {
    /// Create a new FeatureGraph from a list of features.
    pub fn new(features: Vec<Box<dyn Feature>>) -> Self {
        let max_lookback = features.iter().map(|f| f.lookback()).max().unwrap_or(1);
        let capacity = max_lookback.max(2);
        Self {
            features,
            max_lookback,
            window: BarWindow::new(capacity),
        }
    }

    /// Builder helper to add a feature.
    pub fn with_feature(mut self, feature: Box<dyn Feature>) -> Self {
        self.add_feature(feature);
        self
    }

    /// Add a feature and update internal window capacity if needed.
    pub fn add_feature(&mut self, feature: Box<dyn Feature>) {
        let lb = feature.lookback();
        if lb > self.max_lookback {
            self.max_lookback = lb;
            let mut new_win = BarWindow::new(lb);
            for bar in self.window.iter() {
                new_win.push(bar.clone());
            }
            self.window = new_win;
        }
        self.features.push(feature);
    }

    /// Returns the maximum lookback across all configured features.
    pub fn max_lookback(&self) -> usize {
        self.max_lookback
    }

    /// Number of features in the graph.
    pub fn feature_count(&self) -> usize {
        self.features.len()
    }

    /// Ingest a bar and compute features.
    /// Returns `None` if still in warmup period (any feature returns `None`).
    pub fn push_bar(&mut self, bar: Bar) -> Option<FeatureRow> {
        let ts = bar.timestamp;
        self.window.push(bar);

        let mut row_values = HashMap::with_capacity(self.features.len());
        for feat in &self.features {
            let val = feat.compute(&self.window)?;
            row_values.insert(feat.name().to_string(), val);
        }

        Some(FeatureRow {
            timestamp: ts,
            values: row_values,
        })
    }

    /// Batch compute features over an ordered slice of bars.
    /// Returns only rows after the initial warmup period.
    pub fn compute_batch(&mut self, bars: &[Bar]) -> Vec<FeatureRow> {
        let mut results = Vec::new();
        for bar in bars {
            if let Some(row) = self.push_bar(bar.clone()) {
                results.push(row);
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moving_avg::Sma;
    use crate::rsi::Rsi;

    fn make_bar(ts: i64, close: f64) -> Bar {
        Bar::same_bar(Timestamp(ts), close, close, close, close, 1000)
    }

    #[test]
    fn test_feature_graph_warmup_and_compute() {
        let mut graph = FeatureGraph::new(vec![Box::new(Sma::new(3)), Box::new(Rsi::new(3))]);

        assert_eq!(graph.feature_count(), 2);
        assert!(graph.max_lookback() >= 3);

        let bars = vec![
            make_bar(1, 10.0),
            make_bar(2, 20.0),
            make_bar(3, 30.0),
            make_bar(4, 40.0),
            make_bar(5, 50.0),
        ];

        let results = graph.compute_batch(&bars);
        // Lookback for RSI(3) is 4 bars (period + 1), so bars 1..3 will be warmup,
        // bar 4 and bar 5 produce output rows.
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].timestamp, Timestamp(4));
        assert!(results[0].values.contains_key("sma"));
        assert!(results[0].values.contains_key("rsi"));
    }
}
