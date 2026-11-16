"""Synthetic negative controls only; these are not strategy performance results."""
from types import SimpleNamespace

import numpy as np
import pytest
import torch
from scipy.stats import pearsonr
from torch.utils.data import DataLoader, TensorDataset

from python.ml import train_orchestrator as orchestrator
from python.ml.dataset import FeatureScaler, PurgedWalkForwardSplitter
from python.ml.models.lstm import LSTMForecaster
from python.ml.trainer import ModelTrainer


def fixture(monkeypatch):
    features = np.arange(1200, dtype=np.float32).reshape(400, 3)
    # Large validation/test shift makes accidental whole-dataset fitting observable.
    features[280:] += 10000
    monkeypatch.setattr(orchestrator, 'load_training_arrays', lambda *args: (
        features, np.zeros(400, dtype=np.float32), ['a', 'b', 'c'],
        np.arange(400), np.arange(400) + 1, 1, 1, None, {},
    ))
    cfg = {'features': {'lookback': 4}, 'training': {'purge_gap': 3, 'embargo_gap': 4}}
    return features, cfg


def test_orchestrator_rejects_purge_embargo_violation(monkeypatch, tmp_path):
    _, cfg = fixture(monkeypatch)
    # No label overlap, but fewer unused rows than the declared purge+embargo.
    args = SimpleNamespace(walk_forward=False, _indices=(
        np.arange(100), np.arange(102, 200), np.arange(210, 400)))
    def must_not_fit(*args):
        pytest.fail('Invalid fold reached scaler fitting')
    monkeypatch.setattr(FeatureScaler, 'fit_transform', must_not_fit)
    with pytest.raises(ValueError, match='purge and embargo'):
        orchestrator.train_and_export(args, cfg, 'canary', tmp_path)


def test_orchestrator_scaler_sees_only_train_fold(monkeypatch, tmp_path):
    features, cfg = fixture(monkeypatch)
    train, _, _ = PurgedWalkForwardSplitter(3, 4).split_train_test(400, .7, .15)
    original = FeatureScaler.fit
    class StopAfterScaling(Exception):
        pass
    def check_fit(self, values):
        original(self, values)
        np.testing.assert_array_equal(values, features[train])
        np.testing.assert_allclose(self.mean_, features[train].mean(axis=0))
        raise StopAfterScaling()
    monkeypatch.setattr(FeatureScaler, 'fit', check_fit)
    with pytest.raises(StopAfterScaling):
        orchestrator.train_and_export(SimpleNamespace(walk_forward=False), cfg, 'canary', tmp_path)


def test_shuffled_labels_have_no_heldout_ic():
    # Frozen negative-control protocol: independent sequences, fixed seed, no tuning.
    # Both train and heldout labels are permuted independently before any fitting.
    # This is a synthetic test fixture, not the real study's final holdout.
    previous_threads = torch.get_num_threads()
    torch.set_num_threads(1)
    try:
        torch.manual_seed(819)
        rng = np.random.default_rng(819)
        x = rng.normal(size=(3072, 4, 3)).astype(np.float32)
        target = x[:, -1, 0].copy()
        train_y = rng.permutation(target[:1024])
        test_y = rng.permutation(target[1024:])
        model = LSTMForecaster(3, hidden_dim=8, num_layers=1, dropout=0)
        trainer = ModelTrainer(model, torch.optim.Adam(model.parameters(), lr=.01),
                               torch.nn.MSELoss(), device=torch.device('cpu'))
        loader = DataLoader(TensorDataset(torch.from_numpy(x[:1024]),
                                         torch.from_numpy(train_y)), batch_size=128, shuffle=False)
        for _ in range(40):
            trainer.train_epoch(loader)
        model.eval()
        with torch.no_grad():
            prediction = model(torch.from_numpy(x[1024:])).numpy().ravel()
        result = pearsonr(prediction, test_y)
        interval = result.confidence_interval(confidence_level=.99)
        print(f'SYNTHETIC CANARY ONLY: IC={result.statistic}; p={result.pvalue}; CI99={interval}')
        assert np.isfinite(result.statistic)
        assert interval.low <= 0 <= interval.high
        assert abs(result.statistic) < .10
    finally:
        torch.set_num_threads(previous_threads)
