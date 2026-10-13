# Deployment build plan

Updated 2026-10-13. LSTM is the active release priority. Free-tier services only.
The owner will create the hosted deployments later; this delivery prepares code,
artifacts, repeatable checks and exact deployment instructions.

Implemented:
1. Opt-in remote Rust serving with a required backend bearer token, bounded
   inference concurrency, and unchanged default loopback/Host protection.
2. Vercel server-only API bridge, restricted GET routes, binary/pagination
   preservation, bounded SSE with reconnect, timeout/disconnect handling and SPA routing.
3. Multi-stage non-root backend Docker image and Render Free blueprint.
4. A sealed, reproducibly packed model/data/backtest/simulation archive that
   survives restarts by being included in the image.
5. Real desktop/mobile browser-to-bridge-to-Rust inference smoke, plus unit,
   transport, artifact-integrity and container restart gates.
6. [Deployment, updates, rollback and interview walkthrough](22-DEPLOYMENT.md).

Release scope: recorded-data research plus optional fresh global stock/ETF forecasts. The included
small LSTM uses explicitly synthetic historical data; inference is real, predictive
usefulness is not established. Replace the bundle with licensed real data and a
validated matching market model to enable the fresh forecast mode. The separate Alpaca live-price page remains independent.
Hosted training/jobs and private user accounts are outside this deployment.

Owner steps:
- Deploy Render Free from the tested commit.
- Import web/ into Vercel Hobby; configure the server-only backend URL/token.
- Complete the hosted acceptance checklist in Docs/22-DEPLOYMENT.md before sharing.

No hosted deployment is claimed from local tests. Docker is unavailable on the
local Windows machine; Linux container validation runs in GitHub Actions.


## 2026-10-13 continuation: global market inputs

Implemented Twelve Data daily/minute OHLCV ingestion, exact market identity and
annualization in Arrow/model provenance, fresh snapshot inference, a shared
provider cache/budget, setup status and frontend mode selection. Missing keys,
legacy models, stale prices and incompatible markets fail with actionable errors.
See [setup and remaining external steps](23-GLOBAL-MARKET-FORECASTS.md).

The shipped synthetic bundle remains recorded-only. User-key access to a chosen
exchange, real-market model training/validation, replacement deployment artifacts
and hosted acceptance are still owner-dependent. No hosting was provisioned.

## Validation checkpoint — 2026-10-13

Completed locally: full Rust workspace tests (including actual training/inference,
backtest and simulation), warning-free Clippy, all 50 Python tests, frontend
production build, 13 frontend unit tests, 7 bridge tests, 50 desktop/mobile
Playwright tests and 4 deployment bundle tests. The deployment smoke passed with
actual Rust inference through the HTTP bridge on two viewports and SSE reconnect.
The shipped synthetic bundle was preserved. Browser fresh-data responses are
fixture-tested; live forecasts with an account-specific provider key are unverified.
The public Twelve Data demo endpoint confirmed authentication and identity fields.

This checkpoint was verified locally. No hosted deployment or new Linux container
verification was performed in this continuation. See 23-GLOBAL-MARKET-FORECASTS.md
for the exact key, market configuration, training, packaging and hosting steps.
