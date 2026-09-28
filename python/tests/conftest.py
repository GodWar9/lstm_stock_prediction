"""The Python test suite must not depend on external network services."""
import socket
import pytest


@pytest.fixture(autouse=True)
def no_python_network(monkeypatch):
    def denied(*args, **kwargs):
        raise AssertionError("Network access is forbidden during offline Python tests")
    monkeypatch.setattr(socket, "create_connection", denied)
    monkeypatch.setattr(socket, "getaddrinfo", denied)
    monkeypatch.setattr(socket.socket, "connect", denied)
    monkeypatch.setattr(socket.socket, "connect_ex", denied)
