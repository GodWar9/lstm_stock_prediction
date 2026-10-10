"""CI smoke against the actual Linux image, with real ONNX inference and restart."""
import json
import secrets
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

token = secrets.token_hex(32)
metadata = json.loads(Path("deploy/artifacts.tar.json").read_text())
container = None

def request(base, path, authenticated=True):
    headers = {"Authorization": "Bearer " + token} if authenticated else {}
    try:
        with urllib.request.urlopen(urllib.request.Request(base + path, headers=headers), timeout=30) as response:
            return response.status, response.read(), response.headers
    except urllib.error.HTTPError as error:
        return error.code, error.read(), error.headers

def ready(base):
    for _ in range(60):
        try:
            if request(base, "/api/health", False)[0] == 200:
                return
        except (urllib.error.URLError, TimeoutError):
            pass
        time.sleep(1)
    raise RuntimeError("Container did not become healthy")

try:
    container = subprocess.check_output(["docker", "run", "-d", "--memory=512m", "--cpus=0.5",
        "-p", "127.0.0.1::10000", "-e", "QUANTCTL_BACKEND_TOKEN=" + token, "lstm-backend:ci"], text=True).strip()
    port = subprocess.check_output(["docker", "port", container, "10000/tcp"], text=True).strip().split(":")[-1]
    base = "http://127.0.0.1:" + port
    ready(base)
    assert request(base, "/api/models", False)[0] == 401
    status, data, _ = request(base, "/api/models")
    assert status == 200 and json.loads(data)[0]["onnx_present"]
    query = urllib.parse.urlencode({key: metadata[key] for key in ("model", "dataset", "symbol")})
    status, data, _ = request(base, "/api/forecast?" + query)
    assert status == 200, data
    prediction = json.loads(data)
    assert prediction["implied_close"] > 0 and prediction["source"] == metadata["source"]
    status, data, _ = request(base, "/api/runs")
    assert status == 200 and json.loads(data)
    run = next(item for item in json.loads(data) if item["kind"] == "backtest")
    status, data, headers = request(base, "/api/runs/" + run["run_id"] + "/equity.arrow?max_points=100")
    assert status == 200 and data.startswith(b"ARROW1")
    assert headers["Content-Type"] == "application/vnd.apache.arrow.file"
    subprocess.run(["docker", "restart", container], check=True, stdout=subprocess.DEVNULL)
    ready(base)
    status, data, _ = request(base, "/api/forecast?" + query)
    assert status == 200 and json.loads(data)["implied_close"] == prediction["implied_close"]
    print("Container passes: 512 MiB limit, authentication, real inference, Arrow, restart recovery.")
finally:
    if container:
        subprocess.run(["docker", "logs", "--tail", "20", container], check=False)
        subprocess.run(["docker", "rm", "-f", container], check=False, stdout=subprocess.DEVNULL)
