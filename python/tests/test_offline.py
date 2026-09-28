"""Network acquisition must fail before importing an online provider."""
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from python.ml.data_fetch_helper import fetch_actions, fetch_ohlcv, require_network_permission
from python.ml.train_orchestrator import load_config


@pytest.mark.parametrize("permission", [None, "0", "false", "true", "yes", ""])
def test_online_acquisition_requires_explicit_permission(monkeypatch, permission, tmp_path):
    monkeypatch.delenv("QUANTCTL_ALLOW_NETWORK", raising=False)
    if permission is not None:
        monkeypatch.setenv("QUANTCTL_ALLOW_NETWORK", permission)
    with pytest.raises(RuntimeError, match="Network acquisition is disabled"):
        require_network_permission()
    output = tmp_path / "bars.csv"
    for fetch in [fetch_actions, fetch_ohlcv]:
        with pytest.raises(SystemExit) as error:
            fetch("AAPL", "2020-01-01", "2021-01-01", str(output))
        assert error.value.code == 1
    assert not output.exists()


def test_missing_training_configuration_fails_closed(tmp_path):
    with pytest.raises(FileNotFoundError, match="configuration not found"):
        load_config(str(tmp_path / "missing.yaml"))
