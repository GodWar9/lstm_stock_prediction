"""TimeSeriesDataset and point-in-time purged/embargoed split utilities."""

from typing import Dict, List, Optional, Tuple
import numpy as np
import pandas as pd


class PurgedWalkForwardSplitter:
    """Purged and embargoed cross-validation splitter for financial time series.
    
    Prevents information leakage between train and test splits:
    - Purging: Removes training observations whose label horizon overlaps with test start.
    - Embargo: Removes training observations immediately following test periods to account
      for memory/autocorrelation in features.
    """

    def __init__(self, purge_bars: int = 5, embargo_bars: int = 20):
        self.purge_bars = max(0, purge_bars)
        self.embargo_bars = max(0, embargo_bars)

    def split_train_test(
        self,
        n_samples: int,
        train_ratio: float = 0.7,
        val_ratio: float = 0.15,
    ) -> Tuple[np.ndarray, np.ndarray, np.ndarray]:
        """Generate (train_indices, val_indices, test_indices) with purge and embargo."""
        assert 0.0 < train_ratio < 1.0
        assert 0.0 <= val_ratio < 1.0
        assert train_ratio + val_ratio < 1.0

        n_train_raw = int(n_samples * train_ratio)
        n_val_raw = int(n_samples * val_ratio)

        # Train range: [0, n_train_raw - purge_bars]
        train_end = max(1, n_train_raw - self.purge_bars)
        train_idx = np.arange(0, train_end)

        # Val range: [n_train_raw + embargo_bars, n_train_raw + n_val_raw - purge_bars]
        val_start = n_train_raw + self.embargo_bars
        val_end = max(val_start + 1, n_train_raw + n_val_raw - self.purge_bars)
        if val_start < n_samples and val_start < val_end:
            val_idx = np.arange(val_start, min(val_end, n_samples))
        else:
            val_idx = np.array([], dtype=int)

        # Test range: [n_train_raw + n_val_raw + embargo_bars, n_samples]
        test_start = n_train_raw + n_val_raw + self.embargo_bars
        if test_start < n_samples:
            test_idx = np.arange(test_start, n_samples)
        else:
            test_idx = np.array([], dtype=int)

        # Assert no overlap
        assert len(np.intersect1d(train_idx, val_idx)) == 0, "Train and Val indices overlap!"
        assert len(np.intersect1d(train_idx, test_idx)) == 0, "Train and Test indices overlap!"
        assert len(np.intersect1d(val_idx, test_idx)) == 0, "Val and Test indices overlap!"

        return train_idx, val_idx, test_idx


def create_sliding_windows(
    features: np.ndarray,
    targets: np.ndarray,
    lookback: int,
) -> Tuple[np.ndarray, np.ndarray]:
    """Convert sequential features and targets into fixed-length sliding windows.
    
    Args:
        features: 2D array of shape [T, F]
        targets: 1D array of shape [T]
        lookback: sequence length L
        
    Returns:
        X: shape [N, L, F] where N = T - L + 1
        y: shape [N] corresponding to target at the end of each window
    """
    assert len(features) == len(targets), "Features and targets must have same length"
    t_steps, num_features = features.shape
    if t_steps < lookback:
        return np.empty((0, lookback, num_features)), np.empty((0,))

    n_windows = t_steps - lookback + 1
    x_windows = np.zeros((n_windows, lookback, num_features), dtype=np.float32)
    y_windows = np.zeros((n_windows,), dtype=np.float32)

    for i in range(n_windows):
        x_windows[i] = features[i : i + lookback]
        y_windows[i] = targets[i + lookback - 1]

    return x_windows, y_windows


class FeatureScaler:
    """Standard scaler (z-score) fitted exclusively on train split to prevent lookahead."""

    def __init__(self):
        self.mean_: Optional[np.ndarray] = None
        self.std_: Optional[np.ndarray] = None

    def fit(self, x: np.ndarray) -> "FeatureScaler":
        flat_x = x.reshape(-1, x.shape[-1])
        self.mean_ = np.mean(flat_x, axis=0)
        self.std_ = np.std(flat_x, axis=0)
        self.std_[self.std_ < 1e-8] = 1.0  # Prevent divide by zero
        return self

    def transform(self, x: np.ndarray) -> np.ndarray:
        assert self.mean_ is not None and self.std_ is not None, "Scaler not fitted"
        return (x - self.mean_) / self.std_

    def fit_transform(self, x: np.ndarray) -> np.ndarray:
        return self.fit(x).transform(x)


try:
    import torch
    from torch.utils.data import Dataset

    class TimeSeriesTorchDataset(Dataset):
        """PyTorch Dataset yielding (X_window, y_target) tensors."""

        def __init__(self, x_windows: np.ndarray, y_windows: np.ndarray):
            self.x = torch.from_numpy(x_windows.astype(np.float32))
            self.y = torch.from_numpy(y_windows.astype(np.float32))

        def __len__(self) -> int:
            return len(self.x)

        def __getitem__(self, idx: int) -> Tuple[torch.Tensor, torch.Tensor]:
            return self.x[idx], self.y[idx]

except ImportError:
    pass

