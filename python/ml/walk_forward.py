"""Walk-Forward Cross Validation Engine for Quantitative Finance."""

from typing import Generator, List, Optional, Tuple
import numpy as np


class WalkForwardCV:
    """Expanding and Rolling Walk-Forward Cross Validation splitter with purge & embargo."""

    def __init__(
        self,
        n_splits: int = 5,
        train_size: Optional[int] = None,
        test_size: Optional[int] = None,
        purge_gap: int = 5,
        embargo_gap: int = 20,
        expanding: bool = True,
    ):
        assert n_splits >= 2, "n_splits must be at least 2"
        self.n_splits = n_splits
        self.train_size = train_size
        self.test_size = test_size
        self.purge_gap = max(0, purge_gap)
        self.embargo_gap = max(0, embargo_gap)
        self.expanding = expanding

    def split(self, n_samples: int) -> Generator[Tuple[np.ndarray, np.ndarray], None, None]:
        """Generate (train_indices, test_indices) tuples across walk-forward folds."""
        test_len = self.test_size or (n_samples // (self.n_splits + 1))
        assert test_len > 0, "test_size is too small for dataset size"

        for i in range(self.n_splits):
            test_start = n_samples - (self.n_splits - i) * test_len
            test_end = test_start + test_len

            if test_start <= self.purge_gap:
                continue

            raw_train_end = test_start - self.purge_gap - self.embargo_gap

            if self.expanding:
                train_start = 0
            else:
                train_len = self.train_size or (test_len * 2)
                train_start = max(0, raw_train_end - train_len)

            train_idx = np.arange(train_start, raw_train_end)
            test_idx = np.arange(test_start, test_end)

            if len(train_idx) == 0 or len(test_idx) == 0:
                continue

            # Strict verification of no leakage
            assert train_idx[-1] + self.purge_gap <= test_idx[0], "Temporal leakage detected!"
            assert len(np.intersect1d(train_idx, test_idx)) == 0, "Indices overlap!"

            yield train_idx, test_idx
