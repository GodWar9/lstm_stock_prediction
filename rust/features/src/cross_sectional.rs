//! Cross-sectional Z-Score normalization across a universe of instruments at time t.
//!
//! This module provides utilities for cross-sectionally normalizing feature values
//! to eliminate systematic market drift and focus on idiosyncratic alpha signals.
//! The normalization transforms raw feature values into z-scores relative to the
//! cross-section at each point in time: z_{i,t} = (x_{i,t} - mu_t) / sigma_t

use std::collections::HashMap;

/// Result of cross-sectional normalization at a single point in time.
#[derive(Debug, Clone, PartialEq)]
pub struct CrossSectionalSnapshot {
    /// Cross-sectional mean of the raw feature values.
    pub mean: f64,
    /// Cross-sectional standard deviation.
    pub std_dev: f64,
    /// Normalized z-scores keyed by instrument identifier.
    pub z_scores: HashMap<u32, f64>,
    /// Number of universe members in the cross-section.
    pub universe_size: usize,
}

/// Normalizes a cross-section of feature values across universe members at a single point in time.
///
/// Uses Welford's online algorithm for numerically stable single-pass mean/variance computation.
///
/// # Arguments
/// * `raw_values` - Iterator of (instrument_id, raw_feature_value) pairs
///
/// # Returns
/// `None` if fewer than 2 instruments provide values (can't compute std dev).
pub fn normalize_cross_section(raw_values: &[(u32, f64)]) -> Option<CrossSectionalSnapshot> {
    let n = raw_values.len();
    if n < 2 {
        return None;
    }

    // Welford single-pass online accumulation
    let mut mean = 0.0_f64;
    let mut m2 = 0.0_f64;

    for (k, &(_, val)) in raw_values.iter().enumerate() {
        let delta = val - mean;
        mean += delta / (k as f64 + 1.0);
        let delta2 = val - mean;
        m2 += delta * delta2;
    }

    let variance = m2 / (n as f64 - 1.0);
    let std_dev = variance.sqrt();
    let inv_std = if std_dev > 1e-12 { 1.0 / std_dev } else { 0.0 };

    let mut z_scores = HashMap::with_capacity(n);
    for &(inst_id, val) in raw_values {
        let z = (val - mean) * inv_std;
        z_scores.insert(inst_id, z);
    }

    Some(CrossSectionalSnapshot {
        mean,
        std_dev,
        z_scores,
        universe_size: n,
    })
}

/// Cross-Sectional Normalizer: accumulates feature values across instruments
/// and computes z-scores for an entire universe at each timestep.
///
/// Supports multiple features simultaneously.
#[derive(Debug, Clone)]
pub struct CrossSectionalNormalizer {
    /// Feature name → Vec<(instrument_id, raw_value)>
    buffer: HashMap<String, Vec<(u32, f64)>>,
}

impl CrossSectionalNormalizer {
    pub fn new() -> Self {
        Self {
            buffer: HashMap::new(),
        }
    }

    /// Register a raw feature value for a specific instrument in the current cross-section.
    pub fn submit(&mut self, feature_name: &str, instrument_id: u32, value: f64) {
        self.buffer
            .entry(feature_name.to_string())
            .or_default()
            .push((instrument_id, value));
    }

    /// Compute cross-sectional z-scores for all accumulated features and clear buffers.
    ///
    /// Returns a map of feature_name → CrossSectionalSnapshot.
    pub fn normalize_and_flush(&mut self) -> HashMap<String, CrossSectionalSnapshot> {
        let mut results = HashMap::with_capacity(self.buffer.len());

        for (feature_name, values) in self.buffer.drain() {
            if let Some(snapshot) = normalize_cross_section(&values) {
                results.insert(feature_name, snapshot);
            }
        }

        results
    }

    /// Clear all accumulated feature values without computing z-scores.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Number of features currently buffered.
    pub fn feature_count(&self) -> usize {
        self.buffer.len()
    }
}

