# Alpaca intraday ingestion

The `/live` page receives continuously updated trade snapshots from a single
server-owned Alpaca stock WebSocket. It works without any saved backtest or model.
Rust authenticates, subscribes to trades, quotes, minute bars and updated bars,
and writes provider events to `datasets/live/<session UUID>/000001.ndjson` under the
`quantctl serve --root` directory. API keys never enter the browser.

## Run locally

Build the UI first, then rebuild the executable so its embedded assets are current:

```powershell
cd web
npm ci
npm run build
cd ..
cargo build --manifest-path rust/Cargo.toml --bin quantctl --locked
```

Set `APCA_API_KEY_ID` and `APCA_API_SECRET_KEY` securely in the environment of the
server process. Do not commit credentials, put them in browser fields, or paste
them into chat. Then select explicit symbols and a feed:

```powershell
$env:QUANTCTL_LIVE_SYMBOLS = 'AAPL,MSFT'
$env:QUANTCTL_ALPACA_FEED = 'iex'
.\rust\target\debug\quantctl.exe serve --root . --port 8787
```

Open `http://127.0.0.1:8787/live`. Setting `QUANTCTL_LIVE_SYMBOLS` opts into the
network connection; leaving it unset disables ingestion. The historical Yahoo
adapter's `QUANTCTL_ALLOW_NETWORK` setting does not control this explicit live feed.
Symbols and credentials are read at startup; restart to change them.

Supported feeds: `iex` (default, one exchange), `sip` (consolidated, requires
entitlement), `delayed_sip` (15-minute delay), and `test` (Alpaca test data).
For an outside-market-hours protocol check, select `test` with the single symbol
`FAKEPACA`. This still requires valid Alpaca credentials. See the official
[stock stream](https://docs.alpaca.markets/us/docs/real-time-stock-pricing-data)
and [authentication protocol](https://docs.alpaca.markets/us/docs/streaming-market-data).

## Behavior and limits

- One upstream connection serves all browser tabs. The browser receives bounded
  snapshots once per second at `/api/live/events`; `/api/live` returns a snapshot.
  These endpoints do not expose a lossless tick stream.
- The table shows latest observed trades, not executable quotes. Older trade
  timestamps cannot replace newer displayed prices. Journal entries preserve
  receipt order, exchange timestamps, local receipt milliseconds and feed identity.
- Quotes, minute bars, updates, corrections and cancellations are journaled;
  corrections are not applied to the latest-trade table. There is no consolidated
  order book, trade-condition filtering, or automatic historical backfill.
- Ping/pong checks detect silent transport loss. Reconnects use exponential delays
  up to 30 seconds and reauthenticate/resubscribe. Gap markers record interrupted
  connections; missed events remain missing. Auth/entitlement/configuration errors
  stop the feed with a visible error instead of substituting synthetic data.
- A trade becomes stale after 30 seconds based on exchange time or receipt time.
  An interrupted upstream or browser connection also makes retained prices stale.
  Closed markets and illiquid stocks can be stale while the connection is healthy.
- Maximum 30 configured symbols and 1 MiB upstream frame/batch. Journals rotate
  at 16 MiB; the live directory has a 2 GiB total limit across sessions. A workspace
  lock prevents competing capture writers. Disk or capacity errors stop ingestion
  visibly. There is no automatic deletion: archive closed sessions deliberately.
- Schema-one records carry a session ID, sequence, receipt time and SHA-256 chain.
  One-second checkpoints call `sync_all`; `durable_seq` reports the checkpointed
  prefix. A hard crash can lose the uncheckpointed tail. Graceful shutdown appends
  and syncs a stop record. Do not repair an incomplete capture by inventing a stop.
- Closed captures can now produce new datasets, models, backtests and simulations
  through `quantctl research`. Import takes the first original regular-session
  minute bar, uses its close time as market time and receipt time as availability,
  and rejects gaps, duplicates, late arrivals and unclean sessions. Updated bars
  remain in the raw journal but cannot revise already available model inputs.
  Research accepts IEX/SIP captures only; test/delayed feeds are not admitted.

## From capture to research and forecasts

Stop capture gracefully with Ctrl+C, then use the session directory shown on the
Live page. Set the dates and single symbol in `configs/intraday.yaml` to the actual
capture coverage. Collect sufficient history for every walk-forward fold; a few
minutes cannot train a meaningful model or even satisfy validation warmup.

```powershell
.\rust\target\debug\quantctl.exe data verify-capture --journal datasets/live/SESSION_ID
.\rust\target\debug\quantctl.exe --config configs/intraday.yaml research --journal datasets/live/SESSION_ID --paths 100
.\rust\target\debug\quantctl.exe serve --root . --port 8787
```

The research command writes a unique dataset/model version and records status,
resolved configuration and logs under `reports/research/research_ID`. A failed
stage retains its evidence; it does not publish a successful result or overwrite
an existing version. Minute metrics use 98,280 periods/year, not 252 daily periods.

Open `/forecast`, select the resulting model and matching dataset, and generate
a prediction. Rust verifies hashes and interval compatibility, reconstructs the
training feature graph, applies the fitted scaler, and runs ONNX inference. The
response includes data availability time, horizon, source and package hashes.
The implied close is `last_close * exp(predicted_log_return)`; it is not a
confidence bound. This is a recorded-dataset forecast, not automatic inference
from the currently open capture. Historical and synthetic inputs are labeled.

## Operations

`/api/health` reports process liveness. `/api/live/ready` returns 503 unless the
capture is connected, has a recent provider message and has checkpointed data;
its separate fresh/stale symbol lists describe market freshness. Neither endpoint
certifies a model or a trading signal. Diagnostics appear on `/live`.

To rotate keys or symbols, stop gracefully, update the service environment and
restart; credentials are read only at startup. On disk-full errors, stop, verify
and archive closed sessions to separate storage, confirm the archive checksums,
then free space under the host's retention policy and restart. Preserve failed
captures for diagnosis; the research importer deliberately refuses them. Restore
archives into an isolated workspace and verify before import. Roll back the
binary and matching embedded UI together; never rewrite sealed model/dataset
versions. Host-specific supervision, alerting and backup restore drills remain
release gates in the production checklist.

## Verification

```powershell
cargo test --manifest-path rust/Cargo.toml -p quant_api --locked
cd web
npm test
npm run test:e2e -- --workers=2
```

The Playwright server builds current assets and the Rust executable, uses a fresh
fixture directory, and removes provider credentials from its child environment.
Browser tests cover disabled setup, incoming price updates, delayed timestamps,
stale transport, disconnect/reconnect display, provider errors and feed labels on
desktop and mobile. Rust tests use an actual local WebSocket peer to verify auth,
subscriptions, persisted receipt order, malformed trades and failure handling.
These deterministic tests do not claim connectivity or entitlement to real Alpaca
markets; that needs a separate run with the user's configured account.
