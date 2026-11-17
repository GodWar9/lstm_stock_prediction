# Human operated Alpaca session

Status: real-provider verification is UNVERIFIED. Local protocol tests do not establish account entitlement, host reliability or model quality. This application does not submit paper or live orders; this session exercises capture and inference with the human's Alpaca account.

## Before the session

Read CLAIMS.md and BLOCKED.md. Build current web assets, then the Rust executable. Use a standalone `quantctl serve` process: the public container deliberately removes Alpaca credentials. Put `APCA_API_KEY_ID` and `APCA_API_SECRET_KEY` in that process's environment through your secret manager; never paste them into chat, commands saved as evidence, the browser or Git.

Confirm the account's current IEX or SIP market-data entitlement with Alpaca. Paper-account access does not automatically prove SIP entitlement. First verify the `test` feed with `FAKEPACA`; test and delayed feeds must be refused by live forecasting. Restart with the intended real symbol and `iex` or entitled `sip` for the market session.

US regular equities hours are 09:30–16:00 New York time. In IST that is 19:00–01:30 during US daylight time and 20:00–02:30 during US standard time, ending the next calendar day. Confirm the exchange calendar, holidays and early closes for the actual session; these windows are not a promise the exchange is open. The November standard-time window is approximately 8 pm to 2:30 am IST.

```powershell
$env:QUANTCTL_LIVE_SYMBOLS = 'AAPL'
$env:QUANTCTL_ALPACA_FEED = 'iex'
.\rust\target\debug\quantctl.exe serve --root . --port 8787
```

Check `/live`, `/api/live/ready` and `/api/live`. Save redacted snapshots showing feed, subscription, exchange/receipt timestamps, connection state, durable sequence and journal path. `/api/health` alone is insufficient.

## Capture and forecast

Have a sealed minute model and dataset trained on the same symbol and first-publication feed. Do not use the synthetic demo model. If no compatible model exists, collect sufficient history, stop gracefully, verify the closed capture, and run the documented research workflow before a later live session. A short session may not satisfy training and fold warmup.

On `/forecast`, choose **Live streaming price feed**. Wait for sufficient contiguous completed regular-session bars, then request a forecast. Save the full JSON response, model/package hash, dataset/input hash, provenance, as-of/availability time and the human's observation timestamp. Verify that only a fresh completed minute is accepted, not the latest tick or an unfinished bar.

## Controlled failures

Do these only in the isolated research process; no orders are involved. Save before/after snapshots and the exact action without secrets.

- Pause or disconnect networking briefly: retained prices must become stale, the inference window and cached forecast must clear, and reconnect must reauthenticate and resubscribe. Missed minutes must not be silently filled.
- Request while the stream is disconnected, empty or older than the freshness threshold: expect rejection, not a historical result labelled live.
- Restart with test/delayed feed or an unsubscribed symbol: expect explicit refusal. Restore the valid configuration afterward.
- Send Ctrl+C and confirm graceful exit. Restart as a new session; do not rewrite the old capture or fabricate a stop record.
- Do not manufacture exchange revisions or malformed market events in a real-provider journal. Those cases belong in the deterministic local tests.

## Close and retain evidence

After graceful shutdown:

```powershell
.\rust\target\debug\quantctl.exe data verify-capture --journal datasets/live/SESSION_ID
```

Keep the full immutable journal outside Git, its checksum manifest, verification stdout, redacted API responses, requested forecast JSON, command/commit/date record, network failure timeline, host resource and disk measurements, and any errors. Store only redacted evidence summaries in `Docs/evidence/live-session/`. Scan evidence for secrets before committing. Assign retention and backup ownership, verify an archive copy, and test restore in an isolated workspace.

The reviewer may mark only the exercised gates verified. A successful connection does not establish real-data profitability, production load capacity, backup recovery, or complete market coverage. Failed or unclean captures remain evidence; do not import them as valid research data.

Primary references checked for this runbook: [NYSE trading information](https://www.nyse.com/trade/trading-information), [NYSE holidays and hours](https://www.nyse.com/trade/hours-calendars), and [Alpaca paper trading market-data entitlement](https://docs.alpaca.markets/us/docs/paper-trading).
