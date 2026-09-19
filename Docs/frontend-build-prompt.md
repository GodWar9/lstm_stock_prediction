# Prompt: build the frontend for the LSTM quant platform

You are a senior full-stack engineer working inside an existing Rust-first quant research repository. Your job is to build a read-only research inspector frontend that fits the existing system. Do not adapt the system to suit the frontend. Read this whole prompt before writing code.

---

## 0. Ground rules

1. **Inspect before you design.** Read `Docs/HANDOVER.md`, the workspace `Cargo.toml`, and the actual output files that `quantctl` produces for the Phase 8 (backtest) and Phase 9 (Monte Carlo and regime) runs. Where this prompt's schema sketches disagree with what the repo really emits, the repo wins. Report the difference and adapt the manifest to it.
2. **The backend is authoritative.** The frontend never recomputes returns, metrics, signals, or risk numbers. It renders what Rust produced. If a number is needed that the backend does not expose, add it to the backend contract and do not calculate it in the browser.
3. **Do not touch training or research logic.** Python stays ML-only. You may add a thin `quant_api` crate and small serialization or manifest additions to existing crates. You may not change feature, portfolio, signal, or backtest behavior.
4. **Stop at each milestone** (section 9), summarize what you built and what you verified, and wait for confirmation before continuing.
5. **No invented data.** Fixtures must come from real `quantctl` outputs. If you need synthetic data for a UI state (empty, error, loading), label it as such in code.

## 1. System facts you must respect

- The Rust workspace owns data ingestion, features, inference, signals, portfolio, risk, execution, backtest, simulation, and the CLI (`quantctl`). Crates include `quant_data`, `quant_features`, `quant_inference`, `quant_signals`, `quant_portfolio`, `quant_simulation`, and `quantctl`.
- Feature datasets use **Arrow IPC** as their contract. Model artifacts are **versioned** and exported as ONNX with a parity check.
- **Point-in-time (PIT) correctness and leakage validation are enforced** by the backend. Training uses walk-forward validation with purge and embargo.
- The backtest is **model-agnostic** through `PredictionProvider` and `SignalStream` traits. The instrument layer is abstracted so options and futures can be added later.
- The tool runs **locally on Windows**, driven by the CLI, with no auth and no cloud.
- Current status: Phases 1–9 of 10 are complete. Phase 10 (CI gates, derivatives, production polish) is in progress. Do not break its work.

## 2. Target architecture

```
quantctl  →  versioned artifacts + run manifest (on disk)
                    │
              quant_api (axum, read-only, localhost)
                    │   OpenAPI spec (utoipa)  ·  Arrow IPC  ·  JSON metadata  ·  SSE (later)
              web/ (Vite + React + TypeScript SPA)
                    │
      built assets embedded in the Rust binary (rust-embed)
      served by:  quantctl serve
```

Why this shape, so you don't drift from it:

- One process and one binary at runtime. Node is a **build-time** dependency only.
- Types flow from Rust to TypeScript, never the other way, and never by hand.
- Series data travels as Arrow, because the backend already speaks Arrow.
- **Do not use Next.js, SSR, server components, or a Node runtime server.**

## 3. The contract (build this first)

### 3.1 Run manifest

Every run directory gets a `manifest.json` written by `quantctl`. Sketch (adapt to reality):

```json
{
  "schema_version": 1,
  "run_id": "…",
  "created_at": "…",
  "kind": "backtest",
  "provenance": {
    "git_commit": "…",
    "config_hash": "…",
    "data_version": "…",
    "model_artifact_id": "…"
  },
  "model": { "kind": "onnx_lstm", "provider": "OnnxLstmProvider" },
  "instruments": [{ "id": "…", "type": "equity" }],
  "capabilities": ["equity_curve", "drawdown", "trades", "signals", "monte_carlo", "regimes", "validation"],
  "artifacts": {
    "equity": "equity.arrow",
    "trades": "trades.arrow",
    "signals": "signals.arrow",
    "simulation_bands": "simulation_bands.arrow",
    "validation": "validation.json"
  },
  "metrics": { "ic": 0.0, "sharpe": 0.0, "max_drawdown": 0.0, "hit_rate": 0.0, "turnover": 0.0 }
}
```

Requirements:

- The UI is **manifest-driven**. A page renders a panel only if the run's `capabilities` list includes it. Nothing is hardcoded to LSTM or to equities. A future non-LSTM provider or an options run must render without page rewrites.
- Include `schema_version` and have the API reject or flag versions it does not understand.

