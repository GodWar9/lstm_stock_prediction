# Capability claims ledger

Read alongside [BLOCKED.md](BLOCKED.md) at every chunk boundary. VERIFIED requires current retained acceptance evidence; source inspection alone is IMPLEMENTED-UNVERIFIED. Historical test reports remain historical.

| Capability | Status | Evidence or acceptance still needed |
|---|---|---|
| Rust workspace formatting | VERIFIED | [Phase 0 formatting](evidence/phase0/fmt-20261116T135218470334Z.txt) |
| Versioned ingestion and leakage validation | IMPLEMENTED-UNVERIFIED | Full baseline and leakage canaries pending |
| Rust feature store and Arrow training export | IMPLEMENTED-UNVERIFIED | Baseline tests pending |
| LSTM training and ONNX parity publication | IMPLEMENTED-UNVERIFIED | Baseline tests pending |
| Integrity hashes and immutable artifacts | IMPLEMENTED-UNVERIFIED | Baseline tests pending; hashes are not signatures |
| Walk-forward training | IMPLEMENTED-UNVERIFIED | Baseline tests pending |
| Cost-aware backtesting, risk and bootstrap simulation | IMPLEMENTED-UNVERIFIED | Baseline tests pending; no market-performance claim |
| React inspector and requested forecasts | IMPLEMENTED-UNVERIFIED | Contract, unit and browser checks pending |
| Alpaca minute bars feeding live forecast windows | IMPLEMENTED-UNVERIFIED | Source commits 372d0d0 and f538609; current acceptance pending |
| Real Alpaca account and host acceptance | IMPLEMENTED-UNVERIFIED | Human session, entitlement, soak, restart, retention and restore pending |
| Public crypto observation and global catalogues | IMPLEMENTED-UNVERIFIED | Bridge and browser checks pending; no model-input claim |
| Twelve Data fresh compatible-model forecasts | IMPLEMENTED-UNVERIFIED | Provider and compatible-model acceptance pending |
| Daily forex reference analytics | IMPLEMENTED-UNVERIFIED | Bridge and browser checks pending; not LSTM predictions |
| CSV default and optional Yahoo helper | IMPLEMENTED-UNVERIFIED | Baseline checks pending; Yahoo acquisition uses Python |
| Offline synthetic demo and portable Windows app | IMPLEMENTED-UNVERIFIED | Current release acceptance pending |
| Hosted Node bridge with private Rust backend | IMPLEMENTED-UNVERIFIED | Current host acceptance pending; deployment files are not deployment evidence |
| CI validation gates | IMPLEMENTED-UNVERIFIED | Workflow exists; branch run unverified |
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

| Forecast provenance and synthetic banner | VERIFIED | [API](evidence/phase2/api-tests-20261116T140100674278Z.txt), [browser](evidence/phase2/browser-20261116T140144706729Z.txt), [packaged inference](evidence/phase2/container-provenance-20261116T140239408226Z.txt) |
