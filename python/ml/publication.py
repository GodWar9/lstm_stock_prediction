"""Publish complete model directories without exposing an in-progress export."""

from contextlib import contextmanager
import hashlib
import json
import math
from pathlib import Path
import re
from tempfile import TemporaryDirectory


REQUIRED_FILES = (
    "model.onnx", "model.pt", "scaler.json", "metadata.json",
    "training_log.json", "validation.json",
)


def seal_package(directory: Path, model_id: str) -> None:
    """Record byte-level hashes after all validation and metadata writes."""
    files = {}
    for name in REQUIRED_FILES:
        path = directory / name
        if path.is_symlink():
            raise ValueError(f"Model package cannot contain symlinks: {name}")
        with path.open("rb") as source:
            digest = hashlib.file_digest(source, "sha256").hexdigest()
        files[name] = {"sha256": digest, "size_bytes": path.stat().st_size}
    manifest = {
        "schema_version": 1, "algorithm": "sha256", "model_id": model_id,
        "files": files,
    }
    (directory / "integrity.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def validate_package(directory: Path, model_id: str) -> None:
    """Require the full export and successful finite parity evidence."""
    documents = {}
    for name in REQUIRED_FILES:
        path = directory / name
        if not path.is_file() or path.stat().st_size == 0:
            raise ValueError(f"Incomplete model package: missing or empty {name}")
        if path.suffix == ".json":
            with path.open(encoding="utf-8") as source:
                documents[name] = json.load(source)
            if not isinstance(documents[name], dict):
                raise ValueError(f"Model package document must be an object: {name}")
    if documents["metadata.json"].get("model_id") != model_id:
        raise ValueError("Model package ID does not match its publication ID")
    parity = documents["validation.json"].get("onnx_parity", {})
    if not isinstance(parity, dict):
        raise ValueError("Invalid ONNX parity evidence")
    error = parity.get("max_abs_error")
    tolerance = parity.get("tolerance")
    if (
        parity.get("passed") is not True
        or type(error) not in (int, float)
        or type(tolerance) not in (int, float)
        or not math.isfinite(error)
        or not math.isfinite(tolerance)
        or not 0 <= error <= tolerance <= 1e-5
    ):
        raise ValueError("Model package has no successful finite ONNX parity evidence")
    runtime = documents["validation.json"].get("runtime_parity", {})
    if not isinstance(runtime, dict) or runtime.get("passed") is not True:
        raise ValueError("Model package requires a passing Rust runtime parity gate")


@contextmanager
def staged_model(models_root: Path, model_id: str):
    """Reserve an ID and atomically rename its validated export into place.

    Staging is on the same filesystem as the final directory. Cooperative
    writers serialize through an exclusive directory lock. Abrupt process
    termination can leave a hidden stage/lock, but never a partial final model.
    """
    reserved = {"CON", "PRN", "AUX", "NUL"} | {
        f"{prefix}{i}" for prefix in ("COM", "LPT") for i in range(1, 10)
    }
    if (
        not isinstance(model_id, str)
        or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]{0,127}", model_id)
        or model_id.upper() in reserved
    ):
        raise ValueError("model_id must be a portable filename of 1-128 letters, digits, underscores or hyphens")
    models_root = Path(models_root).resolve()
    destination = models_root / model_id
    if destination.exists() or destination.is_symlink():
        raise FileExistsError(f"Model artifact already exists: {destination}; choose a new model_id")
    staging_root = models_root / ".staging"
    staging_root.mkdir(parents=True, exist_ok=True)
    lock = staging_root / f"{model_id}.lock"
    try:
        lock.mkdir()
    except FileExistsError as exc:
        raise FileExistsError(f"Model ID is reserved by another training attempt: {model_id}; inspect {lock}") from exc
    try:
        if destination.exists() or destination.is_symlink():
            raise FileExistsError(f"Model artifact already exists: {destination}")
        with TemporaryDirectory(prefix=f"{model_id}-", dir=staging_root) as temporary:
            stage = Path(temporary)
            yield stage
            validate_package(stage, model_id)
            seal_package(stage, model_id)
            if destination.exists() or destination.is_symlink():
                raise FileExistsError(f"Model artifact already exists: {destination}")
            stage.rename(destination)
    finally:
        lock.rmdir()
