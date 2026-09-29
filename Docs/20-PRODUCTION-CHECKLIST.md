# Production readiness checklist

Baseline: `4c004f0`. Started 2026-09-28; scope clarified 2026-09-29:
end-to-end intraday research and backtesting, with public website access and
forecast generation. Automated order execution is outside this release.
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

- [x] Validate configuration with pure tests: `configuration_and_error_classification_are_explicit`.
- [x] Confirm every requested channel after authentication: `incomplete_subscription_fails_instead_of_claiming_live` and WebSocket protocol test.
- [x] Classify terminal/retryable errors and redact upstream text: configuration/classification and provider rejection tests.
- [x] Bound network/disk waits: `live::session`, `send`, `Journal::write`; protocol timeout paths and capacity tests.
- [x] Reconnect → authenticate → resubscribe → new data and gap record: `reconnect_resubscribes_and_shutdown_seals_the_capture`.
- [ ] Verify OS Ctrl+C/SIGTERM on deployment host. Controller shutdown and SSE termination pass deterministic Rust tests; process supervision acceptance remains open.

## C. Data integrity and storage

- [x] Validate market events and timestamps: `quant_data::capture` and invalid-trade API tests.
- [x] Version/sequence/hash-chain journals with session, gap and stop records: capture verification tests.
- [x] Rotate segments, bound total storage and fail visibly: `rotation_chain_durability_lock_and_clean_shutdown`, `capacity_and_backward_clock_fail_closed`.
- [x] Checkpoint durability and expose sequence: journal tests and `/api/live` diagnostics.
- [x] Detect truncation, corruption, sequence gaps and unclean sessions: `quant_data::capture` tests; `data verify-capture`.

## D. Operations and browser reliability

- [x] Separate liveness/capture readiness and symbol freshness: `readiness_and_sse_clients_follow_state_and_shutdown`.
- [x] Expose attempts, message time, storage and durable progress: API snapshot and Live diagnostics.
- [x] Validate browser snapshots and future/stale timestamps: frontend `live.test.ts` (13 total frontend unit tests including format tests).
- [ ] Test navigation cleanup, reconnect, multiple clients and shutdown using actual SSE transport.
- [x] Document operations and capture-to-research workflow: `Docs/19-LIVE-INGESTION.md` (host drills remain in F).
- [ ] Add a repeatable local acceptance command and CI gates for the new checks.

## E. Release verification

- [x] Rust formatting, Clippy and full workspace suite: 180 reported passes including 2 doctests, 2026-09-29. Two legacy provider tests return early without a sealed `lstm_v1`; the capture integration independently runs real ONNX inference.
- [x] Python contract/leakage/parity suite: 49 passed, 2026-09-29.
- [x] Frontend: 13 unit tests, production build, OpenAPI drift check and 46 desktop/mobile Playwright cases pass, 2026-09-29.
- [x] Final diff reviewed: no credentials or generated model/data fixtures tracked; historical/synthetic forecast labels and open deployment gates retained.

## F. Deployment acceptance (requires configured account/host)

- [ ] Authenticate against Alpaca `test` with FAKEPACA; save redacted evidence and journal verification output. **Pending local credentials.**
- [ ] Validate selected IEX/SIP subscription during market hours, including reconnect behavior and timestamp freshness. **Pending account entitlement and market session.**
- [ ] Sustained target-symbol soak test: measure peak event throughput, durable-write latency, memory, reconnects and disk growth on the deployment host. Set an explicit capacity/SLO from measured results.
- [ ] Configure OS service supervision, restricted credential storage, disk/connection alerts and retention/backup ownership on the actual host.
- [ ] Exercise restart, interrupted journal recovery and backup restore on the deployment filesystem.

## G. Intraday research gates

These are part of the requested research release. Production trading is outside
scope; synthetic protocol acceptance does not establish market performance.

- [x] Define first-publication provider minute bars, regular sessions, duplicate rejection, correction and gap policy. Provider bar construction is trusted; this does not reconstruct trade-condition-filtered bars from raw trades.
- [x] Import versioned PIT datasets with receipt-time availability: `capture` replay/integrity tests.
- [x] Train interval-matched purged walk-forward models and ONNX parity: captured-minute integration plus Python suite.
- [x] Run fresh captured-data backtest/simulation with minute annualization and configured costs: `captured_minutes_train_fresh_model_backtest_and_simulation` (synthetic protocol fixture, real training/inference).
- [ ] Validate slippage assumptions and out-of-sample behavior on actual captured market data, beyond synthetic acceptance.

## H. Public forecast website

- [x] Browser forecast selection, missing-data errors, interval mismatches, historical labels and input-change reset: eight desktop/mobile forecast cases, 2026-09-29.
- [x] Real ONNX forecast from a freshly trained minute model, timestamp/return/provenance assertions and incompatible interval rejection: CLI capture integration, 2026-09-29.
- [ ] Wire open live capture into cadence-matched model features with warmup, gap and freshness gates. Current forecasts consume recorded datasets.
- [ ] Add authenticated hosted research job execution/status; current research launch is CLI-only.
- [ ] Select hosting/domain/storage; configure authenticated HTTPS access and service supervision. **Owner: hosting details to follow.**
- [ ] Deploy and test the public URL, access controls, SSE, restart and restore on the target host.

See `Docs/21-WEB-FORECAST-DEPLOYMENT.md` for the exact current forecast scope.

## Evidence log

- Baseline `4c004f0`: 169 Rust reported passes (including doctests), 38 Playwright,
  4 frontend unit tests; Clippy and API contract check passed. These are baseline
  results, not completion evidence for changes made after this checklist began.
- 2026-09-29 after GitHub sync at `5a0b039`: 180 Rust reported passes (178
  unit/integration plus 2 doctests), 49 Python, 13 frontend unit and 46 Playwright.
  Clippy, formatting, production build and contract drift check passed. Logs are
  local ignored `audit-forecast-*.log` files. The repeated-capture acceptance test
  now varies its training seed so retained test-evaluation guards do not collide;
  production reuse protection remains enabled.
- Local HTTP acceptance on port 8787 returned an integrity-checked minute forecast
  for the fresh `research_9758fa4a95bf401190844360f0f3a404` synthetic capture model.
  This proves inference execution, not predictive quality or live market access.
