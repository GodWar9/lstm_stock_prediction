# Public market terminal

Open `/terminal` for BTC-USD, ETH-USD and SOL-USD on Coinbase Exchange. No account or credentials are required. This is spot crypto market observation, separate from the equity model and Alpaca ingestion pages.

The browser subscribes to the public Exchange WebSocket `ticker` and `heartbeat` channels. Connections without messages for 15 seconds are closed and retried with exponential backoff capped at 30 seconds. Quotes show exchange timestamps and become stale after 30 seconds without a trade or 15 seconds without receipt; connection loss also clears the live indicator.

REST snapshots fetch the selected product's top 10 bid/ask levels and latest 30 trades every five seconds. The tape labels Coinbase's **maker side**, not aggressor side. Minute candles refresh every 60 seconds, show at most 300 observations, and may include an unfinished candle. Missing intervals are not filled. Snapshot requests have a ten-second timeout, never overlap within a panel, abort on product changes/unmount, and show failures/staleness instead of simulated values. Data stays in browser memory; this is not a durable, complete trade archive.

The CSP in the Rust server, container server and frontend hosting configuration permits only the public Coinbase REST and WebSocket hosts in addition to self. No keys, trading actions, arbitrary upstream proxy or server credential changes are introduced.

Validation: `npm test` and `npm run test:e2e` in `web`. Terminal browser tests mock the exchange to check live rendering, product switches, failures and desktop/mobile sizing. Separately verify the public feed against an actual running website; deterministic tests do not prove exchange availability.

Protocol references: [WebSocket channels](https://docs.cdp.coinbase.com/exchange/websocket-feed/channels), [product candles](https://docs.cdp.coinbase.com/api-reference/exchange-api/rest-api/products/get-product-candles), [product book](https://docs.cdp.coinbase.com/api-reference/exchange-api/rest-api/products/get-product-book).
