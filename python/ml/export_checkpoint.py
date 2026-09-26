"""Re-export a verified checkpoint to a new, atomically published model ID."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
import numpy as np
import torch
from python.ml.dataset import FeatureScaler
from python.ml.export_onnx import export_artifact_package
from python.ml.models.lstm import LSTMForecaster
from python.ml.publication import REQUIRED_FILES, staged_model


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", required=True)
    parser.add_argument("--model-id", required=True)
    args = parser.parse_args()
    source = Path(args.source)
    integrity = json.loads((source / "integrity.json").read_text())
    if integrity.get("schema_version") != 1 or integrity.get("algorithm") != "sha256" or set(integrity.get("files", {})) != set(REQUIRED_FILES):
        raise ValueError("Unsupported source package integrity manifest")
    for name in REQUIRED_FILES:
        content = (source / name).read_bytes()
        if integrity["files"][name] != {"sha256": hashlib.sha256(content).hexdigest(), "size_bytes": len(content)}:
            raise ValueError(f"Source integrity mismatch: {name}")
    meta = json.loads((source / "metadata.json").read_text())
    if meta["model_id"] != integrity["model_id"]:
        raise ValueError("Source model identity mismatch")
    validation = json.loads((source / "validation.json").read_text())
    scaler_data = json.loads((source / "scaler.json").read_text())
    scaler = FeatureScaler()
    scaler.mean_ = np.asarray(scaler_data["mean"])
    scaler.std_ = np.asarray(scaler_data["std"])
    arch = meta["architecture"]
    model = LSTMForecaster(input_dim=len(meta["feature_schema"]), hidden_dim=arch["hidden_size"],
                           num_layers=arch["num_layers"], dropout=arch["dropout"])
    model.load_state_dict(torch.load(source / "model.pt", map_location="cpu", weights_only=True))
    model.eval()
    executable = os.environ["QUANTCTL_EXECUTABLE"]
    with staged_model(Path("models"), args.model_id) as stage:
        export_artifact_package(model, scaler, str(stage), model_id=args.model_id,
            model_version=meta["model_version"], feature_schema=meta["feature_schema"], lookback=meta["lookback"],
            architecture=arch, hyperparameters=meta["hyperparameters"],
            training_period=meta["training_period"], validation_period=meta["validation_period"],
            test_period=meta["test_period"], evaluation_metrics=meta["evaluation_metrics"],
            training_log=json.loads((source / "training_log.json").read_text()),
            random_seed=meta["random_seed"], opset_version=meta["onnx_opset"])
        meta.update(model_id=args.model_id, exported_from=integrity["model_id"])
        (stage / "metadata.json").write_text(json.dumps(meta, indent=2))
        (stage / "validation.json").write_text(json.dumps(validation, indent=2))
        result = subprocess.run([executable, "verify-model", "--artifact-dir", str(stage)],
                                capture_output=True, text=True, check=True)
        validation["runtime_parity"].update(json.loads(result.stdout.strip().splitlines()[-1]))
        (stage / "validation.json").write_text(json.dumps(validation, indent=2))
    print(json.dumps({"status": "SUCCESS", "model_id": args.model_id, "artifact_dir": f"models/{args.model_id}"}))


if __name__ == "__main__":
    main()
