# Project handover and hardening acceptance

Updated on 20 November 2026. Branch: `hardening/interview-ready`. Comparison baseline: `main` at `e8dc7d0`. This document supersedes the state assessment in [the earlier handover](HANDOVER-2026-11-16.md), which remains historical evidence. Nothing has been merged into main by this task.

## Current project and drift from main

The baseline already contains Rust ingestion/features/inference, Python model training, the React inspector, live Alpaca forecast inputs and newer market observation features. The hardening branch concentrates on research validity and evidence: explicit provenance, mutation-checked leakage controls, stronger CI, a frozen study protocol and runner, benchmark tooling, live-path tests, decision logging, drift monitoring, optional signatures and reconciled documentation. It adds no live order execution or new market dashboard scope.

Compared with the original Python forecasting prototype, the architectural drift is a deliberate expansion into a research platform. The approved real-data development comparison is now complete, and the preselected sealed model has been benchmarked. All active development strategies had negative after-cost Sharpe; the mean LSTM result was better than ridge but worse than GBM. These are descriptive development findings, not final-holdout superiority tests. The user explicitly kept the final holdout blocked. The packaged demonstration remains synthetic; study artifacts stay local and are not silently deployed.

## Phase acceptance

| Phase | Status | Evidence | Deviations and remaining acceptance |
|---|---|---|---|
| 0 — preflight | DONE | [Baseline evidence](evidence/phase0/), [known issues](KNOWN-ISSUES.md) | Sandbox network/loopback failures retained; unrestricted reruns passed. Remote fetch succeeded. |
| 1 — claims and documentation | DONE | [Claims ledger](CLAIMS.md), [live ingestion](19-LIVE-INGESTION.md), Git history | Historical design examples explicitly remain historical; host/provider gates remain unchecked. |
| 2 — provenance | DONE | [API, UI and packaged acceptance](evidence/phase2/) | Synthetic labels derive from model and dataset sources; this does not validate real model quality. |
| 3 — CI | DONE | [Latest code green run](evidence/phase3/report-followup-ci-green-20261118T125244152326Z.txt), [workflow](../.github/workflows/ci.yml) | Code revision `5cea24d` passed; CI evidence is revision-specific. |
| 4 — validity canaries | DONE | [Mutation and restored-suite evidence](evidence/phase4/), [limits](VALIDITY-CANARIES.md) | Controls use synthetic test data; no synthetic performance results are presented. |
| 5 — real-data study | PARTIAL | [Completed development audit](evidence/phase5/completed-development-audit-20261120T035137660607Z.txt), [results and plot](STUDY-REPORT.md) | Development completed with negative after-cost findings. Final holdout remains blocked by explicit user choice. PBO/CSCV remains unavailable. |
| 6 — inference comparison | DONE | [Audited raw samples](evidence/phase6/benchmark-results-audit-20261120T035308115714Z.txt), [measured comparison](BENCHMARKS.md) | Parity passed on the preselected real-data model. Results describe one host and measurement session, not a deployment SLA. |
| 7 — live path | DONE | [Local live tests](evidence/phase7/live-tests-20261117T082022261647Z.txt), [human runbook](LIVE-SESSION-RUNBOOK.md) | Uses both local WebSocket integration and direct deterministic handler tests. Actual Alpaca session remains unverified. |
| 8 — logging and monitoring | DONE | [Decision/CLI/reference tests](evidence/phase8/), [monitoring guide](MONITORING.md) | Hash chain is tamper-evidence, not external authentication; halt signals a supervisor. |
| 9 — artifact signing | DONE | [Signature tests and full regression](evidence/phase9/), [trust guide](MODEL-SIGNING.md) | Optional and default-off. Production key provisioning/rotation and filesystem controls are operator responsibilities. |
| 10 — interview pack | DONE | [Pitch, walkthrough, answers and resume bullets](INTERVIEW.md) | Wording reflects negative development findings and measured runtimes; no alpha or final-holdout claim. |

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
| `python/pyproject.toml` pytest configuration | Resolve repository imports from the Python working directory without depending on another test module changing the import path first. |

The final local Rust/Python, format/Clippy, generated contract, web unit/bridge/build, browser and packaged-app logs are linked from [CLAIMS.md](CLAIMS.md). Test fixtures establish engineering behavior, not real-market returns.

## Unverified and human-gated work

- Development review and execution are complete. Preserve the unchanged configuration, snapshots, trial ledger and results. Do not revise this protocol to chase a favorable result.
- The user reviewed the development outcome and explicitly kept the final holdout blocked. A later explicit authorization is required before its single evaluation. Do not delete reuse guards or treat a general request to continue as that authorization.
- Runtime comparison is complete. Preserve the sealed benchmark model and raw timing samples; any later measurement must use a new output file and identify its environment.
- Operate the real Alpaca paper session using the runbook: credentials and entitlement, market-hours window, first-publication journal, compatible minute model, freshness/failure behavior and redacted evidence. No orders are placed by this work.
- Complete host-specific shutdown/SSE, load, retention, restart and restore acceptance. Current provider availability, licensing, corporate-action completeness, survivorship coverage, executable fills and capacity are not established by local tests.
- Provision signing keys and protect the trust policy/artifact directories if authenticity is required. Preserve decision-log checkpoints outside the workspace if stronger audit guarantees are required.
- Use the updated resume bullets: a research platform and evaluated baselines, with negative development findings reported honestly. No profitable-strategy or LSTM-superiority claim is supported.

## Maintainer starting points

Read [CLAIMS.md](CLAIMS.md) and [BLOCKED.md](BLOCKED.md) at every work-chunk boundary. Use [STUDY-REPORT.md](STUDY-REPORT.md) for authorized experiment commands, [BENCHMARKS.md](BENCHMARKS.md) for runtime measurement, [MONITORING.md](MONITORING.md) for decision outcomes and drift, and [MODEL-SIGNING.md](MODEL-SIGNING.md) for trust configuration. The original handover explains older architecture; this report controls current acceptance claims.

## Retained artifacts

The study report links the published aggregate results JSON, simulated equity plot and retained evidence. The development trial ledger and source snapshot hashes are under `Docs/evidence/phase5/`. Full per-trial predictions, raw snapshots and model packages remain in the local `reports/studies/`, `datasets/` and `models/study_*/` directories; preserve them for reproduction and do not assume a Git clone contains them. The benchmark report links every raw timing sample. Metrics cover eligible evaluation dates, not a continuous calendar trading record.
