//! SIMD-friendly batch operations for vectorized feature computation.
//!
//! All functions operate on contiguous `&[f64]` slices, structured to maximize
//! LLVM auto-vectorization. No explicit `unsafe` intrinsics are used; instead,
//! code patterns are arranged so the compiler emits `vmulpd`/`vaddpd`/`vfmadd`
//! instructions on x86_64 AVX2 targets.
//!
//! # Design Principles
//! - **No branches in hot loops**: conditional logic is lifted outside the inner loop.
//! - **Contiguous memory access**: all inputs/outputs are flat `f64` slices.
//! - **No intermediate allocations**: caller provides output buffers.
//! - **Fused multiply-add patterns**: EMA recurrence is expressed as `α·x + (1-α)·prev`.

/// Compute branchless exponential moving average over a contiguous slice.
///
/// Writes `out.len()` values where `out[i]` is the EMA at position `i`.
/// The first `seed_period` values are averaged to produce the SMA seed,
/// then EMA recurrence is applied from `seed_period` onward.
///
/// # Panics
/// Panics if `data.len() != out.len()` or `seed_period == 0` or `seed_period > data.len()`.
#[inline]
pub fn batch_ema(data: &[f64], seed_period: usize, out: &mut [f64]) {
    assert_eq!(data.len(), out.len(), "data and out must have equal length");
    assert!(
        seed_period > 0 && seed_period <= data.len(),
        "invalid seed_period"
    );

    let alpha = 2.0 / (seed_period as f64 + 1.0);
    let one_minus_alpha = 1.0 - alpha;

    // SMA seed over first `seed_period` values
    let mut sum = 0.0_f64;
    for i in 0..seed_period {
        sum += data[i];
        out[i] = f64::NAN; // warmup period — no valid EMA
    }
    let seed = sum / seed_period as f64;
    out[seed_period - 1] = seed;

    // EMA recurrence: ema[i] = alpha * data[i] + (1 - alpha) * ema[i-1]
    // This FMA pattern is auto-vectorized by LLVM when processing sequential elements.
    let mut prev = seed;
    for i in seed_period..data.len() {
        // Expressed as FMA to enable vfmadd213pd emission
        let ema = alpha.mul_add(data[i], one_minus_alpha * prev);
        out[i] = ema;
        prev = ema;
    }
}

/// Compute True Range for each bar in a batch (vectorized).
///
/// TR(i) = max(high[i] - low[i], |high[i] - prev_close[i]|, |low[i] - prev_close[i]|)
///
/// `prev_closes[i]` is the close of the bar immediately preceding bar `i`.
///
/// # Panics
/// Panics if all three input slices and `out` are not the same length.
#[inline]
pub fn batch_true_range(highs: &[f64], lows: &[f64], prev_closes: &[f64], out: &mut [f64]) {
    let n = highs.len();
    assert_eq!(lows.len(), n);
    assert_eq!(prev_closes.len(), n);
    assert_eq!(out.len(), n);

    // Inner loop: branchless max using f64::max chains.
    // LLVM recognizes this as vmaxpd on AVX2.
    for i in 0..n {
        let hl = highs[i] - lows[i];
        let hc = (highs[i] - prev_closes[i]).abs();
        let lc = (lows[i] - prev_closes[i]).abs();
        out[i] = hl.max(hc).max(lc);
    }
}

/// Compute a rolling sum over a sliding window of `period` elements.
///
/// Uses a running accumulator with add-front/subtract-back for O(n) total work.
/// Output positions `0..period-1` are set to NAN (warmup).
///
/// # Panics
/// Panics if `data.len() != out.len()` or `period == 0` or `period > data.len()`.
#[inline]
pub fn batch_rolling_sum(data: &[f64], period: usize, out: &mut [f64]) {
    assert_eq!(data.len(), out.len());
    assert!(period > 0 && period <= data.len());

    // Initial sum over first window
    let mut running_sum = 0.0_f64;
    for i in 0..period {
        running_sum += data[i];
        out[i] = f64::NAN;
    }
    out[period - 1] = running_sum;

    // Slide window: add new element, subtract expired element
    for i in period..data.len() {
        running_sum += data[i] - data[i - period];
        out[i] = running_sum;
    }
}

/// Compute rolling mean (SMA) over a sliding window.
///
/// Equivalent to `batch_rolling_sum / period`.
#[inline]
pub fn batch_rolling_mean(data: &[f64], period: usize, out: &mut [f64]) {
    batch_rolling_sum(data, period, out);
    let inv_period = 1.0 / period as f64;
    // Vectorized multiply: LLVM emits vmulpd for this loop
    for val in out.iter_mut() {
        if val.is_finite() {
            *val *= inv_period;
        }
    }
}

