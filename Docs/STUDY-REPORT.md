# Frozen daily study — awaiting human review

Status: **BLOCKED**. No real data has been acquired for this study, no candidate has been fitted, and no final holdout has been evaluated. There are no performance tables or plots to report. In particular, neither LSTM superiority nor inferiority has been established.

Review [the frozen configuration](../configs/study_daily.yaml) before authorizing acquisition or fitting. The retained [freeze evidence](evidence/phase5/protocol-freeze-20261117T081541420566Z.txt) records its contents and hash. Configuration changes require a new review, even if the runner accepts their new hash. Never change the protocol in response to results.

## Reproduction after review

Install the locked research dependencies with `uv sync --locked --group study` from `python/`, and build the CLI with `cargo build --locked --manifest-path rust/Cargo.toml --bin quantctl`. Run from the repository root, replacing `REVIEWED_SHA256` with the hash whose contents the human reviewed:

```powershell
python/.venv/Scripts/python.exe scripts/hardening_evidence.py --phase phase5 --name development-study --timeout 86400 -- python/.venv/Scripts/python.exe scripts/run_daily_study.py --approved-sha256 REVIEWED_SHA256 --stage development
```

The script acquires data through the existing Rust ingestion and feature export path, seals snapshot identities, fits the fixed candidates on identical folds, and retains each attempt and its outcome under `reports/studies/<configuration hash>/`. Acquisition and integrity errors stop execution. Failed model attempts consume the cap; a partial stage is not silently retried. The approval hash is a local workflow guard, not proof of who approved it. Preserve the evidence and ledgers outside the working machine.

After development results have been reviewed, a **separate human sign-off** permits the final evaluation:

```powershell
python/.venv/Scripts/python.exe scripts/hardening_evidence.py --phase phase5 --name final-holdout --timeout 86400 -- python/.venv/Scripts/python.exe scripts/run_daily_study.py --approved-sha256 REVIEWED_SHA256 --stage holdout --holdout-signoff REVIEWED_SHA256
```

A one-use marker is retained even if the holdout invocation fails. Deleting or copying a workspace defeats local reuse guards; do not do so to repeat evaluation. The initial snapshot covers the full frozen date range for integrity and feature construction; development fitting and scoring exclude final evaluation dates. Snapshot access is not a holdout performance evaluation.

The generated report reads saved JSON values directly. `--stage report` regenerates it without fitting. The equity plot and per-trial predictions remain with the JSON; keep those artifacts together when sharing. Missing statistics remain null. Probability of backtest overfitting is explicitly unavailable because CSCV is not implemented. Deflated Sharpe is an assumption-dependent diagnostic, using all logged attempts conservatively.

The universe uses present-day knowledge, historical provider revisions are not independently certified, and price returns exclude dividend accounting. Common-date aggregation reports dropped dates per symbol in JSON. Fixed simulated next-open costs do not establish executable fills or capacity. The final comparison must determine the interview and resume wording; development results alone cannot justify an alpha claim.

## Offline validation

[Python acceptance](evidence/phase5/final-python-20261117T084550720087Z.txt) includes hand-calculated execution checks, deterministic block resampling, undefined-statistic behavior, multiple-trial correction direction, review-hash rejection, one-use holdout rejection, target boundary isolation, and report-value fidelity. [An unapproved invocation](evidence/phase5/approval-guard-20261117T084659818545Z.txt) failed before acquisition, as required. These tests use synthetic controls only and do not satisfy real-study acceptance.
