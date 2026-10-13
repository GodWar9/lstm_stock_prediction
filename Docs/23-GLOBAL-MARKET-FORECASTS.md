# Global market forecasts

Updated 2026-10-13. The app supports recorded forecasts and on-demand forecasts
from completed Twelve Data stock/ETF OHLCV bars. All ingestion and inference run
in Rust. Python trains only from Rust-produced Arrow files.

## What you need to supply

| Setting | Where | Purpose |
| --- | --- | --- |
| `TWELVE_DATA_API_KEY` | Local ingestion shell and Render backend environment | Historical and fresh market bars |
| `QUANTCTL_BACKEND_TOKEN` | Render and Vercel server environment | Existing authenticated backend bridge |
| `QUANTCTL_BACKEND_URL` | Vercel server environment | Render service HTTPS URL |

There is no frontend market-data key. Never use a `VITE_` prefix for credentials.
Alpaca credentials remain optional for the separate US live-capture page; they
are not needed for Twelve Data forecasts. No database is required.

Create a key in your [Twelve Data account](https://twelvedata.com/account).
Its [market coverage and plan](https://twelvedata.com/pricing) determine which
exchanges are available and whether bars are real-time, delayed or end-of-day.
The integration does not turn a free key into free worldwide real-time access.
No subscription or hosting purchase is performed by this project. Confirm the
provider's display/redistribution terms before sharing a hosted data view.

The public provider demo key was used only to verify header authentication and
response identity. Your account, its exchange access, and live forecasting with
your key still require an acceptance check.

## Prepare a market model

Copy `configs/global-equity.yaml` to a new configuration. Use one instrument per
configuration. Set the provider's exact symbol, exchange, currency, IANA timezone
and type (`Common Stock` or `ETF`). The `data.symbols` value is a filesystem-safe
alias, e.g. `AAPL_NASDAQ` or `INFY_NSE`; it is separate from the provider symbol.
`data.exchange` must match `data.market.exchange`. Set unique dataset and model
versions for every import/training run; existing artifacts are never overwritten.

The example is daily NASDAQ AAPL, denominated in USD. For another exchange change
all identity fields together. Use the provider's returned exchange/timezone
spelling; mismatches are deliberately rejected. Set annualization to that market's
bars per year (daily example: 252; US regular-session minute example: 252 * 390).

From the repository root, with the key configured in the process environment:

```powershell
# Read a key without placing it in terminal history or printing it.
$marketKey = Read-Host 'Twelve Data API key' -AsSecureString
$env:TWELVE_DATA_API_KEY = [System.Net.NetworkCredential]::new('', $marketKey).Password
cargo build --manifest-path rust/Cargo.toml --locked --bin quantctl
```

Then run the following, stopping if any command fails:

```powershell
rust/target/debug/quantctl.exe --config configs/global-equity.yaml config validate
rust/target/debug/quantctl.exe --config configs/global-equity.yaml data ingest
rust/target/debug/quantctl.exe --config configs/global-equity.yaml data validate
rust/target/debug/quantctl.exe --config configs/global-equity.yaml features build
rust/target/debug/quantctl.exe --config configs/global-equity.yaml train
```

Use the repository Python environment through `QUANTCTL_PYTHON` if necessary,
as described in the main README. On Linux use `quantctl` without `.exe`.
Dates are start-inclusive and end-exclusive; minute date filters use UTC.
Historical imports are bounded to 5,000 bars. A request reaching this cap fails
with an instruction to narrow the date range instead of accepting truncated data.
Choose a range with enough rows for feature warmup, lookback, purge/embargo and
chronological train/validation/test splits. Training does not certify usefulness;
inspect the held-out validation metrics before relying on a model for research.

Set `bar_interval: 1m` and the matching annualization for minute models. Changing
the cadence requires a new dataset and a newly trained model. Crypto, forex and
instruments without integer traded volume are intentionally not supported by
this OHLCV model. There is no silent volume substitution.

## Use fresh forecasts

Start the backend with the key in its environment, open **Make a forecast**, and
select **Fresh global market bars**. Select the model and its corresponding
global dataset. The backend verifies the sealed model, training identity,
currency, exchange, cadence and feature annualization before requesting bars.
Demo and legacy models stay usable in recorded mode and cannot consume fresh
inputs. The reference dataset remains immutable; new snapshots live only in
bounded server memory and are identified by a SHA-256 in the forecast result.

The UI shows market currency, data timestamp, fetch time, age and feed delay.
Feed delay is always `unknown` until it can be independently established; a
recent timestamp is never labelled proof of real-time exchange entitlement.
The result is a point estimate, not a confidence interval or calibrated probability.

Runtime protections:

- Completed minute bars only; newest close must be within three minutes.
- Daily dates stay in exchange-local time and become eligible at the following
  local midnight. This conservative boundary is shown as the bar timestamp;
  it is not the exchange's actual session-close timestamp. Daily data older than
  four days is rejected in fresh mode.
- Holidays, halts, session gaps and exchange calendars are not independently
  certified. Horizons count observed bars. Closed/delayed feeds can fail the
  freshness check; switch explicitly to recorded mode to inspect older data.
- No silent fallback after a provider error, missing key, stale bar or quota failure.
- HTTPS with a sensitive Authorization header, redirects disabled, 20-second
  timeout, 4 MiB response limit, exact identity checks and OHLCV validation.
- One uncached request per minute across the process, a 120-second cache shared
  across users, at most 64 cached selections and 300 attempts per UTC day.
  Failures consume the budget. Freshness is rechecked on cache hits. These
  process-local counters reset on restart; they are not a provider billing cap
  and do not coordinate multiple replicas. Use one backend instance for this setup.

Historical REST bars can contain revisions. Availability timestamps estimate bar
completion, not first publication. All provider requests explicitly disable
corporate-action adjustments, so splits can cause jumps. Both training and fresh
inference use this same convention. The existing Alpaca journal workflow remains
the option for captured first-publication research.

## Prepare hosting later

Follow [deployment and rollback](22-DEPLOYMENT.md). Replace the synthetic bundle
with your new model, market dataset and research artifacts using the existing
bundle script, verify extraction, and rebuild the image. Training runs locally;
the free backend runs inference only. Add `TWELVE_DATA_API_KEY` as a secret in
Render after creating the service. Keep only the backend URL/token on Vercel.
The existing package continues to work without a market key in recorded mode.

After deployment verify:

1. `/api/market/status` through the authenticated bridge reports key presence.
   This does not validate the key or spend provider credits.
2. The model and dataset list the same market identity and cadence.
3. Recorded inference succeeds; then fresh inference succeeds during a supported
   session and shows the expected exchange/currency and recent timestamp.
4. An unavailable exchange or stale feed produces a clear error and no prediction.
5. Reload/restart preserves the bundled model and recorded forecasts. The next
   fresh request retrieves data again after the in-memory cache is lost.

Provider references: [API contract](https://twelvedata.com/docs),
[timezones](https://support.twelvedata.com/en/articles/5745849-timezones),
[historical queries](https://support.twelvedata.com/en/articles/5214728-getting-historical-data).
