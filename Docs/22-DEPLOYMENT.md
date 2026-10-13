# Deploy LSTM on free services

This release serves the React frontend on Vercel and Rust inference on a separate
Render Free web service. Visitors can inspect saved backtests, compare model
evidence and request actual ONNX forecasts from recorded datasets.

The included package is a real, small, trained LSTM using **synthetic historical
data**. It proves the software workflow, not useful market forecasts. The UI
displays its source, timestamps and warnings. No live market feed or paid service
is required. Optional fresh global forecasts now use a separately trained matching
model and a backend market-data key; see [global setup](23-GLOBAL-MARKET-FORECASTS.md).
The bundled synthetic model stays in recorded mode.

## 1. Deploy the backend

1. Sign in to Render and choose **New > Blueprint**.
2. Connect this GitHub repository and select the commit you have tested.
3. Review `render.yaml`: one Docker web service, **plan: free**, no database,
   disk, paid worker, or scheduled task. Keep it on Free.
4. Deploy. The first Rust container build can take several minutes.
5. In the service Environment settings, copy the generated
   `QUANTCTL_BACKEND_TOKEN` privately for step 2. Do not put it in Git, a
   screenshot, an issue, a chat message, or a frontend variable.
6. Record the HTTPS origin, for example `https://your-service.onrender.com`.
   Visiting `/api/health` should return `{"status":"alive"}`.
   Visiting `/api/models` directly without a bearer token must return **401**.

The token has to be 32–256 printable ASCII characters without spaces. Remote mode
is explicit: `quantctl serve --remote --workers 1 --root /app`. Local CLI serving
retains loopback binding and localhost Host checks. The token is loaded from the
environment, never supplied on the CLI. Health reports liveness only.

## 2. Deploy the frontend

1. Sign in to Vercel and import this repository on the Hobby plan.
2. Set **Root Directory to `web`**, Framework to Vite, and Node.js to 22.x.
3. Build command: `npm run build`; output directory: `dist`.
4. Add these **server-only** environment variables to Production (and Preview
   only if you intend previews to access the same public research bundle):
   - `QUANTCTL_BACKEND_URL`: the backend HTTPS origin, without a path/query.
   - `QUANTCTL_BACKEND_TOKEN`: exactly the token from Render.
5. Deploy. Do not prefix either setting with `VITE_`; that would expose it to
   frontend builds. Redeploy Vercel after changing environment variables.
6. Open the Vercel URL at `/forecast`, choose `deploy_demo_v1` and
   `AAPL · 1d · deploy_demo_v1 · synthetic`, then generate a forecast.

`web/vercel.json` forwards allowed API paths to a Node function and routes
browser deep links to the SPA. The function adds the backend token privately.
It accepts GET only, never accepts a user-supplied upstream URL, rejects
redirects, preserves Arrow bytes/pagination headers, and forwards SSE.
Requests/streams end within 110 seconds; EventSource reconnects automatically.
Ordinary responses are capped at 4 MiB; use pagination/downsampling for larger
artifacts. Backend calls and errors are not shared-cacheable.

**Public access is intentional:** the Vercel bridge exposes the selected research
bundle to visitors. The backend token protects direct backend access; it is not a
visitor login. Package only artifacts you want public. This release does not
provide private user accounts or tenant isolation.

## 3. Hosted acceptance before sharing

- Load and refresh `/`, `/forecast`, `/models`, and `/live` on desktop/mobile.
- Generate a forecast and inspect source, data time, horizon and evidence hashes.
- Open a backtest; confirm charts and paginated records load.
- The live page should honestly show disabled until a feed is configured.
- Leave the live page open over two minutes; verify stream reconnection.
- Check that direct unauthenticated backend API access returns 401.
- Restart the Render service; the same bundled forecast must still work.
- Check browser network/build assets: no backend token should be present.
- Let Render sleep, then visit again; verify loading/retry behavior after wakeup.

Local validation uses the real trained bundle, Rust server, bridge and production
browser build. It does not substitute for these checks on your actual hosted URLs.

## Storage and free-tier limits

Render Free sleeps after inactivity and has no persistent disk. Model, market and
run artifacts are embedded in the image from a checksum-verified archive. A
restart restores these immutable inputs; requests do not train or mutate them.
There are no uploads or jobs that falsely promise durable storage.

A first request after sleep can be slow. The UI explains wakeup and provides
retry controls. A single inference/artifact worker bounds concurrent CPU work;
busy requests receive 503. CI runs real inference under a 512 MiB / 0.5 CPU
container limit. This is a small research app, not an availability guarantee or
a high-traffic service.

Optional Alpaca capture writes journals to local disk. **Do not use this free
deployment as durable live capture storage:** journals disappear on redeploy or
restart. Keep capture/training on a durable machine, then publish a new sealed
bundle. Configure feed credentials only server-side and check provider
entitlements before showing live data publicly.

Provider limits checked 2026-10-10:
[Render Free](https://render.com/docs/free),
[Render Blueprint](https://render.com/docs/blueprint-spec),
[Vercel Functions](https://vercel.com/docs/functions/limitations),
[Vercel Node runtime](https://vercel.com/docs/functions/runtimes/node-js).

## Update the model/data bundle

Keep immutable source data, training outputs and their provenance in your private
research workspace. The public archive is `deploy/artifacts.tar.gz`; its adjacent
`artifacts.tar.json` records every file hash and the overall checksum.

For the included synthetic model, the recorded build recipe is:
```sh
quantctl --config deploy/demo.yaml data ingest
quantctl --config deploy/demo.yaml data validate
quantctl --config deploy/demo.yaml features build
quantctl --config deploy/demo.yaml train
quantctl --config deploy/demo.yaml backtest run --model deploy_demo_v1 --split test
quantctl simulate --report reports/runs/PRINTED_RUN_ID/report.json --paths 500
python scripts/deployment_bundle.py pack --model deploy_demo_v1 --dataset deploy_demo_v1 --symbol AAPL
```

For new work choose new model/dataset IDs; do not overwrite existing versions.
Use your licensed real CSV data and matching training config to replace the
synthetic sample. Run the same integrity and forecast checks before packaging.
The packer includes only the named model, dataset/symbol and matching runs; it
does not copy the rest of your workspace or environment.

```sh
python scripts/test_deployment_bundle.py
cd web
npm ci
npm run build
npm run test:bridge
npm run test:deployment
```

The deployment smoke expects a current `rust/target/debug/quantctl` binary
(`.exe` on Windows); build it with Cargo first. Set `QUANTCTL_BIN` to test another
binary and `PYTHON` if Python is not on PATH. Python **training dependencies are
not required** to run the archived model or deployment smoke.

Commit the archive, manifest and config together after review. CI builds the
backend image and verifies authentication, inference, Arrow downloads and
restart recovery. Manually deploy the tested commit in Render; the blueprint
disables automatic backend deployment. Deploy Vercel separately. Roll back both
services to the last known-good commit if acceptance fails.

## Interview walkthrough

1. Explain the boundary: Python trains; Rust validates, computes features and
   runs ONNX inference; React presents backend evidence.
2. Generate a forecast and show that it includes immutable data/model hashes and
   explicit historical/synthetic warnings.
3. Inspect held-out backtest equity, execution costs and the saved validation
   split. Explain why held-out synthetic results cannot prove trading returns.
4. Show the deployment bridge, API authentication tests and container restart test.
5. Show the fresh global data mode and explain its key, trained-model and exchange
   requirements. Account-specific live connectivity, durable hosted training jobs
   and private multi-user workspaces remain outside the verified deployment.