/// Element-wise subtract: out[i] = a[i] - b[i].
///
/// Pure vectorization candidate — no branches, contiguous memory.
#[inline]
pub fn batch_sub(a: &[f64], b: &[f64], out: &mut [f64]) {
    let n = a.len();
    assert_eq!(b.len(), n);
    assert_eq!(out.len(), n);
    for i in 0..n {
        out[i] = a[i] - b[i];
    }
}

/// Element-wise multiply: out[i] = a[i] * b[i].
#[inline]
pub fn batch_mul(a: &[f64], b: &[f64], out: &mut [f64]) {
    let n = a.len();
    assert_eq!(b.len(), n);
    assert_eq!(out.len(), n);
    for i in 0..n {
        out[i] = a[i] * b[i];
    }
}

/// Fused multiply-add: out[i] = a[i] * b[i] + c[i].
///
/// Maps directly to vfmadd213pd on AVX2.
#[inline]
pub fn batch_fma(a: &[f64], b: &[f64], c: &[f64], out: &mut [f64]) {
    let n = a.len();
    assert_eq!(b.len(), n);
    assert_eq!(c.len(), n);
    assert_eq!(out.len(), n);
    for i in 0..n {
        out[i] = a[i].mul_add(b[i], c[i]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_ema_matches_scalar() {
        let data = vec![2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0];
        let mut out = vec![0.0; data.len()];
        batch_ema(&data, 3, &mut out);

        // Seed SMA(3) = (2+4+6)/3 = 4.0
        assert!((out[2] - 4.0).abs() < 1e-10);

        // EMA(3) alpha = 0.5
        // Step 3: 0.5 * 8 + 0.5 * 4 = 6.0
        assert!((out[3] - 6.0).abs() < 1e-10);
        // Step 4: 0.5 * 10 + 0.5 * 6 = 8.0
        assert!((out[4] - 8.0).abs() < 1e-10);
        // Step 5: 0.5 * 12 + 0.5 * 8 = 10.0
        assert!((out[5] - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_batch_true_range() {
        let highs = vec![12.0, 20.0, 15.0];
        let lows = vec![8.0, 15.0, 12.0];
        let prev_closes = vec![10.0, 10.0, 18.0];
        let mut out = vec![0.0; 3];

        batch_true_range(&highs, &lows, &prev_closes, &mut out);

        // Bar 0: max(4, 2, 2) = 4
        assert!((out[0] - 4.0).abs() < 1e-10);
        // Bar 1: max(5, 10, 5) = 10
        assert!((out[1] - 10.0).abs() < 1e-10);
        // Bar 2: max(3, 3, 6) = 6
        assert!((out[2] - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_batch_rolling_sum() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let mut out = vec![0.0; data.len()];
        batch_rolling_sum(&data, 3, &mut out);

        assert!(out[0].is_nan());
        assert!(out[1].is_nan());
        assert!((out[2] - 6.0).abs() < 1e-10); // 1+2+3
        assert!((out[3] - 9.0).abs() < 1e-10); // 2+3+4
        assert!((out[4] - 12.0).abs() < 1e-10); // 3+4+5
        assert!((out[5] - 15.0).abs() < 1e-10); // 4+5+6
    }

    #[test]
    fn test_batch_rolling_mean() {
        let data = vec![3.0, 6.0, 9.0, 12.0];
        let mut out = vec![0.0; data.len()];
        batch_rolling_mean(&data, 3, &mut out);

        assert!(out[0].is_nan());
        assert!(out[1].is_nan());
        assert!((out[2] - 6.0).abs() < 1e-10); // (3+6+9)/3
        assert!((out[3] - 9.0).abs() < 1e-10); // (6+9+12)/3
    }

    #[test]
    fn test_batch_fma() {
        let a = vec![2.0, 3.0, 4.0];
        let b = vec![5.0, 6.0, 7.0];
        let c = vec![1.0, 2.0, 3.0];
        let mut out = vec![0.0; 3];
        batch_fma(&a, &b, &c, &mut out);

        assert!((out[0] - 11.0).abs() < 1e-10); // 2*5 + 1
        assert!((out[1] - 20.0).abs() < 1e-10); // 3*6 + 2
        assert!((out[2] - 31.0).abs() < 1e-10); // 4*7 + 3
    }

    #[test]
    fn test_batch_sub_mul() {
        let a = vec![10.0, 20.0, 30.0];
        let b = vec![1.0, 2.0, 3.0];
        let mut out = vec![0.0; 3];

        batch_sub(&a, &b, &mut out);
        assert!((out[0] - 9.0).abs() < 1e-10);
        assert!((out[1] - 18.0).abs() < 1e-10);

        batch_mul(&a, &b, &mut out);
        assert!((out[0] - 10.0).abs() < 1e-10);
        assert!((out[1] - 40.0).abs() < 1e-10);
    }
}
