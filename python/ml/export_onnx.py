"""Export PyTorch LSTM models to validated ONNX artifacts with dynamic axes."""

from typing import Dict, Optional
import os
import sys

# Ensure root directory is on Python path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "../..")))
if hasattr(sys.stdout, "reconfigure"):
    try:
        sys.stdout.reconfigure(encoding="utf-8")
        sys.stderr.reconfigure(encoding="utf-8")
    except Exception:
        pass

import onnx
import onnxruntime as ort
import torch
import torch.nn as nn
from python.ml.models.lstm import LSTMForecaster


def export_to_onnx(
    model: nn.Module,
    input_dim: int,
    seq_len: int,
    output_path: str,
    model_id: str = "lstm_v1",
    opset_version: int = 17,
) -> str:
    """Export PyTorch model to ONNX format with validation and dynamic batch size."""
    os.makedirs(os.path.dirname(output_path), exist_ok=True)
    model.eval()

    dummy_input = torch.randn(1, seq_len, input_dim, dtype=torch.float32)

    torch.onnx.export(
        model,
        dummy_input,
        output_path,
        export_params=True,
        opset_version=opset_version,
        do_constant_folding=True,
        input_names=["input"],
        output_names=["output"],
        dynamic_axes={
            "input": {0: "batch_size"},
            "output": {0: "batch_size"},
        },
        dynamo=False,
    )

    # Validate ONNX graph integrity
    onnx_model = onnx.load(output_path)
    onnx.checker.check_model(onnx_model)

    # Embed versioning metadata
    meta = onnx_model.metadata_props.add()
    meta.key = "model_id"
    meta.value = model_id

    meta_in = onnx_model.metadata_props.add()
    meta_in.key = "input_dim"
    meta_in.value = str(input_dim)

    meta_seq = onnx_model.metadata_props.add()
    meta_seq.key = "seq_len"
    meta_seq.value = str(seq_len)

    onnx.save(onnx_model, output_path)

    # Verify model executable via ONNX Runtime
    session = ort.InferenceSession(output_path, providers=["CPUExecutionProvider"])
    ort_inputs = {session.get_inputs()[0].name: dummy_input.numpy()}
    ort_outputs = session.run(None, ort_inputs)
    assert len(ort_outputs) > 0, "ONNX Runtime output is empty"

    return output_path


if __name__ == "__main__":
    test_model = LSTMForecaster(input_dim=9, hidden_dim=64, num_layers=2)
    path = export_to_onnx(test_model, 9, 20, "models/lstm_v1/model.onnx")
    print(f"Successfully exported and validated ONNX model at: {path}")
