//! Thread-local scratchpad allocator for zero-allocation hot paths.
//!
//! Provides re-usable per-thread scratch buffers that grow to high-water-mark
//! capacity and never deallocate, eliminating repeated `Vec` allocations in
//! tight inner loops (execution fills, portfolio construction steps).

use std::cell::RefCell;

/// Thread-local reusable `f64` scratch buffer.
///
/// Each thread gets its own buffer that grows as needed but is never freed,
/// amortizing allocation cost to zero after warmup.
///
/// # Example
/// ```
/// use quant_execution::scratch::with_f64_scratch;
///
/// let result = with_f64_scratch(100, |buf| {
///     buf.extend((0..100).map(|i| i as f64));
///     buf.iter().sum::<f64>()
/// });
/// assert!((result - 4950.0).abs() < 1e-10);
/// ```
pub fn with_f64_scratch<F, R>(min_capacity: usize, f: F) -> R
where
    F: FnOnce(&mut Vec<f64>) -> R,
{
    thread_local! {
        static SCRATCH: RefCell<Vec<f64>> = const { RefCell::new(Vec::new()) };
    }

    SCRATCH.with(|cell| {
        let mut buf = cell.borrow_mut();
        buf.clear();
        if buf.capacity() < min_capacity {
            buf.reserve(min_capacity);
        }
        f(&mut buf)
    })
}

/// Thread-local reusable scratch buffer for `(f64, f64, f64)` tuples.
///
/// Used by portfolio constructors to avoid per-step allocations
/// when assembling (instrument_index, symbol_hash, weight) triples.
pub fn with_weight_scratch<F, R>(min_capacity: usize, f: F) -> R
where
    F: FnOnce(&mut Vec<(usize, f64)>) -> R,
{
    thread_local! {
        static WEIGHT_SCRATCH: RefCell<Vec<(usize, f64)>> = const { RefCell::new(Vec::new()) };
    }

    WEIGHT_SCRATCH.with(|cell| {
        let mut buf = cell.borrow_mut();
        buf.clear();
        if buf.capacity() < min_capacity {
            buf.reserve(min_capacity);
        }
        f(&mut buf)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scratch_reuse_no_realloc() {
        // First call: buffer allocated
        with_f64_scratch(1000, |buf| {
            buf.extend((0..1000).map(|i| i as f64));
            assert_eq!(buf.len(), 1000);
        });

        // Second call: buffer reused (capacity retained, length cleared)
        with_f64_scratch(500, |buf| {
            assert!(buf.is_empty());
            assert!(buf.capacity() >= 1000); // High-water mark retained
            buf.extend((0..500).map(|i| i as f64));
            assert_eq!(buf.len(), 500);
        });
    }

    #[test]
    fn test_scratch_grows_on_demand() {
        with_f64_scratch(10, |buf| {
            buf.extend((0..10).map(|i| i as f64));
        });

        // Request more than previous high-water mark
        with_f64_scratch(5000, |buf| {
            assert!(buf.capacity() >= 5000);
            buf.extend((0..5000).map(|i| i as f64));
            let sum: f64 = buf.iter().sum();
            assert!((sum - 12497500.0).abs() < 1e-6);
        });
    }

    #[test]
    fn test_weight_scratch() {
        with_weight_scratch(50, |buf| {
            for i in 0..50 {
                buf.push((i, i as f64 * 0.01));
            }
            assert_eq!(buf.len(), 50);
        });

        // Reuse
        with_weight_scratch(10, |buf| {
            assert!(buf.is_empty());
            assert!(buf.capacity() >= 50);
        });
    }
}