### 3.2 API

Read-only, bound to `127.0.0.1`, generated OpenAPI via `utoipa`:

| Endpoint | Returns |
|---|---|
| `GET /api/runs` | List of run summaries (JSON) |
| `GET /api/runs/{run_id}/manifest` | Manifest (JSON) |
| `GET /api/runs/{run_id}/equity.arrow` | Equity and drawdown series (Arrow) |
| `GET /api/runs/{run_id}/trades.arrow` | Trade blotter (Arrow) |
| `GET /api/runs/{run_id}/signals.arrow?asof=&instrument=` | Signals (Arrow), with an explicit as-of parameter |
| `GET /api/runs/{run_id}/simulation/bands.arrow` | Monte Carlo percentile bands (Arrow) |
| `GET /api/runs/{run_id}/validation` | Leakage and PIT report, walk-forward folds (JSON) |
| `GET /api/models` and `GET /api/models/{artifact_id}` | Versioned model artifacts, training history, ONNX parity (JSON) |
| `GET /api/events` | SSE job progress (milestone 6 only) |

Rules:

- Any series endpoint accepts `max_points` and downsamples **server-side** (LTTB or equivalent) while preserving extremes.
- Monte Carlo returns **percentile bands computed in Rust**, never raw paths.
- Run URLs are keyed by `run_id` or artifact hash and are **immutable**. Send long-lived cache headers.
- Errors return a typed JSON body with a machine-readable code and a plain-language message.

### 3.3 Types

- Generate the OpenAPI spec from the axum handlers, then generate TS types with `openapi-typescript`. Commit the generated file.
- Add a CI check (and a local script) that regenerates the types and **fails if the committed file is stale**. Wire it alongside the cargo and pytest gates already planned for Phase 10.
- Arrow column names and types are part of the contract. Document them in `Docs/` next to the manifest schema, and add a Rust test that asserts the schema of each emitted Arrow file.

## 4. Frontend stack

- Vite, React, TypeScript in strict mode, Tailwind v4.
- TanStack Query (`staleTime: Infinity` for immutable run data) and TanStack Router for typed routes.
- `apache-arrow` for decoding Arrow IPC in the browser. Decode in a Web Worker if payloads are large.
- Charts:
  - Lightweight Charts for price and equity time series.
  - uPlot for dense series such as signals, drawdown, and rolling IC.
  - Canvas (or visx) for heatmaps and the fan chart.
  - Avoid SVG-based chart libraries for long series.
- Vitest for unit tests and Playwright for a smoke test of every page.
- Suggested layout: `web/src/{api,arrow,routes,components,charts,lib}`. Match the existing workspace conventions if they differ.
- Dev mode: Vite dev server proxying `/api` to `quantctl serve`. Production mode: assets embedded via `rust-embed`, served from the same origin.

## 5. Pages

Build these in the order given in section 9. Every page shows the **provenance footer** (section 6).

1. **Overview.** Run selector, active model version, latest data date, headline metrics (IC, Sharpe, max drawdown, hit rate, turnover). Clear indication of which run is being viewed.
2. **Backtest.** Equity curve against benchmark, drawdown, cost sensitivity, turnover, positions, trade blotter (sortable, virtualized).
3. **Data and validation.** Ingestion versions, the PIT and leakage report, and the **walk-forward fold timeline** showing train, purge, embargo, and test windows for each fold. This is the most important page for learning, so make it excellent.
4. **Models.** Versioned artifacts, training and validation loss curves, per-fold IC, ONNX parity status, side-by-side comparison of two artifact versions.
5. **Signals.** Predicted vs realized returns, calibration plot, IC over time, per-instrument signal table with the as-of control.
6. **Risk and simulation.** Monte Carlo fan chart from percentile bands, regime breakdown, distribution of outcomes.
7. **Derivatives (later).** Greeks panel, only after Greeks are wired into the risk engine.

Behavior that applies everywhere:

- **Out-of-sample only by default.** In-sample results, if the backend exposes them, sit behind a clearly labelled toggle with a warning that they overstate performance.
- **Explain the concepts.** Each metric, and each element of the fold timeline, has a short plain-language explanation (what it measures, why it can mislead) available on demand. The goal is for the person using it to learn the concepts while inspecting results.
- Loading, empty, and error states are designed, not left as spinners or blank panels. Errors say what went wrong and what to do. Empty states say how to produce the missing data (for example, which `quantctl` command creates it).
- Numbers use consistent formatting (percentages, basis points, decimals) and always show their units.

