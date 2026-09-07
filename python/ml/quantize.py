"""Model quantization utility for low-latency production inference."""

from typing import Optional
import os
import sys

# Ensure root directory is on Python path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "../..")))

from onnxruntime.quantization import QuantType, quantize_dynamic


def quantize_onnx_model(
    input_model_path: str,
    output_model_path: str,
    weight_type: QuantType = QuantType.QInt8,
) -> str:
    """Apply dynamic INT8 quantization to an ONNX model artifact."""
    assert os.path.exists(input_model_path), f"Model not found at: {input_model_path}"
    os.makedirs(os.path.dirname(output_model_path), exist_ok=True)

    quantize_dynamic(
        model_input=input_model_path,
        model_output=output_model_path,
        weight_type=weight_type,
    )

    orig_size = os.path.getsize(input_model_path)
    quant_size = os.path.getsize(output_model_path)
    print(
        f"Quantization completed: {orig_size / 1024:.1f} KB -> {quant_size / 1024:.1f} KB "
        f"({(1 - quant_size / orig_size) * 100:.1f}% reduction)"
    )

    return output_model_path


if __name__ == "__main__":
    in_path = "models/lstm_v1/model.onnx"
    out_path = "models/lstm_v1/model_int8.onnx"
    if os.path.exists(in_path):
        quantize_onnx_model(in_path, out_path)
    else:
        print(f"Base model {in_path} not found; skipping CLI run.")
