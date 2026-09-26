"""Training orchestrator CLI called by Rust quantctl platform."""

import argparse
import json
import os
import sys
import numpy as np
import torch
from torch.utils.data import DataLoader

# Ensure root directory is on Python path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "../..")))
if hasattr(sys.stdout, "reconfigure"):
    try:
        sys.stdout.reconfigure(encoding="utf-8")
        sys.stderr.reconfigure(encoding="utf-8")
    except Exception:
        pass

import yaml
from python.ml.arrow_dataset import load_arrow_training_dataset
from python.ml.dataset import (
    FeatureScaler,
    PurgedWalkForwardSplitter,
    TimeSeriesTorchDataset,
    create_sliding_windows,
)
from python.ml.evaluate import compute_metrics
from python.ml.export_onnx import export_artifact_package
from python.ml.loss import DirectionalAsymmetricLoss
from python.ml.models.lstm import LSTMForecaster
from python.ml.trainer import ModelTrainer


def parse_args():
    parser = argparse.ArgumentParser(description="Train LSTM Quant Model")
    parser.add_argument("--config", type=str, default="configs/default.yaml")
    parser.add_argument("--dataset", type=str, default=None)
    parser.add_argument("--manifest", type=str, default=None)
    parser.add_argument(
        "--synthetic",
        action="store_true",
        help="Use deterministic synthetic data when no Arrow dataset is available",
    )
    return parser.parse_args()


def load_config(path: str) -> dict:
    if os.path.exists(path):
        with open(path, "r", encoding="utf-8") as f:
            return yaml.safe_load(f)
    return {}


def load_training_arrays(args, cfg, lookback):
    """Load the Rust dataset, retaining synthetic data only as an explicit fallback."""
    data_cfg = cfg.get("data", {})
    dataset_version = data_cfg.get("dataset_version", "ds_2024_v1")
    symbol = (data_cfg.get("symbols") or ["AAPL"])[0]
    dataset_path = args.dataset or os.path.join(
        "datasets", "training", dataset_version, f"{symbol}.arrow"
    )
    manifest_path = args.manifest or os.path.join(
        "datasets", "training", dataset_version, f"{symbol}.manifest.json"
    )

    if os.path.exists(dataset_path):
        dataset = load_arrow_training_dataset(
            dataset_path, manifest_path if os.path.exists(manifest_path) else None
        )
        if dataset.features.shape[0] < lookback:
            raise ValueError(
                f"Training dataset has {dataset.features.shape[0]} rows, "
                f"but lookback requires {lookback}"
            )
        return (
            dataset.features,
            dataset.targets,
            dataset.feature_names,
            dataset.timestamps,
            dataset.target_timestamps,
            dataset.feature_set_version,
            dataset.target_horizon,
            dataset_path,
        )

    if not args.synthetic:
        raise FileNotFoundError(
            f"Arrow training dataset not found at {dataset_path}. "
            "Run `quantctl features build` or pass --synthetic for a test run."
        )

    print(f"[Python ML] Using explicit synthetic training mode: {dataset_path}")
    n_samples = 300
    n_features = 9
    raw_features = np.random.randn(n_samples, n_features).astype(np.float32)
    raw_targets = (0.01 * np.random.randn(n_samples)).astype(np.float32)
    return (
        raw_features,
        raw_targets,
        [f"feature_{i}" for i in range(n_features)],
        np.arange(n_samples, dtype=np.int64),
        np.arange(1, n_samples + 1, dtype=np.int64),
        1,
        1,
        None,
    )