## 6. Provenance footer (required)

Every page shows, in a compact and always visible footer or panel: run ID, data version, model artifact ID, git commit, and config hash, each copyable. A mismatch between the manifest and what is displayed is a bug.

## 7. Design direction

Treat this as a distinctive tool for people who inspect model results critically, not a generic dashboard. Before writing UI code:

1. **Write a short design plan** and save it as `web/DESIGN.md`:
   - **Color:** 4–6 named hex values, including how positive, negative, and out-of-sample vs in-sample are distinguished. Do not rely on color alone; add shape, pattern, or labels for accessibility.
   - **Type:** one or two typefaces and their roles, plus a type scale. Numbers must use tabular figures.
   - **Layout:** one-sentence concept and an ASCII wireframe for the Overview and the fold timeline page, with alignment guidance.
   - **Principles:** what makes this interface specific to quant research inspection.
2. **Review the plan against these common generic defaults and revise if it lands on one by habit:**
   - warm cream background with a high-contrast serif and a terracotta accent
   - near-black background with a single acid-green or vermilion accent
   - broadsheet layout with hairline rules and dense newspaper columns
   - identical rounded cards with the same soft shadow and gradient washes
   - an all-caps tracked eyebrow above every heading, middle-dot meta strings, "WORD — fragment" labels, a monospace face on every small label, arrows appended to every link
3. Note what you changed and why, then build.

Design constraints:

- Spend boldness in one place, such as the fold timeline or the equity chart treatment. Keep everything else quiet.
- Structural devices (borders, dividers, numbering, labels) must carry information. Number things only when they are a real sequence.
- Motion is minimal and answers user actions. No entrance animations on every section.
- Quality floor: responsive down to mobile, visible keyboard focus, `prefers-reduced-motion` respected, sufficient contrast, chart data available in an accessible table view.
- Copy: sentence case, plain verbs, active voice, named for what users understand (say "Drawdown" and "Signal strength", not internal type names).

## 8. Testing and quality gates

- Rust: tests for each API handler, the manifest schema, and each Arrow file's schema. `cargo clippy` and `rustfmt` clean.
- Contract: the stale-types check from 3.3 passes.
- Frontend: strict TypeScript with no `any`, Vitest for Arrow decoding and formatting utilities, Playwright smoke test that loads every page against fixture runs.
- Performance: the Backtest page must stay responsive with the largest real run in the repo. Report the payload size and time to first chart.
- Do not regress the existing 70+ Rust tests or the 11 Python tests.

## 9. Milestones

Stop after each one, summarize, and wait.

1. **Contract.** Inspect real outputs. Define the manifest schema, add manifest writing to `quantctl`, define Arrow schemas, produce the OpenAPI spec, generate TS types, add the drift check. Docs written in `Docs/`. *Accept when:* fixtures generated from real Phase 8/9 runs validate against the schema.
2. **`quant_api` skeleton.** axum server, `quantctl serve`, runs list and manifest endpoints, embedded SPA shell with routing and the provenance footer. *Accept when:* one binary serves the app and a run list from real artifacts, with no Node running.
3. **Overview and Backtest pages** with Arrow decoding, downsampling, and the blotter. *Accept when:* metrics on screen match the backend's report values exactly.
4. **Data and validation, and Models pages,** including the fold timeline and artifact comparison.
5. **Signals, then Risk and simulation pages.**
6. **SSE job progress** and a read-only jobs view. Triggering runs from the UI is out of scope unless explicitly requested.
7. **Optional:** Tauri wrap of the same SPA, and the derivatives panel once Greeks exist in the risk engine.

## 10. Non-goals

- No Next.js, SSR, or Node runtime server.
- No metric, return, or signal computation in the browser.
- No auth, user accounts, or remote deployment.
- No storing run data in `localStorage`. Use it only for UI preferences.
- No changes to model training, feature, or backtest behavior.
- No hand-written API types.

## 11. How to report

At each milestone, give:

- what changed, by file
- what you verified and how (commands and results)
- any place the repo's reality differed from this prompt, and what you did about it
- risks or decisions you need me to make before the next milestone

Begin with milestone 1, step one: inspect the repo and report the actual output formats you find before proposing any schema.
