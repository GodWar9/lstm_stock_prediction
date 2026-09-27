"""Training orchestrator CLI called by Rust quantctl platform."""

import argparse
import hashlib
import json
import os
import sys
from pathlib import Path
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
from python.ml.publication import staged_model


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
    parser.add_argument(
        "--walk-forward",
        action="store_true",
        help="Enable rolling walk-forward cross validation with per-fold models",
    )
    parser.add_argument(
        "--folds",
        type=int,
        default=None,
        help="Number of folds for rolling walk-forward CV",
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
            dataset.integrity,
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
        {},
    )


def main():
    args = parse_args()
    cfg = load_config(args.config)

    model_id = cfg.get("training", {}).get("model_id", "lstm_v1")
    with staged_model(Path("models"), model_id) as stage:
        report = train_and_export(args, cfg, model_id, stage)
    # Success is emitted only after the complete directory has been published.
    print(json.dumps(report))


def train_and_export(args, cfg, model_id, stage):
    train_cfg = cfg.get("training", {})
    features_cfg = cfg.get("features", {})

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
        data_integrity,
    ) = load_training_arrays(args, cfg, lookback)
    n_samples, n_features = raw_features.shape

    purge_gap = max(target_horizon, train_cfg.get("purge_gap", 5))
    embargo_gap = train_cfg.get("embargo_gap", 10)
    is_walk_forward = getattr(args, "walk_forward", False) or train_cfg.get("walk_forward", False)

    if is_walk_forward and not hasattr(args, "_indices"):
        import copy
        import shutil
        from python.ml.walk_forward import WalkForwardCV
        from python.ml.publication import REQUIRED_FILES
        n_splits = getattr(args, "folds", None) or train_cfg.get("n_folds", 5)
        folds = list(WalkForwardCV(n_splits=n_splits, purge_gap=purge_gap,
                                  embargo_gap=embargo_gap, expanding=False).split(n_samples))
        if len(folds) != n_splits:
            raise ValueError("Dataset cannot produce the requested walk-forward folds")
        folds_info, all_preds, all_targets = [], [], []
        for number, (development, test) in enumerate(folds, 1):
            boundary = int(len(development) * 0.85)
            train = development[:max(0, boundary - purge_gap)]
            val = development[boundary + embargo_gap:]
            fold_args = copy.copy(args)
            fold_args._indices = (train, val, test)
            fold_id = f"fold_{number}"
            with staged_model(Path(stage) / "folds", fold_id) as fold_stage:
                report = train_and_export(fold_args, cfg, fold_id, fold_stage)
            fold_dir = Path(stage) / "folds" / fold_id
            validation = json.loads((fold_dir / "validation.json").read_text())
            outcomes = json.loads((fold_dir / "oos_predictions.json").read_text())
            all_preds.extend(outcomes["predictions"])
            all_targets.extend(outcomes["targets"])
            folds_info.append({"fold": number, "artifact_dir": f"folds/{fold_id}",
                               "segments": validation["folds"][0]["segments"],
                               "metrics": report["metrics"],
                               "integrity_sha256": hashlib.sha256((fold_dir / "integrity.json").read_bytes()).hexdigest()})
        for name in (*REQUIRED_FILES, "oos_predictions.json"):
            shutil.copyfile(fold_dir / name, Path(stage) / name)
        metadata_path = Path(stage) / "metadata.json"
        metadata = json.loads(metadata_path.read_text())
        metadata["model_id"] = model_id
        metadata_path.write_text(json.dumps(metadata, indent=2))
        validation["folds"] = folds_info
        validation["evaluation"] = "rolling walk-forward; root model and periods are the final fold"
        validation["pooled_oos_metrics"] = compute_metrics(np.array(all_preds), np.array(all_targets))
        (Path(stage) / "validation.json").write_text(json.dumps(validation, indent=2))
        report.update(model_id=model_id, artifact_dir=f"models/{model_id}",
                      onnx_artifact=f"models/{model_id}/model.onnx",
                      pooled_oos_metrics=validation["pooled_oos_metrics"])
        return report

    # Split dataset with purge and embargo
    splitter = PurgedWalkForwardSplitter(
        purge_bars=purge_gap,
        embargo_bars=embargo_gap,
    )
    train_idx, val_idx, test_idx = getattr(args, "_indices", None) or splitter.split_train_test(n_samples, train_ratio=0.7, val_ratio=0.15)

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
    onnx_out_dir = str(stage)
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
    del session
    if not np.isfinite(parity_error) or parity_error > 1e-5:
        raise ValueError(f"ONNX parity failed: {parity_error}")
    periods = {name: [str(timestamps[idx[lookback-1]]), str(target_timestamps[idx[-1]])]
               for name, idx in [("train", train_idx), ("validation", val_idx), ("test", test_idx)]}

    segments = []
    for name, begin, end in [("train", int(train_idx[0]), int(train_idx[-1])+1),
                             ("purge", int(train_idx[-1])+1, int(val_idx[0])-embargo_gap),
                             ("embargo", int(val_idx[0])-embargo_gap, int(val_idx[0])),
                             ("validation", int(val_idx[0]), int(val_idx[-1])+1),
                             ("purge", int(val_idx[-1])+1, int(test_idx[0])-embargo_gap),
                             ("embargo", int(test_idx[0])-embargo_gap, int(test_idx[0])),
                             ("test", int(test_idx[0]), int(test_idx[-1])+1)]:
        if end > begin:
            segments.append({"kind": name, "start_row": begin, "end_row": end,
                             "start_ms": int(timestamps[begin]) // 1000000,
                             "end_ms": int(timestamps[end-1]) // 1000000})
    folds_data = [{"fold": 1, "segments": segments}]
    eval_desc = "single chronological holdout"

    validation = {"schema_version": 1, "data_version": cfg.get("data", {}).get("dataset_version", "unknown"),
                  "data_integrity": data_integrity, "timestamp_unit": "nanoseconds", "periods": periods, "pit_passed": True,
                  "scaler_train_only": True, "purge_bars": purge_gap,
                  "embargo_bars": embargo_gap, "folds": folds_data,
                  "onnx_parity": {"passed": True, "max_abs_error": parity_error, "tolerance": 1e-5},
                  "evaluation": eval_desc}
    with open(os.path.join(onnx_out_dir, "validation.json"), "w") as output:
        json.dump(validation, output, indent=2)
    meta_path = os.path.join(onnx_out_dir, "metadata.json")
    with open(meta_path) as source:
        metadata = json.load(source)
    metadata.update(training_dataset_version=validation["data_version"], feature_set_version=feature_set_version,
                    target_definition={"horizon": target_horizon, "transformation": "log_return"})
    executable = os.environ.get("QUANTCTL_EXECUTABLE")
    if not executable:
        raise RuntimeError("Set QUANTCTL_EXECUTABLE to record source provenance")
    snapshot = subprocess.run([executable, "source-snapshot"], capture_output=True, text=True, check=True)
    metadata["source_snapshot"] = json.loads(snapshot.stdout.strip().splitlines()[-1])
    metadata["git_commit"] = metadata["source_snapshot"]["git_commit"]
    with open(meta_path, "w") as output:
        json.dump(metadata, output, indent=2)
    raw_windows, _ = create_sliding_windows(raw_features[test_idx], raw_targets[test_idx], lookback)
    selected = np.unique(np.linspace(0, len(x_te) - 1, min(3, len(x_te)), dtype=int))
    with torch.no_grad():
        references = model(torch.from_numpy(x_te[selected].astype(np.float32))).numpy().reshape(-1)
    validation["runtime_parity"] = {"cases": [
        {"raw_features": raw_windows[int(index)].reshape(-1).astype(float).tolist(), "expected": float(expected)}
        for index, expected in zip(selected, references)
    ]}
    with open(os.path.join(onnx_out_dir, "validation.json"), "w") as output:
        json.dump(validation, output, indent=2)
    executable = os.environ.get("QUANTCTL_EXECUTABLE")
    if not executable:
        raise RuntimeError("Train through quantctl or set QUANTCTL_EXECUTABLE for the mandatory Rust parity gate")
    check = subprocess.run([executable, "verify-model", "--artifact-dir", onnx_out_dir],
                           capture_output=True, text=True, check=True)
    validation["runtime_parity"].update(json.loads(check.stdout.strip().splitlines()[-1]))
    with open(os.path.join(onnx_out_dir, "validation.json"), "w") as output:
        json.dump(validation, output, indent=2)
    outcomes_path = Path(stage) / "oos_predictions.json"
    outcomes_path.write_text(json.dumps({
        "predictions": preds.reshape(-1).astype(float).tolist(),
        "targets": y_te.reshape(-1).astype(float).tolist(),
        "timestamps": [str(t) for t in timestamps[test_idx][lookback-1:]],
    }))
    validation["oos_predictions_sha256"] = hashlib.sha256(outcomes_path.read_bytes()).hexdigest()
    (Path(stage) / "validation.json").write_text(json.dumps(validation, indent=2))
    published_dir = os.path.join("models", model_id)
    onnx_path = os.path.join(published_dir, "model.onnx")

    report = {
        "status": "SUCCESS",
        "model_id": model_id,
        "artifact_dir": published_dir,
        "onnx_artifact": onnx_path,
        "metrics": metrics,
        "epochs_completed": len(history["train_loss"]),
        "dataset_path": dataset_path,
        "feature_schema": feature_schema,
        "feature_set_version": feature_set_version,
        "target_horizon": target_horizon,
    }
    return report


if __name__ == "__main__":
    main()
