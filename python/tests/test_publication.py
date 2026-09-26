"""Failure, visibility and collision guarantees for model publication."""

import json
import hashlib
from pathlib import Path
from types import SimpleNamespace

import pytest

from python.ml.publication import staged_model


def write_package(stage, model_id="example", error=0.0):
    for name in ("model.onnx", "model.pt"):
        (stage / name).write_bytes(b"test fixture")
    documents = {
        "metadata.json": {"model_id": model_id},
        "scaler.json": {"mean": [0], "std": [1]},
        "training_log.json": {"train_loss": [0.1]},
        "validation.json": {"onnx_parity": {
            "passed": True, "max_abs_error": error, "tolerance": 1e-5,
        }, "runtime_parity": {"passed": True}},
    }
    for name, value in documents.items():
        (stage / name).write_text(json.dumps(value), encoding="utf-8")


def test_model_only_visible_after_success(tmp_path):
    with staged_model(tmp_path, "example") as stage:
        write_package(stage)
        assert not (tmp_path / "example").exists()
        assert not any((p / "metadata.json").exists() for p in tmp_path.iterdir())
    assert (tmp_path / "example" / "model.onnx").read_bytes() == b"test fixture"
    manifest = json.loads((tmp_path / "example" / "integrity.json").read_text())
    assert manifest["schema_version"] == 1
    assert manifest["algorithm"] == "sha256"
    assert manifest["model_id"] == "example"
    assert len(manifest["files"]) == 6
    for name, entry in manifest["files"].items():
        payload = (tmp_path / "example" / name).read_bytes()
        assert entry == {"sha256": hashlib.sha256(payload).hexdigest(), "size_bytes": len(payload)}
    assert list((tmp_path / ".staging").iterdir()) == []


def test_export_failure_cleans_stage_and_allows_retry(tmp_path):
    with pytest.raises(RuntimeError, match="export interrupted"):
        with staged_model(tmp_path, "example") as stage:
            write_package(stage)
            raise RuntimeError("export interrupted")
    assert not (tmp_path / "example").exists()
    assert list((tmp_path / ".staging").iterdir()) == []
    with staged_model(tmp_path, "example") as stage:
        write_package(stage)


@pytest.mark.parametrize("error", [float("nan"), float("inf"), -1.0, 1e-3])
def test_invalid_parity_never_publishes(tmp_path, error):
    with pytest.raises(ValueError, match="parity"):
        with staged_model(tmp_path, "example") as stage:
            write_package(stage, error=error)
    assert not (tmp_path / "example").exists()
    assert list((tmp_path / ".staging").iterdir()) == []


def test_missing_file_never_publishes(tmp_path):
    with pytest.raises(ValueError, match="validation.json"):
        with staged_model(tmp_path, "example") as stage:
            write_package(stage)
            (stage / "validation.json").unlink()
    assert not (tmp_path / "example").exists()


def test_wrong_model_id_never_publishes(tmp_path):
    with pytest.raises(ValueError, match="publication ID"):
        with staged_model(tmp_path, "example") as stage:
            write_package(stage, model_id="different")
    assert not (tmp_path / "example").exists()


@pytest.mark.parametrize("content", ["{", "[]", '{"onnx_parity": null}'])
def test_invalid_validation_document_never_publishes(tmp_path, content):
    with pytest.raises(ValueError):
        with staged_model(tmp_path, "example") as stage:
            write_package(stage)
            (stage / "validation.json").write_text(content, encoding="utf-8")
    assert not (tmp_path / "example").exists()
    assert list((tmp_path / ".staging").iterdir()) == []


def test_destination_created_during_export_is_preserved(tmp_path):
    with pytest.raises(FileExistsError, match="already exists"):
        with staged_model(tmp_path, "example") as stage:
            write_package(stage)
            (tmp_path / "example").mkdir()
            (tmp_path / "example" / "sentinel").write_text("another writer")
    assert (tmp_path / "example" / "sentinel").read_text() == "another writer"
    assert list((tmp_path / ".staging").iterdir()) == []


def test_duplicate_writer_does_not_release_first_writers_lock(tmp_path):
    with staged_model(tmp_path, "example") as stage:
        with pytest.raises(FileExistsError, match="reserved"):
            with staged_model(tmp_path, "example"):
                pytest.fail("Duplicate writer must not start")
        assert (tmp_path / ".staging" / "example.lock").is_dir()
        write_package(stage)
    original = (tmp_path / "example" / "metadata.json").read_bytes()
    with pytest.raises(FileExistsError, match="already exists"):
        with staged_model(tmp_path, "example"):
            pytest.fail("Published models must not be overwritten")
    assert (tmp_path / "example" / "metadata.json").read_bytes() == original


@pytest.mark.parametrize("model_id", ["../outside", "/absolute", "a/b", "a\\b", "", ".", "CON", "lpt1", "a" * 129])
def test_invalid_ids_rejected_before_writing(tmp_path, model_id):
    with pytest.raises(ValueError, match="model_id"):
        with staged_model(tmp_path, model_id):
            pytest.fail("Invalid ID must not start an export")
    assert list(tmp_path.iterdir()) == []


def test_orchestrator_reports_success_only_after_publication(tmp_path, monkeypatch, capsys):
    from python.ml import train_orchestrator as orchestrator

    monkeypatch.chdir(tmp_path)
    monkeypatch.setattr(orchestrator, "parse_args", lambda: SimpleNamespace(config="unused"))
    monkeypatch.setattr(orchestrator, "load_config", lambda _: {"training": {"model_id": "example"}})

    def fake_export(args, cfg, model_id, stage):
        write_package(stage, model_id)
        assert not Path("models/example").exists()
        return {"status": "SUCCESS", "artifact_dir": "models/example"}

    monkeypatch.setattr(orchestrator, "train_and_export", fake_export)
    orchestrator.main()
    assert json.loads(capsys.readouterr().out)["status"] == "SUCCESS"
    assert Path("models/example/validation.json").is_file()
    with pytest.raises(FileExistsError):
        orchestrator.main()
    assert capsys.readouterr().out == ""


def test_orchestrator_failure_does_not_report_success(tmp_path, monkeypatch, capsys):
    from python.ml import train_orchestrator as orchestrator

    monkeypatch.chdir(tmp_path)
    monkeypatch.setattr(orchestrator, "parse_args", lambda: SimpleNamespace(config="unused"))
    monkeypatch.setattr(orchestrator, "load_config", lambda _: {"training": {"model_id": "example"}})

    def failing_export(args, cfg, model_id, stage):
        write_package(stage, model_id)
        raise RuntimeError("parity failed")

    monkeypatch.setattr(orchestrator, "train_and_export", failing_export)
    with pytest.raises(RuntimeError, match="parity failed"):
        orchestrator.main()
    assert capsys.readouterr().out == ""
    assert not Path("models/example").exists()
    assert list(Path("models/.staging").iterdir()) == []
