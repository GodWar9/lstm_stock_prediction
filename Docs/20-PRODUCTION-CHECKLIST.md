# Production readiness checklist

Baseline: `4c004f0`. Started 2026-09-28. Scope: local Alpaca market-data ingestion
and research UI, with downstream research/trading gates tracked separately.
**Release decision: NOT READY** until the applicable unchecked gates below pass.
An implemented feature is not evidence of an authenticated production deployment.

Check an item only after its acceptance test passes. Record the test or artifact
beside it. `[ ]` means incomplete or unverified, including external dependencies.

## A. Existing foundation (verified at baseline)

- [x] Server-owned TLS connection; credentials excluded from API responses and journals. `live::tests::provider_rejection_is_terminal_and_redacts_upstream_text`.
- [x] Browser works without saved backtests. Playwright `unconfigured Rust feed explains setup without requiring saved runs`.
- [x] Late trades cannot replace newer displayed prices. `websocket_authenticates_subscribes_persists_and_preserves_event_order`.
- [x] Missing credentials and delayed/test feeds are explicit. Playwright live suite.
- [x] Snapshot-only browser delivery and bounded provider message size. API source plus baseline tests.

## B. Protocol and service lifecycle

- [ ] Validate configuration with pure tests: feeds, symbols, credentials and bounds.
- [ ] Confirm every requested channel, after authentication; reject incomplete/out-of-order control messages.
- [ ] Separate terminal HTTP/auth/protocol failures from retryable network/provider failures; redact provider error text.
- [ ] Bound connect, authenticate, subscribe, send, heartbeat and disk waits.
- [ ] Test actual disconnect → reauthenticate → resubscribe → new data; record the gap.
- [ ] Graceful Ctrl+C/SIGTERM drains the journal and stops SSE clients; no indefinite shutdown.

## C. Data integrity and storage

- [ ] Validate subscribed trade/quote/bar/correction/cancel payloads before recording; reject impossible timestamps and malformed prices.
- [ ] Version journals and sequence records; include session/feed/symbol metadata and explicit gap/stop records.
- [ ] Rotate journal segments without silently losing data; bound storage across sessions and fail visibly on exhaustion.
- [ ] Define and test durability checkpoints and clean shutdown; expose the durable sequence.
- [ ] Read-only journal verifier detects malformed/truncated records, sequence discontinuities and unclean sessions.

## D. Operations and browser reliability

- [ ] Liveness/readiness API distinguishes process availability, provider connectivity, market freshness and persistence failure.
- [ ] Diagnostics expose connection attempts, last message time, journal usage and durable progress without secrets.
- [ ] Browser validates snapshot payloads; stale/error states survive malformed messages and future timestamps.
- [ ] Test navigation cleanup, reconnect, multiple clients and shutdown using actual SSE transport.
- [ ] Document launch, restart, key rotation, disk-full recovery, journal verification and rollback.
- [ ] Add a repeatable local acceptance command and CI gates for the new checks.

## E. Release verification

- [ ] Rust formatting, Clippy and full workspace suite pass on final changes.
- [ ] Python contract/leakage/parity suite passes on final changes.
- [ ] Frontend unit tests, production build, OpenAPI drift check and desktop/mobile Playwright pass.
- [ ] Review final diff for credentials, accidental fixtures and misleading readiness claims.

## F. Deployment acceptance (requires configured account/host)

- [ ] Authenticate against Alpaca `test` with FAKEPACA; save redacted evidence and journal verification output. **Pending local credentials.**
- [ ] Validate selected IEX/SIP subscription during market hours, including reconnect behavior and timestamp freshness. **Pending account entitlement and market session.**
- [ ] Sustained target-symbol soak test: measure peak event throughput, durable-write latency, memory, reconnects and disk growth on the deployment host. Set an explicit capacity/SLO from measured results.
- [ ] Configure OS service supervision, restricted credential storage, disk/connection alerts and retention/backup ownership on the actual host.
- [ ] Exercise restart, interrupted journal recovery and backup restore on the deployment filesystem.

## G. Gates before intraday model research or trading

These remain separate from a market-data capture release; do not label this system
a production trading engine while they are open.

- [ ] Define bar interval, session calendar, trade-condition filters, deduplication, corrections and gap/backfill policy.
- [ ] Convert captured events to versioned PIT datasets with receipt-time availability and replay tests.
- [ ] Train/evaluate interval-matched models using purged walk-forward validation and Python/Rust parity.
- [ ] Wire fresh intraday datasets through backtesting/simulation with matching annualization, fees and slippage; demonstrate new results derived from captured data.
- [ ] If execution is requested: paper broker adapter, idempotent orders, reconciliation, exposure limits, kill switch and recovery drills.

## Evidence log

- Baseline `4c004f0`: 169 Rust reported passes (including doctests), 38 Playwright,
  4 frontend unit tests; Clippy and API contract check passed. These are baseline
  results, not completion evidence for changes made after this checklist began.
