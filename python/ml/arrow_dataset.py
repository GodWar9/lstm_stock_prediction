"""Reader and schema validation for Rust-generated Arrow training datasets."""

from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
from typing import Dict, List

import numpy as np


REQUIRED_COLUMNS = {
    "timestamp",
    "target_timestamp",
    "target",
    "asset_id",
    "feature_set_version",
    "target_horizon",
}


@dataclass(frozen=True)
class ArrowTrainingDataset:
    """Typed in-memory representation consumed by the training pipeline."""

    timestamps: np.ndarray
    features: np.ndarray
    targets: np.ndarray
    feature_names: List[str]
    asset_ids: np.ndarray
    target_timestamps: np.ndarray
    feature_set_version: int
    target_horizon: int
    integrity: dict


def load_arrow_training_dataset(
    path: str | Path,
    manifest_path: str | Path | None = None,
) -> ArrowTrainingDataset:
    """Load and validate a Rust Arrow IPC training dataset.

    The loader rejects missing required columns, null values, unsorted
    timestamps, and manifest row-count mismatches before model training.
    """

    try:
        import pyarrow.ipc as ipc
    except ImportError as exc:
        raise RuntimeError(
            "pyarrow is required to read Rust Arrow IPC datasets; install requirements.txt"
        ) from exc

    dataset_path = Path(path)
    if manifest_path is None:
        raise ValueError("A hashed training manifest is required; rebuild features")
    with Path(manifest_path).open(encoding="utf-8") as handle:
        manifest = json.load(handle)
    with dataset_path.open("rb") as handle:
        digest = hashlib.file_digest(handle, "sha256").hexdigest()
        if manifest.get("content_sha256") != digest:
            raise ValueError("Training dataset integrity mismatch; rebuild features")
        handle.seek(0)
        table = ipc.open_stream(handle).read_all()

    columns = set(table.column_names)
    missing = REQUIRED_COLUMNS - columns
    if missing:
        raise ValueError(f"Arrow dataset is missing required columns: {sorted(missing)}")

    if table.num_rows == 0:
        raise ValueError("Arrow training dataset is empty")

    pandas_frame = table.to_pandas()
    if pandas_frame[list(REQUIRED_COLUMNS)].isnull().any().any():
        raise ValueError("Arrow training dataset contains nulls in required columns")

    timestamps = pandas_frame["timestamp"].to_numpy(dtype=np.int64)
    if np.any(np.diff(timestamps) <= 0):
        raise ValueError("Arrow training timestamps must be strictly increasing")

    feature_names = sorted(columns - REQUIRED_COLUMNS)
    if not feature_names:
        raise ValueError("Arrow training dataset contains no feature columns")

    features = pandas_frame[feature_names].to_numpy(dtype=np.float32)
    targets = pandas_frame["target"].to_numpy(dtype=np.float32)
    if not np.isfinite(features).all() or not np.isfinite(targets).all():
        raise ValueError("Training features and targets must be finite")
    target_timestamps = pandas_frame["target_timestamp"].to_numpy(dtype=np.int64)
    asset_ids = pandas_frame["asset_id"].to_numpy(dtype=np.uint32)

    if np.any(target_timestamps <= timestamps):
        raise ValueError("Target timestamps must be later than feature timestamps")

    feature_set_versions = pandas_frame["feature_set_version"].to_numpy(dtype=np.uint32)
    horizons = pandas_frame["target_horizon"].to_numpy(dtype=np.uint16)
    if len(np.unique(feature_set_versions)) != 1 or len(np.unique(horizons)) != 1:
        raise ValueError("Arrow dataset must contain one feature-set version and horizon")

    if manifest_path is not None:
        with Path(manifest_path).open("r", encoding="utf-8") as handle:
            manifest: Dict[str, object] = json.load(handle)
        if manifest.get("row_count") != table.num_rows:
            raise ValueError("Arrow row count does not match its manifest")
        manifest_features = sorted(manifest.get("feature_columns", []))
        if manifest_features != feature_names:
            raise ValueError("Arrow feature columns do not match its manifest")

    return ArrowTrainingDataset(
        timestamps=timestamps,
        features=features,
        targets=targets,
        feature_names=feature_names,
        asset_ids=asset_ids,
        target_timestamps=target_timestamps,
        feature_set_version=int(feature_set_versions[0]),
        target_horizon=int(horizons[0]),
        integrity={"training_sha256": digest,
                   "market_sha256": manifest.get("source_market_sha256", ""),
                   "symbol": manifest.get("symbol", ""),
                   "dataset_version": manifest.get("dataset_version", "")},
    )
