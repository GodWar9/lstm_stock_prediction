# Forecast website release scope

Updated 2026-09-29 after syncing GitHub main at `5a0b039`.

The requested target is end-to-end intraday research and backtesting, with a
public website that can generate forecasts. Automated broker order execution is
outside this release. Hosting details are pending from the owner.

## Implemented browser path

`/forecast` works independently of saved backtests. It lists `/api/models` and
`/api/datasets`, then calls `/api/forecast?model=ID&dataset=ID&symbol=SYMBOL` only
when the user requests a forecast. No training or file mutation occurs in this
request. Four bounded blocking workers are shared with artifact requests; excess
work receives 503 rather than creating an unlimited inference queue.

`forecast::run` checks root containment, file sizes, the sealed model package,
market-data hash, timestamp order, feature version, target transformation and bar
interval. `quant_features::standard_graph` is shared with CLI training and
prediction. The backend returns log return, converted simple return and implied
close, with the actual market/availability timestamps and provenance hashes.
Models without complete integrity manifests need retraining.

Recorded forecasts can be historical, synthetic or overlap training periods.
These are stated in the response and UI. Training-symbol compatibility and
out-of-sample status are not certified by this endpoint. A fresh request does not
make an old dataset current. Neither a probability nor an uncertainty interval is
invented from an LSTM point estimate.

## Gates still open

- Public hosting: choose host, domain and persistent storage. The application
  intentionally still binds loopback and rejects remote Host headers.
- Put an authenticated HTTPS gateway in front of the private research workspace.
  A proxy on the same host can route to loopback and supply the accepted upstream
  Host header after authenticating the client. Do not weaken the application Host
  checks or expose filesystem artifacts directly to make a deployment work.
- Configure Alpaca credentials and data entitlements only on the server. Verify
  whether the intended website audience is permitted to access the chosen feed.
- Streaming forecasts: add a bounded, cadence-matched feature buffer, warmup,
  first-publication policy, gap handling and a model/data freshness gate. The
  current live price table is not wired into `/api/forecast`.
- Browser research execution: the capture-to-training/backtest/simulation workflow
  currently runs through `quantctl research`. A hosted job queue needs explicit
  authorization, concurrency/storage limits, cancellation and durable status.
- Deployment acceptance: test authenticated browser access, unauthorized denial,
  TLS, provider reconnect, process restart, restore, disk exhaustion, SSE proxy
  behavior and sustained load on the actual host. No public deployment is claimed
  from localhost or mocked Playwright results.

## Local verification

Build the frontend, rebuild `quantctl`, and run `quantctl serve --root . --port
8787` from the repository root. Open `http://127.0.0.1:8787/forecast`.
See [the ingestion runbook](19-LIVE-INGESTION.md) for generating a new captured
dataset and matching model. Tests distinguish browser response rendering from
real ONNX inference: the CLI capture integration test trains a new model before
calling the same inference service the API uses.
