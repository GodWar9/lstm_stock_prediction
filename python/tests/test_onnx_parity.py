"""Parity tests verifying PyTorch model output matches ONNX Runtime within strict tolerance."""

import os
import sys
import numpy as np
import onnxruntime as ort
import pytest
import torch

# Ensure root directory is on Python path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "../..")))

from python.ml.export_onnx import export_to_onnx
from python.ml.models.lstm import LSTMForecaster


@pytest.mark.parametrize("batch_size", [1, 4, 16])
@pytest.mark.parametrize("hidden_dim", [32, 64])
def test_pytorch_onnx_numerical_parity(tmp_path, batch_size, hidden_dim):
    input_dim = 6
    seq_len = 15

    model = LSTMForecaster(
        input_dim=input_dim,
        hidden_dim=hidden_dim,
        num_layers=2,
        dropout=0.0,
    )
    model.eval()

    onnx_path = str(tmp_path / f"parity_test_h{hidden_dim}.onnx")
    export_to_onnx(model, input_dim, seq_len, onnx_path)

    # Random test input
    np.random.seed(123)
    dummy_np = np.random.randn(batch_size, seq_len, input_dim).astype(np.float32)
    dummy_torch = torch.from_numpy(dummy_np)

    # PyTorch inference
    with torch.no_grad():
        torch_out = model(dummy_torch).numpy()

    # ONNX Runtime inference
    session = ort.InferenceSession(onnx_path, providers=["CPUExecutionProvider"])
    ort_inputs = {session.get_inputs()[0].name: dummy_np}
    ort_out = session.run(None, ort_inputs)[0]

    # Check shape match
    assert torch_out.shape == ort_out.shape == (batch_size, 1)

    # Numerical parity within 1e-4 absolute tolerance
    np.testing.assert_allclose(
        torch_out,
        ort_out,
        rtol=1e-4,
        atol=1e-4,
        err_msg="PyTorch and ONNX Runtime outputs diverge!",
    )
