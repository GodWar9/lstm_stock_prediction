# Project handover and hardening acceptance

Prepared on 17 November 2026. Branch: `hardening/interview-ready`. Comparison baseline: `main` at `e8dc7d0`. This document supersedes the state assessment in [the earlier handover](HANDOVER-2026-11-16.md), which remains historical evidence. Nothing has been merged into main by this task.

## Current project and drift from main

The baseline already contains Rust ingestion/features/inference, Python model training, the React inspector, live Alpaca forecast inputs and newer market observation features. The hardening branch concentrates on research validity and evidence: explicit provenance, mutation-checked leakage controls, stronger CI, a frozen study protocol and runner, benchmark tooling, live-path tests, decision logging, drift monitoring, optional signatures and reconciled documentation. It adds no live order execution or new market dashboard scope.

Compared with the original Python forecasting prototype, the architectural drift is a deliberate expansion into a research platform. The unresolved product gap is empirical: a validated real-data model and honest comparison with simpler baselines have not yet been delivered. The packaged demonstration remains synthetic. More runtime infrastructure does not establish predictive advantage.

## Phase acceptance

| Phase | Status | Evidence | Deviations and remaining acceptance |
|---|---|---|---|
| 0 — preflight | DONE | [Baseline evidence](evidence/phase0/), [known issues](KNOWN-ISSUES.md) | Sandbox network/loopback failures retained; unrestricted reruns passed. Remote fetch succeeded. |
| 1 — claims and documentation | DONE | [Claims ledger](CLAIMS.md), [live ingestion](19-LIVE-INGESTION.md), Git history | Historical design examples explicitly remain historical; host/provider gates remain unchecked. |
| 2 — provenance | DONE | [API, UI and packaged acceptance](evidence/phase2/) | Synthetic labels derive from model and dataset sources; this does not validate real model quality. |
| 3 — CI | PARTIAL | [Published green run](evidence/phase3/github-green-20261117T082144099685Z.txt), [workflow](../.github/workflows/ci.yml) | First published revision passed; final hardening revision still requires its own GitHub run. |
| 4 — validity canaries | DONE | [Mutation and restored-suite evidence](evidence/phase4/), [limits](VALIDITY-CANARIES.md) | Controls use synthetic test data; no synthetic performance results are presented. |
| 5 — real-data study | BLOCKED | [Frozen protocol and guard tests](evidence/phase5/), [study status](STUDY-REPORT.md) | Human protocol review pending; no acquisition/fitting or final evaluation. PBO/CSCV remains unavailable. |
| 6 — inference comparison | BLOCKED | [Compilation and provenance refusal](evidence/phase6/), [benchmark command](BENCHMARKS.md) | Tooling exists; a sealed real-data model is needed before measurement. |
| 7 — live path | DONE | [Local live tests](evidence/phase7/live-tests-20261117T082022261647Z.txt), [human runbook](LIVE-SESSION-RUNBOOK.md) | Uses both local WebSocket integration and direct deterministic handler tests. Actual Alpaca session remains unverified. |
| 8 — logging and monitoring | DONE | [Decision/CLI/reference tests](evidence/phase8/), [monitoring guide](MONITORING.md) | Hash chain is tamper-evidence, not external authentication; halt signals a supervisor. |
| 9 — artifact signing | DONE | [Signature tests and full regression](evidence/phase9/), [trust guide](MODEL-SIGNING.md) | Optional and default-off. Production key provisioning/rotation and filesystem controls are operator responsibilities. |
| 10 — interview pack | PARTIAL | [Pitch, walkthrough, answers and resume bullets](INTERVIEW.md) | Engineering claims are supported; empirical numbers await the study and benchmark. |

DONE refers only to the stated local acceptance scope. Evidence files retain command, date, revision, working-diff identity, output and exit code. An expected guard failure is labeled as such and is not a successful study or benchmark.

## Test changes and reasons

No tests were deleted, skipped, ignored or weakened to obtain a pass.

| Test source | Reason |
|---|---|
| `python/tests/test_validity_canaries.py` (new) | Exercise shuffle/null control, pre-fit fold rejection and train-only scaling; mutation harness proves the assertions detect removed protections. |
| `rust/data/tests/pit_property_tests.rs` | Add deliberately future-available input rejection. |
| `rust/api/src/forecast.rs` test module | Check provenance classification and live-input rejection, including delayed feeds. |
| `rust/api/tests/contracts.rs` | Require provenance and decision identity in generated response contracts. |
| `web/tests/forecast.spec.ts` | Assert the visible synthetic warning. |
| `web/scripts/container-app.test.mjs` | Require synthetic provenance from real packaged demo inference. |
| `rust/api/src/live.rs` test module | Add bounded buffer, duplicate/revision/late-minute and cache-reset coverage. |
| `python/tests/test_walk_forward_orchestration.py` | Assert stored monitoring references are exactly drawn from training rows. |
| `rust/api/src/decisions.rs` test module (new) | Reject chain mutation, partial tails and duplicate outcomes; detect injected drift and validate outcome statistics. |
| `rust/cli/tests/monitor_tests.rs` (new) | Exercise actual monitor CLI warn/halt behavior and exit status. |
| `rust/inference/src/integrity.rs` test module | Cover optional/required unsigned policy, valid signatures, wrong keys and tampered packages. |
| `python/tests/test_study_metrics.py` (new) | Hand-calculated simulated fills, deterministic resampling, undefined values and multiple-trial diagnostic direction. |
| `python/tests/test_study_guards.py` (new) | Approval-byte binding, one-use holdout, target boundary isolation and generated report fidelity. |

The final local Rust/Python, format/Clippy, generated contract, web unit/bridge/build, browser and packaged-app logs are linked from [CLAIMS.md](CLAIMS.md). Test fixtures establish engineering behavior, not real-market returns.

## Unverified and human-gated work

- Review the exact bytes of `configs/study_daily.yaml` before acquisition/fitting. If approved, run development only and preserve every failed attempt. Do not revise the protocol to chase a favorable result.
- Review development results, then separately authorize the final holdout invocation. Its one-use marker must not be removed to repeat evaluation.
- Benchmark the same sealed real-data model across runtimes and generate the report from retained results.
- Operate the real Alpaca paper session using the runbook: credentials and entitlement, market-hours window, first-publication journal, compatible minute model, freshness/failure behavior and redacted evidence. No orders are placed by this work.
- Complete host-specific shutdown/SSE, load, retention, restart and restore acceptance. Current provider availability, licensing, corporate-action completeness, survivorship coverage, executable fills and capacity are not established by local tests.
- Provision signing keys and protect the trust policy/artifact directories if authenticity is required. Preserve decision-log checkpoints outside the workspace if stronger audit guarantees are required.
- Match resume wording to the actual study, including negative findings. Until then, use the implementation-only bullets in the interview pack.

## Maintainer starting points

Read [CLAIMS.md](CLAIMS.md) and [BLOCKED.md](BLOCKED.md) at every work-chunk boundary. Use [STUDY-REPORT.md](STUDY-REPORT.md) for authorized experiment commands, [BENCHMARKS.md](BENCHMARKS.md) for runtime measurement, [MONITORING.md](MONITORING.md) for decision outcomes and drift, and [MODEL-SIGNING.md](MODEL-SIGNING.md) for trust configuration. The original handover explains older architecture; this report controls current acceptance claims.
