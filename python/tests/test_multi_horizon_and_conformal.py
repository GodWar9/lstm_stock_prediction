"""Unit tests for Multi-Horizon LSTM forecasting, loss, and conformal prediction."""

import tempfile
from pathlib import Path
import numpy as np
import pytest
import torch

from python.ml.models.lstm import MultiHorizonLSTMForecaster
from python.ml.loss import MultiHorizonLoss, DirectionalAsymmetricLoss
from python.ml.conformal import ConformalPredictor


def test_multi_horizon_forecaster_shapes():
    batch_size = 8
    seq_len = 30
    input_dim = 10
    horizons = (1, 5, 20)

    model = MultiHorizonLSTMForecaster(
        input_dim=input_dim,
        horizons=horizons,
        hidden_dim=64,
        num_layers=2,
        dropout=0.1,
    )
    model.eval()

    x = torch.randn(batch_size, seq_len, input_dim)
    with torch.no_grad():
        out = model(x)
        assert out.shape == (batch_size, 3)

        out_dict = model(x, return_dict=True)
        assert isinstance(out_dict, dict)
        assert set(out_dict.keys()) == {1, 5, 20}
        for h in horizons:
            assert out_dict[h].shape == (batch_size, 1)


def test_multi_horizon_loss():
    batch_size = 16
    y_pred = torch.randn(batch_size, 3, requires_grad=True)
    y_true = torch.randn(batch_size, 3)

    criterion = MultiHorizonLoss(
        base_criterion=DirectionalAsymmetricLoss(alpha=2.0),
        weights=[1.0, 0.5, 0.25],
    )
    loss = criterion(y_pred, y_true)
    assert loss.dim() == 0
    assert loss.item() > 0.0

    loss.backward()
    assert y_pred.grad is not None
    assert torch.all(torch.isfinite(y_pred.grad))


def test_conformal_predictor_coverage_and_intervals():
    np.random.seed(42)
    n = 200
    y_true = np.random.normal(0.01, 0.02, size=n)
    # Add noise to simulate predictions
    y_pred = y_true + np.random.normal(0.0, 0.005, size=n)

    # Split calibration and test
    cal_true, test_true = y_true[:100], y_true[100:]
    cal_pred, test_pred = y_pred[:100], y_pred[100:]

    predictor = ConformalPredictor(alpha=0.10, adaptive=False)
    q = predictor.calibrate(cal_true, cal_pred)
    assert q > 0.0

    lower, upper = predictor.predict_intervals(test_pred)
    assert len(lower) == len(test_pred)
    assert np.all(upper >= lower)

    eval_res = predictor.evaluate_coverage(test_true, test_pred)
    assert eval_res["empirical_coverage"] >= 0.85  # Finite-sample tolerance

    # Confidence metric
    conf = predictor.compute_signal_confidence(test_pred)
    assert len(conf) == len(test_pred)
    assert np.all((conf >= 0.0) & (conf <= 1.0))

    # Test artifact save & load
    with tempfile.TemporaryDirectory() as tmpdir:
        art_path = Path(tmpdir) / "conformal_cal.json"
        predictor.save_artifact(art_path)
        assert art_path.exists()

        loaded = ConformalPredictor.load_artifact(art_path)
        assert loaded.calibrated_quantile == pytest.approx(predictor.calibrated_quantile)
        loaded_lower, loaded_upper = loaded.predict_intervals(test_pred)
        np.testing.assert_allclose(lower, loaded_lower)
        np.testing.assert_allclose(upper, loaded_upper)
