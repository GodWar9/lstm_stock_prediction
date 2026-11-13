# Public market terminal

## Zero-cost public configuration

The public site uses Coinbase for live spot crypto and Frankfurter v2 for daily forex reference history. No API key or paid subscription is needed for these paths. Stocks retain the searchable dated catalogue; stock price/history access remains disabled without an appropriately licensed provider. The Twelve Data Basic account is not used for public prices or derived forecasts.

`GET /api/markets/reference?symbol=EUR/USD` requests one year of blended daily reference observations from the fixed Frankfurter endpoint. It validates the requested pair, response identity, positive rates, date bounds and duplicate dates. Requests coalesce and cache for six hours (up to 128 cached provider requests per process). The dashboard shows source date, a stale warning beyond seven days, and calculated change, low, high and mean from up to 30 observations. Unsupported pairs and upstream failures show an unavailable state. The reference catalogue is broader than guaranteed free history coverage.

Reference rates have no fabricated open/high/low/volume fields. The line chart uses calendar dates and connects observed points without filling missing days. These descriptive metrics are separate from LSTM inference. The existing deployed LSTM still uses its named synthetic demonstration dataset; adding an FX reference chart does not train a new model or validate its predictions. Fresh real-market LSTM training remains separate work requiring suitable historical training inputs and validation.

Source and usage: [Frankfurter v2 documentation](https://frankfurter.dev/) states that blended rates are free for commercial use and caching is encouraged. Its rates are references, not executable quotes. [Twelve Data usage](https://support.twelvedata.com/en/articles/5332349-commercial-and-personal-usage) distinguishes personal/internal use from public display. No plan changes, purchases, or credential transfers are needed for this configuration.

Validation adds reference-series identity/date checks, caching, coalescing, empty/error/stale states and desktop/mobile browser tests for analytics, date ranges, accessible observations and clearing data on a failed pair switch.

Open `/terminal` for active Coinbase spot pairs, global securities catalogues, and a backend-connected forecast workspace. Public crypto and reference listings require no account or credentials. This is spot crypto market observation, separate from the equity model and Alpaca ingestion pages.

The browser subscribes to the public Exchange WebSocket `ticker` and `heartbeat` channels. Connections without messages for 15 seconds are closed and retried with exponential backoff capped at 30 seconds. Quotes show exchange timestamps and become stale after 30 seconds without a trade or 15 seconds without receipt; connection loss also clears the live indicator.

REST snapshots fetch the selected product's top 10 bid/ask levels and latest 30 trades every five seconds. The tape labels Coinbase's **maker side**, not aggressor side. Minute candles refresh every 60 seconds, show at most 300 observations, and may include an unfinished candle. Missing intervals are not filled. Snapshot requests have a ten-second timeout, never overlap within a panel, abort on product changes/unmount, and show failures/staleness instead of simulated values. Data stays in browser memory; this is not a durable, complete trade archive.

The CSP in the Rust server, container server and frontend hosting configuration permits only the public Coinbase REST and WebSocket hosts in addition to self. No keys, trading actions, arbitrary upstream proxy or server credential changes are introduced.

Validation: `npm test` and `npm run test:e2e` in `web`. Terminal browser tests mock the exchange to check live rendering, product switches, failures and desktop/mobile sizing. Separately verify the public feed against an actual running website; deterministic tests do not prove exchange availability.

Protocol references: [WebSocket channels](https://docs.cdp.coinbase.com/exchange/websocket-feed/channels), [product candles](https://docs.cdp.coinbase.com/api-reference/exchange-api/rest-api/products/get-product-candles), [product book](https://docs.cdp.coinbase.com/api-reference/exchange-api/rest-api/products/get-product-book).


## Global catalogues and dashboard forecasts

The terminal now uses the public Coinbase product catalogue instead of a fixed three-pair universe. Search and quote-currency filtering cover active spot products; only core pairs and the selected product are streamed. Twelve Data stock and forex reference catalogues are served by the Node container through `/api/markets/catalog`, filtered and paginated to 50 results. A listing is not a promise of price entitlement, and these catalogues do not include every financial security worldwide.

`/api/markets/status` reports availability without exposing credentials. `/api/markets/snapshot` supplies validated UTC OHLC history for selected symbol/exchange/interval. Enable it only with `TWELVE_DATA_API_KEY` and `TWELVE_DATA_PUBLIC_DISPLAY=1` in server secret settings after confirming the relevant display permissions. Never use a `VITE_` key or the provider's demo key. The same opt-in passes the key to the private Rust process for its existing compatible-model fresh forecasting path. Existing synthetic models cannot use fresh global inputs.

The packaged reference catalogue has an explicit retrieval date; the network fallback caches for 12 hours; paid history caches for 60 seconds, coalesces concurrent identical calls, and limits requests to 4/minute and 400/day per process. These process-local limits are not account-wide limits across scaled instances. Large deployments require a shared budget/cache. Node markets endpoints are provided by the unified container and Vite development middleware; the standalone Rust inspector retains its prior market APIs.

The dashboard forecast workspace calls real `/api/models`, `/api/datasets`, `/api/runs` and `/api/forecast` endpoints. A forecast uses its explicitly selected research dataset, not whichever exchange ticker happens to be open. Synthetic source, data time and model horizon remain visible. The separate full forecast page retains recorded/fresh/live input controls and identity checks.

Validation includes backend catalogue pagination, credential isolation, request limits, error redaction, desktop/mobile search, interval switching, unavailable-price states and dashboard inference. Provider prices and catalogue availability are also checked separately against the actual service.


Global reference data is packaged as `web/data/securities.json.gz` with a retrieval timestamp and source. Refresh explicitly with `node web/scripts/refresh-market-catalog.mjs`; review the count and commit the new snapshot. The November 13, 2026 snapshot contains 211,794 stock listings and 1,399 forex pairs. Search operates on this dated snapshot without provider credentials or a large cold-start network request. The original response was approximately 56 MB; the packaged normalized catalogue is approximately 2.4 MB compressed. The API exposes the catalogue timestamp. Counts describe listings, not distinct companies or guaranteed real-time price coverage.
