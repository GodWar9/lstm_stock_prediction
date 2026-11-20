# Hardening blockers

Read this file and CLAIMS.md before each work chunk. Preserve failed attempts and append resolutions with evidence rather than erasing history.

## Phase 0 remote access

`git fetch origin` failed in the current environment. Evidence: [fetch log](evidence/phase0/fetch-20261116T135218080602Z.txt). Owner: maintainer/environment. Local work may continue; remote branch freshness and a GitHub CI run remain unverified.

## Human gates

- Phase 5: prepare the frozen study configuration for user review before running the study. Final holdout requires explicit sign-off and one evaluation only. No real-data acquisition failure may be replaced by synthetic performance results.
- Real Alpaca paper session: human-operated using the phase 7 runbook. Credentials remain in the server environment. Real-provider verification stays unverified until retained evidence is reviewed.
- Resume and interview claims must follow measured results, including negative findings.

## Remote access resolution

The unrestricted fetch succeeded; see `evidence/phase0/fetch-unrestricted-20261116T140145070547Z.txt`. Upstream remains at the original baseline. GitHub workflow execution is still a separate gate.

## Initial study and benchmark gate (subsequently resolved below)

The frozen review protocol is `configs/study_daily.yaml`; its hash and contents are retained under `evidence/phase5/`. User approval for development acquisition/fitting is pending. Final holdout sign-off remains separate. No study has run.

The three-runtime benchmark tooling requires a sealed real-data model. The synthetic demo was deliberately refused by its provenance guard; no benchmark numbers have been substituted. See `BENCHMARKS.md` and `evidence/phase6/`.

## First chunk CI resolution

After explicit user authorization, the hardening branch was pushed. GitHub run https://github.com/GodWar9/lstm_stock_prediction/actions/runs/43883425760 passed at commit `76664d3`. Subsequent code still requires its own validation; this does not certify later commits.

## Development authorization and final code CI

On 18 November 2026 the user explicitly approved **development study only** for the unchanged reviewed configuration, SHA256 `436e8abf43eb8776322c14214c3fd88eb5dd12f21d5a3ed5879aeff201e41d85`. This resolves the development approval gate above. Final holdout sign-off and the human-operated Alpaca session remain separate and unapproved. Execution evidence is retained under `evidence/phase5/`.

The complete code revision `03b411c` passed [GitHub CI](https://github.com/GodWar9/lstm_stock_prediction/actions/runs/44050702449). [Saved result](evidence/phase3/final-code-ci-green-20261118T121218505217Z.txt). This supersedes the earlier first-chunk-only CI status.

## Current state — 20 November 2026

The authorized development study completed and its trial ledger, snapshots and reported values passed [the audit](evidence/phase5/completed-development-audit-20261120T035137660607Z.txt). The configuration was not changed. [Development results](STUDY-REPORT.md) are negative after costs for all active strategies; no alpha claim is justified.

The preselected real-data model was [benchmarked across runtimes](BENCHMARKS.md), with retained raw timing samples and passing parity. The benchmark gate is resolved. The report follow-up code at `5cea24d` also passed [GitHub CI](https://github.com/GodWar9/lstm_stock_prediction/actions/runs/44306198197); [saved result](evidence/phase3/report-followup-ci-green-20261118T125244152326Z.txt).

After reviewing the development findings, the user explicitly selected **“Keep final holdout blocked; finish handover.”** Do not evaluate, tune on, or otherwise unlock that holdout without a later explicit authorization. The human-operated Alpaca session and host acceptance remain unverified. Finish and share the handover with these limits intact.
