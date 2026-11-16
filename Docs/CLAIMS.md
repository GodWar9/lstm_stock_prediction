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
| CSV default and optional Yahoo helper | IMPLEMENTED-UNVERIFIED | Local CSV tests pass; actual Yahoo acquisition unverified and uses Python |
| Offline synthetic demo and portable Windows app | IMPLEMENTED-UNVERIFIED | Current release acceptance pending |
| Hosted Node bridge with private Rust backend | IMPLEMENTED-UNVERIFIED | Current host acceptance pending; deployment files are not deployment evidence |
| CI validation gates | IMPLEMENTED-UNVERIFIED | [actionlint](evidence/phase3/workflow-lint-unrestricted-20261116T140509224498Z.txt) passed; GitHub branch run pending |
| Local Git history secret scan | VERIFIED | [gitleaks](evidence/phase3/secret-scan-unrestricted-20261116T140509784779Z.txt); scope is the logged revision |
| Forecast provenance and synthetic banner | VERIFIED | [API](evidence/phase2/api-tests-20261116T140100674278Z.txt), [browser](evidence/phase2/browser-20261116T140144706729Z.txt), [packaged inference](evidence/phase2/container-provenance-20261116T140239408226Z.txt) |
| Advanced research modules | IMPLEMENTED-UNVERIFIED | Library availability does not establish default CLI integration |
| Multi-asset default replay | NOT-IMPLEMENTED | Default workflow remains single-symbol |
| Live orders | NOT-IMPLEMENTED | Outside research scope |
| Complete corporate-action and survivorship coverage | NOT-IMPLEMENTED | Not established by current data pipeline |
| LSTM advantage over real-data baselines | NOT-IMPLEMENTED | Frozen study and final holdout not executed |
| Signed model authenticity | NOT-IMPLEMENTED | Phase 9 pending |
| Append-only decisions and decay monitoring | NOT-IMPLEMENTED | Phase 8 pending |

Numbers in historical documents are not re-certified by this ledger. No study, benchmark or alpha claim is made here.

## Current local acceptance evidence

- Core Rust workspace: [unrestricted baseline](evidence/phase0/rust-test-unrestricted-20261116T135433128565Z.txt).
- Python training tests: [baseline](evidence/phase0/pytest-20261116T135417824610Z.txt).
- Browser baseline: [desktop and mobile](evidence/phase0/e2e-unrestricted-20261116T135743458639Z.txt).

These verify local test scenarios, not real-provider access or model advantage.


## Leakage canary acceptance

VERIFIED for the synthetic test protocol: [mutation detection](evidence/phase4/mutations-20261116T140816381471Z.txt), [restored Python suite](evidence/phase4/restored-python-suite-20261116T140921089695Z.txt), and [restored Rust suite](evidence/phase4/restored-rust-suite-20261116T140938425784Z.txt). See [protocol and limits](VALIDITY-CANARIES.md). This does not establish actual-market predictive quality or survivorship coverage.