def main():
    args = parse_args()
    cfg = load_config(args.config)

    train_cfg = cfg.get("training", {})
    features_cfg = cfg.get("features", {})

    model_id = train_cfg.get("model_id", "lstm_v1")
    lookback = features_cfg.get("lookback", 20)
    hidden_dim = train_cfg.get("hidden_size", 64)
    num_layers = train_cfg.get("num_layers", 2)
    dropout = train_cfg.get("dropout", 0.2)
    lr = train_cfg.get("learning_rate", 0.001)
    batch_size = train_cfg.get("batch_size", 32)
    epochs = train_cfg.get("epochs", 5)  # Fast default for orchestrator run
    seed = train_cfg.get("random_seed", 42)

    torch.manual_seed(seed)
    np.random.seed(seed)

    print(
        f"[Python ML] Training model '{model_id}' "
        f"(lookback={lookback}, hidden={hidden_dim}, epochs={epochs})..."
    )

    (
        raw_features,
        raw_targets,
        feature_schema,
        timestamps,
        target_timestamps,
        feature_set_version,
        target_horizon,
        dataset_path,
    ) = load_training_arrays(args, cfg, lookback)
    n_samples, n_features = raw_features.shape

    # Split dataset with purge and embargo
    splitter = PurgedWalkForwardSplitter(
        purge_bars=max(target_horizon, train_cfg.get("purge_gap", 5)),
        embargo_bars=train_cfg.get("embargo_gap", 10),
    )
    train_idx, val_idx, test_idx = splitter.split_train_test(n_samples, train_ratio=0.7, val_ratio=0.15)

    for name, indices in [("train", train_idx), ("validation", val_idx), ("test", test_idx)]:
        if len(indices) < lookback + 1:
            raise ValueError(f"{name} split needs at least {lookback + 1} rows; increase dataset length or reduce lookback/gaps")
    if target_timestamps[train_idx[-1]] >= timestamps[val_idx[0]] or target_timestamps[val_idx[-1]] >= timestamps[test_idx[0]]:
        raise ValueError("Label horizon overlaps a later split")

    # Scale using train statistics
    scaler = FeatureScaler()
    scaled_train = scaler.fit_transform(raw_features[train_idx])
    scaled_val = scaler.transform(raw_features[val_idx])
    scaled_test = scaler.transform(raw_features[test_idx])

    if len(train_idx) < lookback:
        raise ValueError("Training split is shorter than the configured lookback")

    # Windows
    x_tr, y_tr = create_sliding_windows(scaled_train, raw_targets[train_idx], lookback)
    x_va, y_va = create_sliding_windows(scaled_val, raw_targets[val_idx], lookback)
    x_te, y_te = create_sliding_windows(scaled_test, raw_targets[test_idx], lookback)

    train_loader = DataLoader(TimeSeriesTorchDataset(x_tr, y_tr), batch_size=batch_size, shuffle=True)
    val_loader = DataLoader(TimeSeriesTorchDataset(x_va, y_va), batch_size=batch_size) if len(x_va) > 0 else None

    # Model & Trainer
    model = LSTMForecaster(input_dim=n_features, hidden_dim=hidden_dim, num_layers=num_layers, dropout=dropout)
    optimizer = torch.optim.AdamW(model.parameters(), lr=lr, weight_decay=1e-4)
    criterion = DirectionalAsymmetricLoss(alpha=2.0)

    trainer = ModelTrainer(model, optimizer, criterion)
    history = trainer.fit(train_loader, val_loader, epochs=epochs, patience=5)

    # Out of sample evaluation
    model.eval()
    with torch.no_grad():
        x_te_tensor = torch.from_numpy(x_te.astype(np.float32))
        preds = model(x_te_tensor).numpy() if len(x_te) > 0 else np.zeros((1, 1))

    metrics = compute_metrics(preds, y_te if len(y_te) > 0 else np.zeros((1,)))
    print(f"[Python ML] OOS Metrics: IC={metrics['ic']:.4f}, DirAcc={metrics['directional_accuracy']:.2%}")

    # Export complete model artifact package
    onnx_out_dir = f"models/{model_id}"
    if os.path.exists(onnx_out_dir):
        raise FileExistsError(f"Model artifact already exists: {onnx_out_dir}; choose a new model_id")
    export_artifact_package(
        model=model,
        scaler=scaler,
        output_dir=onnx_out_dir,
        model_id=model_id,
        model_version=1,
        lookback=lookback,
        architecture={"hidden_size": hidden_dim, "num_layers": num_layers, "dropout": dropout},
        hyperparameters={"lr": lr, "batch_size": batch_size, "weight_decay": 1e-4},
        evaluation_metrics=metrics,
        training_log=history,
        random_seed=seed,
        feature_schema=feature_schema,
        training_period=[str(timestamps[train_idx[0]]), str(target_timestamps[train_idx[-1]])],
        validation_period=(
            [str(timestamps[val_idx[0]]), str(target_timestamps[val_idx[-1]])]
            if len(val_idx)
            else None
        ),
        test_period=(
            [str(timestamps[test_idx[0]]), str(target_timestamps[test_idx[-1]])]
            if len(test_idx)
            else None
        ),
    )
    # Persist exact split windows, units, and an artifact-specific numerical parity check.
    import onnxruntime as ort
    import subprocess
    with torch.no_grad():
        reference = model(torch.from_numpy(x_te[:1].astype(np.float32))).numpy()
    session = ort.InferenceSession(os.path.join(onnx_out_dir, "model.onnx"), providers=["CPUExecutionProvider"])
    exported = session.run(None, {session.get_inputs()[0].name: x_te[:1].astype(np.float32)})[0]
    parity_error = float(np.max(np.abs(reference - exported)))
    if parity_error > 1e-5:
        raise ValueError(f"ONNX parity failed: {parity_error}")
    periods = {name: [str(timestamps[idx[lookback-1]]), str(target_timestamps[idx[-1]])]
               for name, idx in [("train", train_idx), ("validation", val_idx), ("test", test_idx)]}
    segments = []
    n_train_raw = int(n_samples * 0.7)
    n_val_end = n_train_raw + int(n_samples * 0.15)
    for name, begin, end in [("train", 0, int(train_idx[-1])+1),
                             ("purge", int(train_idx[-1])+1, n_train_raw),
                             ("embargo", n_train_raw, int(val_idx[0])),
                             ("validation", int(val_idx[0]), int(val_idx[-1])+1),
                             ("purge", int(val_idx[-1])+1, n_val_end),
                             ("embargo", n_val_end, int(test_idx[0])),
                             ("test", int(test_idx[0]), n_samples)]:
        segments.append({"kind": name, "start_row": begin, "end_row": end,
                         "start_ms": int(timestamps[min(begin, n_samples-1)]) // 1000000,
                         "end_ms": int(timestamps[min(max(begin, end-1), n_samples-1)]) // 1000000})
    validation = {"schema_version": 1, "data_version": cfg.get("data", {}).get("dataset_version", "unknown"),
                  "timestamp_unit": "nanoseconds", "periods": periods, "pit_passed": True,
                  "scaler_train_only": True, "purge_bars": splitter.purge_bars,
                  "embargo_bars": splitter.embargo_bars, "folds": [{"fold": 1, "segments": segments}],
                  "onnx_parity": {"passed": True, "max_abs_error": parity_error, "tolerance": 1e-5},
                  "evaluation": "single chronological holdout; rolling walk-forward retraining is not orchestrated"}
    with open(os.path.join(onnx_out_dir, "validation.json"), "w") as output:
        json.dump(validation, output, indent=2)
    meta_path = os.path.join(onnx_out_dir, "metadata.json")
    with open(meta_path) as source:
        metadata = json.load(source)
    metadata.update(training_dataset_version=validation["data_version"], feature_set_version=feature_set_version,
                    target_definition={"horizon": target_horizon, "transformation": "log_return"})
    metadata["git_commit"] = subprocess.run(["git", "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip() or "unknown"
    with open(meta_path, "w") as output:
        json.dump(metadata, output, indent=2)
    onnx_path = os.path.join(onnx_out_dir, "model.onnx")

    report = {
        "status": "SUCCESS",
        "model_id": model_id,
        "artifact_dir": onnx_out_dir,
        "onnx_artifact": onnx_path,
        "metrics": metrics,
        "epochs_completed": len(history["train_loss"]),
        "dataset_path": dataset_path,
        "feature_schema": feature_schema,
        "feature_set_version": feature_set_version,
        "target_horizon": target_horizon,
    }
    print(json.dumps(report))


if __name__ == "__main__":
    main()
