"""Tests for the Rust-to-Python Arrow training contract."""

import hashlib
import json
import os
import sys

import numpy as np
import pytest

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "../..")))

pyarrow = pytest.importorskip("pyarrow")

from python.ml.arrow_dataset import load_arrow_training_dataset


def _write_dataset(tmp_path, timestamps=(1, 2), target_timestamps=(2, 3)):
    import pyarrow as pa
    import pyarrow.ipc as ipc

    table = pa.table(
        {
            "timestamp": pa.array(timestamps, type=pa.int64()),
            "target_timestamp": pa.array(target_timestamps, type=pa.int64()),
            "target": pa.array([0.1, -0.2], type=pa.float32()),
            "asset_id": pa.array([1, 1], type=pa.uint32()),
            "feature_set_version": pa.array([1, 1], type=pa.uint32()),
            "target_horizon": pa.array([1, 1], type=pa.uint16()),
            "sma": pa.array([10.0, 11.0], type=pa.float32()),
        }
    )
    path = tmp_path / "dataset.arrow"
    with path.open("wb") as handle:
        with ipc.new_stream(handle, table.schema) as writer:
            writer.write_table(table)
    manifest = tmp_path / "dataset.manifest.json"
    manifest.write_text(
        json.dumps({"row_count": 2, "feature_columns": ["sma"], "content_sha256": hashlib.sha256(path.read_bytes()).hexdigest()}), encoding="utf-8"
    )
    return path, manifest


def test_loads_valid_arrow_training_dataset(tmp_path):
    path, manifest = _write_dataset(tmp_path)
    dataset = load_arrow_training_dataset(path, manifest)
    assert dataset.features.shape == (2, 1)
    np.testing.assert_array_equal(dataset.targets, np.array([0.1, -0.2], dtype=np.float32))
    assert dataset.feature_names == ["sma"]


def test_rejects_non_monotonic_timestamps(tmp_path):
    path, manifest = _write_dataset(tmp_path, timestamps=(2, 1), target_timestamps=(3, 2))
    with pytest.raises(ValueError, match="strictly increasing"):
        load_arrow_training_dataset(path, manifest)


def test_rejects_changed_bytes_and_missing_manifest(tmp_path):
    path, manifest = _write_dataset(tmp_path)
    with path.open("ab") as output:
        output.write(b" ")
    with pytest.raises(ValueError, match="integrity mismatch"):
        load_arrow_training_dataset(path, manifest)
    with pytest.raises(ValueError, match="manifest is required"):
        load_arrow_training_dataset(path)
