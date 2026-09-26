"""Export PyTorch LSTM models to validated ONNX artifacts with dynamic axes."""

from typing import Dict, List, Optional
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

import numpy as np
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


def export_artifact_package(
    model: nn.Module,
    scaler,
    output_dir: str,
    model_id: str = "lstm_v1",
    model_version: int = 1,
    feature_schema: Optional[List[str]] = None,
    lookback: int = 20,
    architecture: Optional[Dict] = None,
    hyperparameters: Optional[Dict] = None,
    training_period: Optional[List[str]] = None,
    validation_period: Optional[List[str]] = None,
    test_period: Optional[List[str]] = None,
    evaluation_metrics: Optional[Dict] = None,
    training_log: Optional[Dict] = None,
    random_seed: int = 42,
    opset_version: int = 17,
) -> str:
    """Export the complete production model artifact directory."""
    import json
    from datetime import datetime, timezone

    os.makedirs(output_dir, exist_ok=True)
    if feature_schema is None:
        feature_schema = [
            "log_return",
            "rolling_vol_10",
            "rsi_14",
            "macd_diff",
            "bollinger_width",
            "atr_14",
            "adx_14",
            "obv_zscore",
            "volume_ratio",
        ]

    input_dim = len(feature_schema)

    # 1. Export ONNX model
    onnx_path = os.path.join(output_dir, "model.onnx")
    export_to_onnx(model, input_dim, lookback, onnx_path, model_id=model_id, opset_version=opset_version)

    # 2. Export PyTorch weights checkpoint
    pt_path = os.path.join(output_dir, "model.pt")
    torch.save(model.state_dict(), pt_path)

    # 3. Export fitted scaler parameters
    scaler_path = os.path.join(output_dir, "scaler.json")
    mean_list = scaler.mean_.tolist() if hasattr(scaler, "mean_") and scaler.mean_ is not None else [0.0] * input_dim
    std_list = scaler.std_.tolist() if hasattr(scaler, "std_") and scaler.std_ is not None else [1.0] * input_dim
    scaler_data = {
        "mean": mean_list,
        "std": std_list,
        "feature_names": feature_schema,
    }
    with open(scaler_path, "w", encoding="utf-8") as f:
        json.dump(scaler_data, f, indent=2)

    # 4. Export metadata.json
    metadata_path = os.path.join(output_dir, "metadata.json")
    metadata = {
        "model_id": model_id,
        "model_version": model_version,
        "training_dataset_version": "ds_2024_v1",
        "feature_set_version": 1,
        "feature_schema": feature_schema,
        "target_definition": {"horizon": 1, "transformation": "log_return"},
        "lookback": lookback,
        "architecture": architecture or {"hidden_size": 64, "num_layers": 2, "dropout": 0.2},
        "hyperparameters": hyperparameters or {"lr": 0.001, "batch_size": 32, "weight_decay": 0.0001},
        "training_period": training_period,
        "validation_period": validation_period,
        "test_period": test_period,
        "random_seed": random_seed,
        "framework_version": f"torch=={torch.__version__}",
        "onnx_opset": opset_version,
        "evaluation_metrics": evaluation_metrics or {},
        "git_commit": "HEAD",
        "created_at": datetime.now(timezone.utc).isoformat(),
    }
    with open(metadata_path, "w", encoding="utf-8") as f:
        json.dump(metadata, f, indent=2)

    # 5. Export training_log.json
    log_path = os.path.join(output_dir, "training_log.json")
    with open(log_path, "w", encoding="utf-8") as f:
        json.dump(training_log or {"status": "trained"}, f, indent=2)

    return output_dir


if __name__ == "__main__":
    from python.ml.dataset import FeatureScaler
    test_model = LSTMForecaster(input_dim=9, hidden_dim=64, num_layers=2)
    dummy_scaler = FeatureScaler()
    dummy_scaler.fit(np.random.randn(100, 9).astype(np.float32))
    path = export_artifact_package(
        test_model,
        dummy_scaler,
        output_dir="models/lstm_v1",
        model_id="lstm_v1",
        model_version=1,
        lookback=20,
    )
    print(f"Successfully exported full artifact package to: {path}")