impl Default for CrossSectionalNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Winsorized cross-sectional normalization: clips extreme z-scores to `[-max_z, +max_z]`
/// before returning, preventing a single outlier from dominating the cross-section.
pub fn normalize_cross_section_winsorized(
    raw_values: &[(u32, f64)],
    max_z: f64,
) -> Option<CrossSectionalSnapshot> {
    let mut snapshot = normalize_cross_section(raw_values)?;
    for z in snapshot.z_scores.values_mut() {
        *z = z.clamp(-max_z, max_z);
    }
    Some(snapshot)
}

/// Rank-based cross-sectional normalization: maps feature values to
/// uniform percentile ranks in [0, 1], then transforms to standard normal
/// z-scores via the inverse CDF approximation.
pub fn normalize_cross_section_rank(raw_values: &[(u32, f64)]) -> Option<CrossSectionalSnapshot> {
    let n = raw_values.len();
    if n < 2 {
        return None;
    }

    // Sort by value for ranking
    let mut sorted: Vec<(usize, u32, f64)> = raw_values
        .iter()
        .enumerate()
        .map(|(i, &(id, v))| (i, id, v))
        .collect();
    sorted.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

    // Assign fractional ranks (midpoint for ties)
    let mut ranks = vec![0.0_f64; n];
    let mut i = 0;
    while i < n {
        let mut j = i;
        while j < n - 1 && (sorted[j + 1].2 - sorted[j].2).abs() < 1e-12 {
            j += 1;
        }
        let avg_rank = (i + j) as f64 / 2.0 + 1.0;
        for k in i..=j {
            ranks[sorted[k].0] = avg_rank;
        }
        i = j + 1;
    }

    // Convert ranks to uniform percentile then to z-score via Beasley-Springer-Moro approximation
    let mut z_scores = HashMap::with_capacity(n);
    let mut sum_z = 0.0_f64;
    let mut sum_z2 = 0.0_f64;

    for (idx, &(id, _)) in raw_values.iter().enumerate() {
        let u = ranks[idx] / (n as f64 + 1.0); // Uniform percentile ∈ (0, 1)
        let z = inverse_normal_cdf(u);
        z_scores.insert(id, z);
        sum_z += z;
        sum_z2 += z * z;
    }

    let mean = sum_z / n as f64;
    let variance = if n > 1 {
        (sum_z2 - n as f64 * mean * mean) / (n as f64 - 1.0)
    } else {
        0.0
    };

    Some(CrossSectionalSnapshot {
        mean,
        std_dev: variance.sqrt(),
        z_scores,
        universe_size: n,
    })
}

