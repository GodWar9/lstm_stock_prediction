"""Unit tests for dataset splits, temporal leakage prevention, and scaling safety."""

import os
import sys
import numpy as np
import pytest

# Ensure root directory is on Python path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "../..")))

from python.ml.dataset import FeatureScaler, PurgedWalkForwardSplitter, create_sliding_windows


def test_purged_walk_forward_splitter_no_overlap():
    n_samples = 1000
    splitter = PurgedWalkForwardSplitter(purge_bars=10, embargo_bars=25)
    train_idx, val_idx, test_idx = splitter.split_train_test(
        n_samples, train_ratio=0.6, val_ratio=0.2
    )

    # Check non-empty
    assert len(train_idx) > 0
    assert len(val_idx) > 0
    assert len(test_idx) > 0

    # Assert mutual exclusivity
    assert len(np.intersect1d(train_idx, val_idx)) == 0
    assert len(np.intersect1d(train_idx, test_idx)) == 0
    assert len(np.intersect1d(val_idx, test_idx)) == 0

    # Assert purge gap between train and val
    train_max = train_idx.max()
    val_min = val_idx.min()
    assert val_min - train_max >= 10 + 25  # purge + embargo gap


def test_sliding_window_shapes_and_temporal_order():
    t_steps = 100
    n_features = 5
    lookback = 15

    features = np.arange(t_steps * n_features, dtype=np.float32).reshape(t_steps, n_features)
    targets = np.arange(t_steps, dtype=np.float32)

    x_win, y_win = create_sliding_windows(features, targets, lookback)

    assert x_win.shape == (t_steps - lookback + 1, lookback, n_features)
    assert y_win.shape == (t_steps - lookback + 1,)

    # Verify last step in window matches target
    for i in range(len(y_win)):
        expected_target = targets[i + lookback - 1]
        assert y_win[i] == expected_target


def test_feature_scaler_no_test_leakage():
    np.random.seed(42)
    train_data = np.random.normal(loc=10.0, scale=2.0, size=(100, 4))
    test_data = np.random.normal(loc=100.0, scale=20.0, size=(50, 4))

    scaler = FeatureScaler()
    scaler.fit(train_data)

    # Scaler mean must match train mean, completely uninfluenced by test_data
    np.testing.assert_allclose(scaler.mean_, np.mean(train_data, axis=0), rtol=1e-5)
    np.testing.assert_allclose(scaler.std_, np.std(train_data, axis=0), rtol=1e-5)

    transformed_train = scaler.transform(train_data)
    np.testing.assert_allclose(np.mean(transformed_train, axis=0), 0.0, atol=1e-6)
