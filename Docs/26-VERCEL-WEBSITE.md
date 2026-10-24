# Unified Vercel website

The root `vercel.json` deploys one container service, `research`, with all public
paths routed to it. `Dockerfile.vercel` builds the current production frontend,
the existing Rust CLI, and the checksum-verified public artifact bundle.

Import **GodWar9/lstm_stock_prediction** in Vercel using the **Hobby** team,
**repository root `./`**, and the root Services configuration. Set the project
environment variable **PORT=8080**. Deploy the tested commit. No provider API key,
database, paid plan or external backend account is required for the recorded app.

The website entrypoint starts Rust inference with a fresh private token generated
in process memory. The Node bridge attaches that token to internal API calls;
visitors never receive it. Only the Node public port is routed by Vercel. Direct
Rust requests still require authentication. The existing standalone and split
Render/Vercel configurations remain available independently.

This publication includes the explicitly labelled synthetic historical AAPL
dataset and sealed LSTM model. Acquisition and live feeds are disabled. Visitors
can generate real ONNX inferences and inspect recorded backtests, validation,
models, signals and bootstrap simulations. The app does not train in requests or
promise persistent private workspaces.

## Verification

`cd web; npm run test:container-app` starts the actual Rust executable and this
public server using isolated extracted artifacts. It checks unauthenticated
backend denial, real inference, Arrow records, static-path safety, read-only API
behavior, desktop/mobile forecast pages, simulation navigation and shutdown.
`PYTHON` and `QUANTCTL_BIN` may select the local test runtimes. Production uses
the paths in the Dockerfile, with no developer toolchain installed in the runtime.

Before sharing a deployment, exercise its public `/forecast`, `/risk` and `/live`
pages, real API inference, Arrow records and browser refresh. Check cold starts
and SSE reconnection on the hosted environment; local tests alone do not confirm
that a deployment succeeded.

Vercel containers and Services are currently Beta and available on Hobby. Function
quotas and cold starts apply. Keep the project on Hobby; no upgrade or paid
add-on is required by this configuration.

Primary platform references, checked 24 October 2026:

- https://vercel.com/docs/functions/runtimes/container
- https://vercel.com/docs/services
- https://vercel.com/docs/services/pricing
