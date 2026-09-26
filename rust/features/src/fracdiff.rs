//! Fractionally Differentiated Stationarity (FracDiff).
//!
//! Implements the fixed-width-window fractional differentiation method described
//! in Marcos López de Prado's "Advances in Financial Machine Learning" (Chapter 5).
//!
//! Fractional differentiation with d ∈ (0, 1) provides a principled balance between:
//! - Stationarity (achieved at d = 1, standard differencing)
//! - Memory retention (preserved at d = 0, raw prices)
//!
//! The weights for fractional differentiation of order d are:
//! w_k = (-1)^k * Π_{i=0}^{k-1} (d - i) / (k!)
//!
//! Which can be computed iteratively: w_0 = 1, w_k = -w_{k-1} * (d - k + 1) / k

use crate::traits::Feature;
use crate::window::BarWindow;

/// Precomputes FracDiff weights for a given order `d` and truncation threshold.
/// Weights are truncated when |w_k| < threshold to bound the window size.
///
/// # Arguments
/// * `d` - Fractional differentiation order, typically ∈ [0.3, 0.7]
/// * `threshold` - Minimum absolute weight magnitude before truncation (e.g. 1e-5)
/// * `max_window` - Hard cap on number of weights to compute
///
/// # Returns
/// Vector of weights from w_0 (newest) to w_K (oldest).
pub fn compute_fracdiff_weights(d: f64, threshold: f64, max_window: usize) -> Vec<f64> {
    let mut weights = Vec::with_capacity(max_window.min(256));
    let mut w = 1.0_f64;
    weights.push(w);

    for k in 1..max_window {
        w *= -(d - (k as f64) + 1.0) / (k as f64);
        if w.abs() < threshold {
            break;
        }
        weights.push(w);
    }

    weights
}

/// Fractionally Differentiated feature using a fixed-window approach.
///
/// Given a series of close prices [x_{t-K}, ..., x_{t-1}, x_t],
/// the fractionally differenced value is: y_t = Σ_{k=0}^{K} w_k * x_{t-k}
///
/// This preserves long-range memory (autocorrelation) while achieving
/// sufficient stationarity for ML model consumption.
#[derive(Debug)]
pub struct FracDiff {
    /// Fractional differentiation order d ∈ (0, 1).
    d: f64,
    /// Precomputed weights truncated at threshold.
    weights: Vec<f64>,
}

impl FracDiff {
    /// Create a new FracDiff feature with differentiation order `d`.
    ///
    /// # Arguments
    /// * `d` - Differentiation order, typically 0.3 to 0.7
    /// * `threshold` - Weight truncation threshold (default 1e-5)
    /// * `max_window` - Maximum lookback window (default 500)
    pub fn new(d: f64, threshold: f64, max_window: usize) -> Self {
        assert!(
            (0.0..=1.0).contains(&d),
            "d must be in [0.0, 1.0], got {}",
            d
        );
        let weights = compute_fracdiff_weights(d, threshold, max_window);
        Self { d, weights }
    }

    /// Create with standard parameters suitable for daily equity prices.
    /// d=0.5 provides a balanced tradeoff between stationarity and memory.
    pub fn standard() -> Self {
        Self::new(0.5, 1e-5, 500)
    }

    /// Create with custom d and default truncation parameters.
    pub fn with_d(d: f64) -> Self {
        Self::new(d, 1e-5, 500)
    }

    /// The differentiation order d.
    pub fn d(&self) -> f64 {
        self.d
    }

    /// The precomputed weights (w_0 to w_K).
    pub fn weights(&self) -> &[f64] {
        &self.weights
    }

    /// The effective window length needed.
    pub fn window_len(&self) -> usize {
        self.weights.len()
    }

    /// Apply FracDiff to a slice of values, returning the fractionally
    /// differenced value for the most recent observation.
    ///
    /// `series[0]` = oldest, `series[len-1]` = newest.
    pub fn apply_to_slice(&self, series: &[f64]) -> Option<f64> {
        let k = self.weights.len();
        if series.len() < k {
            return None;
        }

        let offset = series.len() - k;
        let mut result = 0.0_f64;
        for (i, &w) in self.weights.iter().enumerate() {
            // w_0 * x_t + w_1 * x_{t-1} + ... + w_K * x_{t-K}
            result += w * series[offset + k - 1 - i];
        }

        Some(result)
    }

    /// Apply FracDiff to an entire series, returning fractionally differenced
    /// values for each position that has sufficient lookback.
    pub fn apply_to_series(&self, series: &[f64]) -> Vec<f64> {
        let k = self.weights.len();
        if series.len() < k {
            return Vec::new();
        }

        let out_len = series.len() - k + 1;
        let mut result = Vec::with_capacity(out_len);

        for t in (k - 1)..series.len() {
            let mut val = 0.0_f64;
            for (i, &w) in self.weights.iter().enumerate() {
                val += w * series[t - i];
            }
            result.push(val);
        }

        result
    }
}

impl Feature for FracDiff {
    fn name(&self) -> &str {
        "fracdiff"
    }

    fn version(&self) -> u32 {
        1
    }

    fn lookback(&self) -> usize {
        self.weights.len()
    }

    fn compute(&self, window: &BarWindow) -> Option<f64> {
        let k = self.weights.len();
        if window.len() < k {
            return None;
        }

        let mut result = 0.0_f64;
        let offset = window.len() - 1;

        for (i, &w) in self.weights.iter().enumerate() {
            let close = window.get(offset - i)?.close;
            if close <= 0.0 {
                return None;
            }
            // Apply to log prices for scale invariance
            result += w * close.ln();
        }

        Some(result)
    }
}

