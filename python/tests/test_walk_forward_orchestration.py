"""Tests for walk-forward cross validation and orchestrator execution."""

import json
import os
import sys
import tempfile
from pathlib import Path
import numpy as np
import pytest

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "../..")))

from python.ml.walk_forward import WalkForwardCV
from python.ml.train_orchestrator import train_and_export, load_training_arrays
from python.ml.publication import staged_model


def test_walk_forward_cv_splits_and_leakage():
    n_samples = 500
    n_splits = 4
    purge_gap = 5
    embargo_gap = 10
    wf = WalkForwardCV(n_splits=n_splits, purge_gap=purge_gap, embargo_gap=embargo_gap, expanding=False)
    splits = list(wf.split(n_samples))

    assert len(splits) == n_splits
    for train_idx, test_idx in splits:
        assert len(train_idx) > 0
        assert len(test_idx) > 0
        assert len(np.intersect1d(train_idx, test_idx)) == 0
        assert train_idx[-1] + purge_gap + embargo_gap < test_idx[0]


def test_walk_forward_orchestrator_synthetic(monkeypatch):
    monkeypatch.chdir(Path(__file__).resolve().parents[2])
    quantctl_bin = os.path.abspath("rust/target/debug/quantctl" + (".exe" if os.name == "nt" else ""))
    monkeypatch.setenv("QUANTCTL_EXECUTABLE", quantctl_bin)

    class Args:
        config = "configs/default.yaml"
        dataset = None
        manifest = None
        synthetic = True
        walk_forward = True
        folds = 3

    cfg = {
        "training": {
            "model_id": "test_wf_model",
            "hidden_size": 16,
            "num_layers": 1,
            "dropout": 0.0,
            "learning_rate": 0.01,
            "batch_size": 32,
            "epochs": 1,
            "random_seed": 42,
            "purge_gap": 2,
            "embargo_gap": 2,
        },
        "features": {
            "lookback": 5,
        },
        "data": {
            "dataset_version": "synthetic_wf_test",
        },
    }

    with tempfile.TemporaryDirectory() as tmp_dir:
        models_root = Path(tmp_dir)
        with staged_model(models_root, "test_wf_model") as stage:
            report = train_and_export(Args(), cfg, "test_wf_model", stage)

        published_dir = models_root / "test_wf_model"
        assert published_dir.exists()
        assert (published_dir / "integrity.json").exists()
        assert (published_dir / "model.onnx").exists()
        assert (published_dir / "model.pt").exists()
        assert (published_dir / "validation.json").exists()

        with open(published_dir / "validation.json", encoding="utf-8") as f:
            val = json.load(f)

        assert "rolling walk-forward" in val["evaluation"]
        assert "pooled_oos_metrics" in val
        assert len(val["folds"]) >= 2
        for fold in val["folds"]:
            assert "segments" in fold
            assert "metrics" in fold
            assert len(fold["segments"]) >= 2

            fold_dir = published_dir / fold["artifact_dir"]
            assert (fold_dir / "model.onnx").is_file()
            evidence = json.loads((fold_dir / "validation.json").read_text())
            assert evidence["runtime_parity"]["passed"] is True
            segments = {s["kind"]: s for s in fold["segments"] if s["kind"] in ("train", "validation", "test")}
            assert segments["train"]["end_row"] + 4 <= segments["validation"]["start_row"]
            assert segments["validation"]["end_row"] + 4 <= segments["test"]["start_row"]
            np.random.seed(42)
            features = load_training_arrays(Args(), cfg, 5)[0]
            train_rows = features[segments["train"]["start_row"]:segments["train"]["end_row"]]
            scaler = json.loads((fold_dir / "scaler.json").read_text())
            np.testing.assert_allclose(scaler["mean"], train_rows.mean(axis=0), atol=1e-7)
            outcomes = json.loads((fold_dir / "oos_predictions.json").read_text())
            assert len(outcomes["predictions"]) == len(outcomes["targets"]) == len(outcomes["timestamps"])
        assert len(val["folds"]) == 3
        root_metadata = json.loads((published_dir / "metadata.json").read_text())
        assert root_metadata["model_id"] == "test_wf_model"