/// Rational approximation of the inverse standard normal CDF (probit function).
/// Beasley-Springer-Moro algorithm. Accurate to ~1e-8 for p ∈ (0.00003, 0.99997).
fn inverse_normal_cdf(p: f64) -> f64 {
    let p = p.clamp(1e-8, 1.0 - 1e-8);

    // Rational approximation coefficients
    const A: [f64; 4] = [
        2.50662823884,
        -18.61500062529,
        41.39119773534,
        -25.44106049637,
    ];
    const B: [f64; 4] = [
        -8.47351093090,
        23.08336743743,
        -21.06224101826,
        3.13082909833,
    ];

    let y = p - 0.5;
    if y.abs() < 0.42 {
        let r = y * y;
        y * (((A[3] * r + A[2]) * r + A[1]) * r + A[0])
            / ((((B[3] * r + B[2]) * r + B[1]) * r + B[0]) * r + 1.0)
    } else {
        let r = if y > 0.0 { 1.0 - p } else { p };
        let r_sqrt = (-2.0 * r.ln()).sqrt();
        let mut result = r_sqrt
            - (2.515517 + 0.802853 * r_sqrt + 0.010328 * r_sqrt * r_sqrt)
                / (1.0
                    + 1.432788 * r_sqrt
                    + 0.189269 * r_sqrt * r_sqrt
                    + 0.001308 * r_sqrt * r_sqrt * r_sqrt);
        if y < 0.0 {
            result = -result;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_cross_section_basic() {
        let values = vec![(1, 10.0), (2, 20.0), (3, 30.0), (4, 40.0), (5, 50.0)];

        let snapshot = normalize_cross_section(&values).unwrap();
        assert_eq!(snapshot.universe_size, 5);
        assert!((snapshot.mean - 30.0).abs() < 1e-8);

        // Z-scores should sum to approximately 0
        let z_sum: f64 = snapshot.z_scores.values().sum();
        assert!(
            z_sum.abs() < 1e-8,
            "z-scores should sum to ~0, got {}",
            z_sum
        );

        // Instrument 1 (value=10) should have negative z-score
        assert!(*snapshot.z_scores.get(&1).unwrap() < 0.0);
        // Instrument 5 (value=50) should have positive z-score
        assert!(*snapshot.z_scores.get(&5).unwrap() > 0.0);
    }

    #[test]
    fn test_normalize_cross_section_too_few_instruments() {
        let values = vec![(1, 10.0)];
        assert!(normalize_cross_section(&values).is_none());
    }

    #[test]
    fn test_winsorized_clips_extremes() {
        let values = vec![
            (1, 1.0),
            (2, 2.0),
            (3, 3.0),
            (4, 100.0), // Extreme outlier
        ];

        let snapshot = normalize_cross_section_winsorized(&values, 2.0).unwrap();
        for z in snapshot.z_scores.values() {
            assert!(
                *z >= -2.0 - 1e-8 && *z <= 2.0 + 1e-8,
                "z-score {} exceeds max_z=2.0",
                z
            );
        }
    }

    #[test]
    fn test_rank_normalization() {
        let values = vec![(1, 100.0), (2, 50.0), (3, 75.0), (4, 200.0), (5, 25.0)];

        let snapshot = normalize_cross_section_rank(&values).unwrap();
        assert_eq!(snapshot.universe_size, 5);

        // Instrument 5 (lowest value=25) should have the most negative z-score
        // Instrument 4 (highest value=200) should have the most positive z-score
        let z5 = *snapshot.z_scores.get(&5).unwrap();
        let z4 = *snapshot.z_scores.get(&4).unwrap();
        assert!(
            z5 < z4,
            "lowest value should have lower z-score than highest"
        );
    }

    #[test]
    fn test_normalizer_submit_and_flush() {
        let mut normalizer = CrossSectionalNormalizer::new();

        // Submit RSI values for 4 instruments
        normalizer.submit("rsi", 1, 30.0);
        normalizer.submit("rsi", 2, 50.0);
        normalizer.submit("rsi", 3, 70.0);
        normalizer.submit("rsi", 4, 45.0);

        // Submit SMA values
        normalizer.submit("sma", 1, 100.0);
        normalizer.submit("sma", 2, 105.0);
        normalizer.submit("sma", 3, 98.0);
        normalizer.submit("sma", 4, 102.0);

        assert_eq!(normalizer.feature_count(), 2);

        let results = normalizer.normalize_and_flush();
        assert_eq!(results.len(), 2);
        assert!(results.contains_key("rsi"));
        assert!(results.contains_key("sma"));

        // After flush, normalizer should be empty
        assert_eq!(normalizer.feature_count(), 0);
    }

    #[test]
    fn test_inverse_normal_cdf_symmetry() {
        let z_50 = inverse_normal_cdf(0.5);
        assert!(z_50.abs() < 0.01, "Phi^-1(0.5) should be ~0, got {}", z_50);

        let z_95 = inverse_normal_cdf(0.95);
        assert!(
            (z_95 - 1.645).abs() < 0.05,
            "Phi^-1(0.95) should be ~1.645, got {}",
            z_95
        );
    }
}
