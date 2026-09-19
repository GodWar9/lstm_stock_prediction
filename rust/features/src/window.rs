//! Fixed-capacity circular ring buffer for OHLCV bar windows.
//!
//! Features consume a rolling window of bars; this avoids reallocating
//! per timestep and supports O(1) push + O(1) indexed access.

use quant_data::Bar;

/// Ring-buffer backed sliding window of OHLCV bars.
#[derive(Debug, Clone)]
pub struct BarWindow {
    buf: Vec<Option<Bar>>,
    capacity: usize,
    head: usize,
    len: usize,
}

impl BarWindow {
    /// Creates a new empty window with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "BarWindow capacity must be > 0");
        Self {
            buf: (0..capacity).map(|_| None).collect(),
            capacity,
            head: 0,
            len: 0,
        }
    }

    /// Pushes a new bar, evicting the oldest if at capacity.
    pub fn push(&mut self, bar: Bar) {
        self.buf[self.head] = Some(bar);
        self.head = (self.head + 1) % self.capacity;
        if self.len < self.capacity {
            self.len += 1;
        }
    }

    /// Number of bars currently in the window.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the window has no bars.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether the window is full (has `capacity` bars).
    pub fn is_full(&self) -> bool {
        self.len == self.capacity
    }

    /// Maximum number of bars this window can hold.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Access bar by index where 0 = oldest, len-1 = newest.
    pub fn get(&self, index: usize) -> Option<&Bar> {
        if index >= self.len {
            return None;
        }
        let actual = if self.len < self.capacity {
            index
        } else {
            (self.head + index) % self.capacity
        };
        self.buf[actual].as_ref()
    }

    /// Returns the most recently pushed bar.
    pub fn latest(&self) -> Option<&Bar> {
        if self.len == 0 {
            return None;
        }
        let idx = if self.head == 0 {
            self.capacity - 1
        } else {
            self.head - 1
        };
        self.buf[idx].as_ref()
    }

    /// Returns the oldest bar in the window.
    pub fn oldest(&self) -> Option<&Bar> {
        self.get(0)
    }

    /// Iterates over bars from oldest to newest.
    pub fn iter(&self) -> BarWindowIter<'_> {
        BarWindowIter {
            window: self,
            index: 0,
        }
    }

    /// Returns a zero-allocation iterator over close prices from oldest to newest.
    pub fn iter_closes(&self) -> impl ExactSizeIterator<Item = f64> + '_ {
        self.iter().map(|b| b.close)
    }

    /// Fills a caller-provided destination slice with close prices without heap allocations.
    pub fn closes_into(&self, out: &mut [f64]) -> usize {
        let count = self.len.min(out.len());
        for (i, b) in self.iter().take(count).enumerate() {
            out[i] = b.close;
        }
        count
    }

    /// Collects the `close` prices from oldest to newest.
    pub fn closes(&self) -> Vec<f64> {
        self.iter_closes().collect()
    }

    /// Returns a zero-allocation iterator over high prices from oldest to newest.
    pub fn iter_highs(&self) -> impl ExactSizeIterator<Item = f64> + '_ {
        self.iter().map(|b| b.high)
    }

    /// Collects the `high` prices from oldest to newest.
    pub fn highs(&self) -> Vec<f64> {
        self.iter_highs().collect()
    }

    /// Returns a zero-allocation iterator over low prices from oldest to newest.
    pub fn iter_lows(&self) -> impl ExactSizeIterator<Item = f64> + '_ {
        self.iter().map(|b| b.low)
    }

    /// Collects the `low` prices from oldest to newest.
    pub fn lows(&self) -> Vec<f64> {
        self.iter_lows().collect()
    }

    /// Collects the `volume` values from oldest to newest.
    pub fn volumes(&self) -> Vec<u64> {
        self.iter().map(|b| b.volume).collect()
    }
}

/// Iterator over bars in a `BarWindow` from oldest to newest.
pub struct BarWindowIter<'a> {
    window: &'a BarWindow,
    index: usize,
}

impl<'a> Iterator for BarWindowIter<'a> {
    type Item = &'a Bar;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.window.len {
            return None;
        }
        let bar = self.window.get(self.index);
        self.index += 1;
        bar
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.window.len - self.index;
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for BarWindowIter<'a> {}

#[cfg(test)]
mod tests {
    use super::*;
    use quant_data::Timestamp;

    fn make_bar(close: f64) -> Bar {
        Bar::same_bar(
            Timestamp(close as i64 * 1_000_000_000),
            close - 1.0,
            close + 1.0,
            close - 2.0,
            close,
            1000,
        )
    }

    #[test]
    fn test_push_and_access() {
        let mut w = BarWindow::new(3);
        assert!(w.is_empty());
        assert_eq!(w.len(), 0);

        w.push(make_bar(10.0));
        w.push(make_bar(20.0));
        assert_eq!(w.len(), 2);
        assert!(!w.is_full());

        assert_eq!(w.oldest().unwrap().close, 10.0);
        assert_eq!(w.latest().unwrap().close, 20.0);
    }

    #[test]
    fn test_ring_buffer_eviction() {
        let mut w = BarWindow::new(3);
        w.push(make_bar(10.0));
        w.push(make_bar(20.0));
        w.push(make_bar(30.0));
        assert!(w.is_full());
        assert_eq!(w.len(), 3);

        // Push a 4th bar — oldest (10.0) should be evicted
        w.push(make_bar(40.0));
        assert_eq!(w.len(), 3);
        assert_eq!(w.oldest().unwrap().close, 20.0);
        assert_eq!(w.latest().unwrap().close, 40.0);

        let closes = w.closes();
        assert_eq!(closes, vec![20.0, 30.0, 40.0]);
    }

    #[test]
    fn test_iter_oldest_to_newest() {
        let mut w = BarWindow::new(4);
        for i in 1..=6 {
            w.push(make_bar(i as f64 * 10.0));
        }
        // After 6 pushes with cap 4, window should contain [30, 40, 50, 60]
        let closes: Vec<f64> = w.iter().map(|b| b.close).collect();
        assert_eq!(closes, vec![30.0, 40.0, 50.0, 60.0]);
    }
}
