# Alpaca intraday ingestion

The `/live` page receives continuously updated trade snapshots from a single
server-owned Alpaca stock WebSocket. It works without any saved backtest or model.
Rust authenticates, subscribes to trades, quotes, minute bars and updated bars,
and writes provider events to `datasets/live/<session UUID>.ndjson` under the
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
- Maximum 30 configured symbols, 1 MiB upstream frame/batch, and 256 MiB per session
  journal. Disk errors or a full journal stop ingestion visibly. Journals accumulate
  across restarts and need archival. Records are written through the OS; there is
  no per-event fsync guarantee against sudden power loss.
- Journals are a separate raw intraday data source. Existing backtests and Monte
  Carlo reports remain historical research artifacts. Converting these records
  into model inputs requires interval-specific bar construction, corrections,
  gap policy, point-in-time validation, and models trained on that same interval.
  No live orders are submitted and no existing daily model is fed raw ticks.

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
