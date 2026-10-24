"""Model publication must survive transient locks without exposing partial models."""
from pathlib import Path

import pytest

from python.ml import publication
from python.tests.test_publication import write_package


def locked(code):
    error = PermissionError('locked by another process')
    error.winerror = code
    return error


@pytest.mark.parametrize('code', [5, 32, 33])
def test_transient_windows_lock_preserves_atomic_visibility(tmp_path, monkeypatch, code):
    rename = Path.rename
    calls, waits = [], []
    monkeypatch.setattr(publication.time, 'sleep', waits.append)

    def transient(stage, destination):
        assert not destination.exists()
        calls.append(stage)
        if len(calls) < 3:
            raise locked(code)
        return rename(stage, destination)

    monkeypatch.setattr(Path, 'rename', transient)
    with publication.staged_model(tmp_path, 'example') as stage:
        write_package(stage)
    assert (tmp_path / 'example' / 'integrity.json').is_file()
    assert waits == [0.1, 0.2]
    assert list((tmp_path / '.staging').iterdir()) == []


def test_persistent_lock_is_bounded_and_never_publishes(tmp_path, monkeypatch):
    calls, waits = [], []
    monkeypatch.setattr(publication.time, 'sleep', waits.append)

    def denied(stage, destination):
        calls.append(stage)
        raise locked(5)

    monkeypatch.setattr(Path, 'rename', denied)
    with pytest.raises(PermissionError):
        with publication.staged_model(tmp_path, 'example') as stage:
            write_package(stage)
    assert len(calls) == 6
    assert sum(waits) == pytest.approx(3.1)
    assert not (tmp_path / 'example').exists()
    assert list((tmp_path / '.staging').iterdir()) == []


def test_collision_during_retry_preserves_other_model(tmp_path, monkeypatch):
    monkeypatch.setattr(publication.time, 'sleep', lambda _: None)

    def collision(stage, destination):
        destination.mkdir()
        (destination / 'sentinel').write_text('another writer')
        raise locked(32)

    monkeypatch.setattr(Path, 'rename', collision)
    with pytest.raises(FileExistsError):
        with publication.staged_model(tmp_path, 'example') as stage:
            write_package(stage)
    assert (tmp_path / 'example' / 'sentinel').read_text() == 'another writer'
    assert not (tmp_path / 'example' / 'model.onnx').exists()
    assert list((tmp_path / '.staging').iterdir()) == []


def test_unrelated_os_error_is_not_retried(tmp_path, monkeypatch):
    waits = []
    monkeypatch.setattr(publication.time, 'sleep', waits.append)

    def denied(stage, destination):
        raise OSError('disk failure')

    monkeypatch.setattr(Path, 'rename', denied)
    with pytest.raises(OSError, match='disk failure'):
        with publication.staged_model(tmp_path, 'example') as stage:
            write_package(stage)
    assert waits == []
    assert not (tmp_path / 'example').exists()
