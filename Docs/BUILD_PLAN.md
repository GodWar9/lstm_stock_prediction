# Deployment build plan

Updated 2026-10-10. LSTM is the active release priority. Free-tier services only.
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

Release scope: recorded-data forecasting and research inspection. The included
small LSTM uses explicitly synthetic historical data; inference is real, predictive
usefulness is not established. Replace the bundle with licensed real data and a
validated model for market research. Live prices do not feed the forecast window.
Hosted training/jobs and private user accounts are outside this deployment.

Owner steps:
- Deploy Render Free from the tested commit.
- Import web/ into Vercel Hobby; configure the server-only backend URL/token.
- Complete the hosted acceptance checklist in Docs/22-DEPLOYMENT.md before sharing.

No hosted deployment is claimed from local tests. Docker is unavailable on the
local Windows machine; Linux container validation runs in GitHub Actions.