/// Determine the minimum d value that achieves stationarity (via ADF test p-value)
/// for a given price series. This is a helper for research/calibration.
///
/// Returns (min_d, weights_length) tuple.
///
/// # Arguments
/// * `series` - Raw price series
/// * `d_range` - Range of d values to test (e.g. 0.0 to 1.0 in steps of 0.05)
/// * `threshold` - Weight truncation threshold
///
/// Note: This performs a simplified variance ratio stationarity check
/// (full ADF requires matrix operations; use Python scipy for production calibration).
pub fn find_minimum_d(series: &[f64], d_step: f64, threshold: f64) -> Option<(f64, usize)> {
    let max_window = series.len().min(500);
    let mut d = 0.0_f64;

    while d <= 1.0 {
        let fd = FracDiff::new(d, threshold, max_window);
        let diffed = fd.apply_to_series(series);

        if diffed.len() >= 20 {
            // Simple variance ratio test for stationarity
            // Split into halves and check if variance is reasonably stable
            let mid = diffed.len() / 2;
            let var1 = sample_variance(&diffed[..mid]);
            let var2 = sample_variance(&diffed[mid..]);

            if var1 > 1e-12 && var2 > 1e-12 {
                let ratio = (var1 / var2).max(var2 / var1);
                // Variance ratio close to 1 suggests stationarity
                if ratio < 2.0 {
                    // Also check that mean is close to 0
                    let mean: f64 = diffed.iter().sum::<f64>() / diffed.len() as f64;
                    let std_dev = sample_variance(&diffed).sqrt();
                    if std_dev > 1e-12 && (mean / std_dev).abs() < 2.0 {
                        return Some((d, fd.window_len()));
                    }
                }
            }
        }

        d += d_step;
    }

    Some((1.0, FracDiff::new(1.0, threshold, max_window).window_len()))
}

fn sample_variance(data: &[f64]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }
    let n = data.len() as f64;
    let mean = data.iter().sum::<f64>() / n;
    data.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use quant_data::{Bar, Timestamp};

    #[test]
    fn test_fracdiff_weights_d_zero() {
        let weights = compute_fracdiff_weights(0.0, 1e-5, 100);
        // d=0: only w_0=1, all subsequent weights are 0
        assert_eq!(weights.len(), 1);
        assert!((weights[0] - 1.0).abs() < 1e-8);
    }

    #[test]
    fn test_fracdiff_weights_d_one() {
        let weights = compute_fracdiff_weights(1.0, 1e-5, 100);
        // d=1: w_0=1, w_1=-1, all subsequent weights are 0
        assert_eq!(weights.len(), 2);
        assert!((weights[0] - 1.0).abs() < 1e-8);
        assert!((weights[1] + 1.0).abs() < 1e-8);
    }

    #[test]
    fn test_fracdiff_weights_d_half() {
        let weights = compute_fracdiff_weights(0.5, 1e-5, 100);
        // d=0.5: w_0=1, w_1=-0.5, w_2=-0.125, etc. (slowly decaying)
        assert!((weights[0] - 1.0).abs() < 1e-8);
        assert!((weights[1] + 0.5).abs() < 1e-8);
        assert!((weights[2] + 0.125).abs() < 1e-8);
        assert!(
            weights.len() > 10,
            "d=0.5 should have many non-trivial weights"
        );
    }

    #[test]
    fn test_fracdiff_apply_to_series() {
        let fd = FracDiff::new(1.0, 1e-5, 100);
        let series = vec![100.0, 102.0, 101.0, 103.0, 105.0];

        let result = fd.apply_to_series(&series);
        // d=1 is standard differencing: y_t = x_t - x_{t-1}
        assert_eq!(result.len(), 4);
        assert!((result[0] - 2.0).abs() < 1e-8);
        assert!((result[1] + 1.0).abs() < 1e-8);
        assert!((result[2] - 2.0).abs() < 1e-8);
        assert!((result[3] - 2.0).abs() < 1e-8);
    }

    #[test]
    fn test_fracdiff_slice() {
        let fd = FracDiff::new(0.5, 1e-5, 100);
        let series: Vec<f64> = (1..=100).map(|i| (i as f64) * 1.01_f64.powi(i)).collect();

        let result = fd.apply_to_slice(&series);
        assert!(result.is_some());
    }

    #[test]
    fn test_fracdiff_feature_trait() {
        let fd = FracDiff::new(0.5, 1e-3, 50);
        assert_eq!(fd.name(), "fracdiff");
        assert!(fd.lookback() > 1);

        // Build a BarWindow with enough data
        let n = fd.lookback() + 5;
        let mut w = BarWindow::new(n);
        for i in 0..n {
            let price = 100.0 + (i as f64) * 0.5;
            w.push(Bar::same_bar(
                Timestamp(i as i64 * 86_400_000_000_000),
                price,
                price + 1.0,
                price - 1.0,
                price,
                1000,
            ));
        }

        let result = fd.compute(&w);
        assert!(
            result.is_some(),
            "FracDiff should compute with sufficient window"
        );
    }

    #[test]
    fn test_find_minimum_d() {
        // Generate a random-walk-like series that needs some differencing
        let mut series = Vec::with_capacity(200);
        let mut price = 100.0_f64;
        let mut seed = 42u64;
        for _ in 0..200 {
            // Simple LCG pseudo-random
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let r = ((seed >> 33) as f64) / (u32::MAX as f64) - 0.5;
            price *= (1.0 + r * 0.02).max(0.5);
            series.push(price);
        }

        let result = find_minimum_d(&series, 0.1, 1e-4);
        assert!(result.is_some());
        let (d, window_len) = result.unwrap();
        assert!((0.0..=1.0).contains(&d));
        assert!(window_len > 0);
    }
}
