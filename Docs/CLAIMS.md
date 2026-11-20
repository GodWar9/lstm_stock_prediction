# Capability claims ledger

Read alongside [BLOCKED.md](BLOCKED.md) at every chunk boundary. VERIFIED requires current retained acceptance evidence; source inspection alone is IMPLEMENTED-UNVERIFIED. Historical test reports remain historical.

| Capability | Status | Evidence or acceptance still needed |
|---|---|---|
| Rust workspace formatting | VERIFIED | [Phase 0 formatting](evidence/phase0/fmt-20261116T135218470334Z.txt) |
| Versioned ingestion and leakage validation in local fixtures | VERIFIED | [Rust baseline](evidence/phase0/rust-test-unrestricted-20261116T135433128565Z.txt); canaries tracked below |
| Rust feature store and Arrow training export in local fixtures | VERIFIED | [Rust baseline](evidence/phase0/rust-test-unrestricted-20261116T135433128565Z.txt) |
| LSTM training and ONNX parity publication in local fixtures | VERIFIED | [Python baseline](evidence/phase0/pytest-20261116T135417824610Z.txt) |
| Integrity hashes and immutable artifact rejection in local fixtures | VERIFIED | [Rust baseline](evidence/phase0/rust-test-unrestricted-20261116T135433128565Z.txt); hashes are not signatures |
| Walk-forward training in local fixtures | VERIFIED | [Python baseline](evidence/phase0/pytest-20261116T135417824610Z.txt) |
| Cost-aware backtesting, risk and bootstrap simulation in local fixtures | VERIFIED | [Rust baseline](evidence/phase0/rust-test-unrestricted-20261116T135433128565Z.txt); no market-performance claim |
| React inspector and requested forecasts in local fixtures | VERIFIED | [Browser acceptance](evidence/phase2/browser-20261116T140144706729Z.txt) |
| Alpaca minute window rules under local protocol tests | VERIFIED | [API tests](evidence/phase2/api-tests-20261116T140100674278Z.txt); not real-provider acceptance |
| Real Alpaca account and host acceptance | IMPLEMENTED-UNVERIFIED | Human session, entitlement, soak, restart, retention and restore pending |
| Public crypto observation and global catalogues | IMPLEMENTED-UNVERIFIED | [Bridge tests](evidence/phase3/bridge-20261116T140537097104Z.txt) pass; current actual-provider acceptance remains open |
| Twelve Data fresh compatible-model forecasts | IMPLEMENTED-UNVERIFIED | Provider and compatible-model acceptance pending |
| Daily forex reference analytics under provider fixtures | VERIFIED | [Bridge tests](evidence/phase3/bridge-20261116T140537097104Z.txt); not LSTM predictions or current provider acceptance |
| CSV default and optional Yahoo helper | VERIFIED | Local CSV tests and [reviewed real Yahoo snapshots](evidence/phase5/completed-development-audit-20261120T035137660607Z.txt); Yahoo acquisition uses Python; provider revisions and corporate actions remain limitations |
| Offline synthetic demo and portable Windows app | IMPLEMENTED-UNVERIFIED | Current release acceptance pending |
| Hosted Node bridge with private Rust backend | IMPLEMENTED-UNVERIFIED | Current host acceptance pending; deployment files are not deployment evidence |
| CI validation gates | VERIFIED | [Latest code revision green run](evidence/phase3/report-followup-ci-green-20261118T125244152326Z.txt) at `5cea24d`; evidence is revision-specific |
| Local Git history secret scan | VERIFIED | [gitleaks](evidence/phase3/secret-scan-unrestricted-20261116T140509784779Z.txt); scope is the logged revision |
| Forecast provenance and synthetic banner | VERIFIED | [API](evidence/phase2/api-tests-20261116T140100674278Z.txt), [browser](evidence/phase2/browser-20261116T140144706729Z.txt), [packaged inference](evidence/phase2/container-provenance-20261116T140239408226Z.txt) |
| Advanced research modules | IMPLEMENTED-UNVERIFIED | Library availability does not establish default CLI integration |
| Multi-asset default replay | NOT-IMPLEMENTED | Default workflow remains single-symbol |
| Live orders | NOT-IMPLEMENTED | Outside research scope |
| Complete corporate-action and survivorship coverage | NOT-IMPLEMENTED | Not established by current data pipeline |
| Real-data development baseline study | VERIFIED | [Audited development results](STUDY-REPORT.md); all scheduled development trials completed; negative after-cost results do not establish alpha |
| Final-holdout evaluation | IMPLEMENTED-UNVERIFIED | User explicitly kept final evaluation blocked on 20 November 2026; no final holdout has been evaluated |
| Signed model authenticity under configured test policy | VERIFIED | [Signature tests](evidence/phase9/signature-locked-20261117T083536947065Z.txt); [policy and deployment limits](MODEL-SIGNING.md); default remains hash-only |
| Append-only decisions and decay monitoring in local fixtures | VERIFIED | [Decision tests](evidence/phase8/decision-tests-20261117T082414771037Z.txt), [CLI drift acceptance](evidence/phase8/monitor-cli-20261117T083008060222Z.txt); [operational limits](MONITORING.md) |
| Comparative runtime performance on a real-data model | VERIFIED | [Measured comparison and parity](BENCHMARKS.md); limited to the recorded model, host and measurement session |

Numbers in historical documents are not re-certified by this ledger. Development and benchmark claims are limited to the linked retained evidence. No alpha or production-readiness claim is made.

## Current local acceptance evidence

- Core Rust workspace: [unrestricted baseline](evidence/phase0/rust-test-unrestricted-20261116T135433128565Z.txt).
- Python training tests: [baseline](evidence/phase0/pytest-20261116T135417824610Z.txt).
- Browser baseline: [desktop and mobile](evidence/phase0/e2e-unrestricted-20261116T135743458639Z.txt).

These verify local test scenarios, not real-provider access or model advantage.

Latest local regression evidence after monitoring/signing implementation: [Rust workspace](evidence/phase9/final-rust-20261117T084423717995Z.txt), [Python suite](evidence/phase5/final-python-20261117T084550720087Z.txt), [browser suite](evidence/phase8/final-web-browser-20261117T084707207661Z.txt), [packaged application](evidence/phase8/final-container-app-20261117T084757766090Z.txt), [format](evidence/phase9/final-format-20261117T084702226307Z.txt), and [Clippy](evidence/phase9/clippy-20261117T083546693536Z.txt). Evidence records the commit and working diff when checks ran. Later report rendering was checked [separately](evidence/phase5/study-report-regression-20261117T084800805751Z.txt).


## Leakage canary acceptance

VERIFIED for the synthetic test protocol: [mutation detection](evidence/phase4/mutations-20261116T140816381471Z.txt), [restored Python suite](evidence/phase4/restored-python-suite-20261116T140921089695Z.txt), and [restored Rust suite](evidence/phase4/restored-rust-suite-20261116T140938425784Z.txt). See [protocol and limits](VALIDITY-CANARIES.md). This does not establish actual-market predictive quality or survivorship coverage.
