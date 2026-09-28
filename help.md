I want a complete, ground-up explanation of this codebase — not a surface-level README summary. Go through it systematically:

1. FEATURE INVENTORY
   - List every major component/module (data ingestion, feature store, PIT/leakage validation, PyTorch training, ONNX export, Rust inference, signal generation, portfolio/risk/execution backtester, Monte Carlo + regime analysis, CLI).
   - For each, state what it does in one sentence, then expand.

2. HOW IT WORKS (mechanism-level)
   - For each component, walk through the actual code path: entry point → key functions/structs → data transformations → output.
   - Explain the design choices: why Rust owns data/features/inference/portfolio/risk/execution/backtest while Python is scoped to ML training only. Why Arrow IPC as the interchange format. Why ONNX as the model handoff.
   - Explain the point-in-time (PIT) correctness and leakage/purge-embargo logic specifically — what bug class it prevents and how the tests catch it.
   - Explain the walk-forward CV / IC / Sharpe metrics: what they measure and why they're the right metrics for this problem instead of plain accuracy or MSE.

3. WHY IT WORKS (the underlying theory)
   - LSTM architecture: why LSTMs (vs. plain feedforward, vs. transformers) for sequential stock return prediction, what the gates are doing conceptually.
   - Why versioned model artifacts and an instrument abstraction (for future options/futures) matter for a production quant system vs. a research notebook.
   - Why ONNX parity testing between Python and Rust inference is necessary (what could silently break without it).

4. REAL-WORLD IMPLICATIONS
   - What breaks in production if PIT/leakage checks are skipped (look-ahead bias inflating backtest performance).
   - What the Monte Carlo + regime analysis is actually protecting against (overfitting to one market regime).
   - Where this architecture would need to change to handle live trading vs. backtesting (latency, real-time data feeds, slippage modeling).
   - Honest limitations: what's still fixture-based/mocked (e.g., ModelSignalStream not yet wired into the live backtest loop) and what risk that carries if someone assumes it's production-ready.

5. PROGRESS UPDATE
   - Confirm current phase status against the Phase 1–10 roadmap (Phases 1–9 complete as of the last handover, Phase 10 — production polish, CI gates, derivatives — in progress).
   - Re-run or check: full Rust test suite (quant_features/quant_inference/quant_data/quant_portfolio/quant_signals/quant_simulation/quantctl) and Python tests (Arrow dataset contract, leakage/purge-embargo, ONNX parity) — report pass/fail counts.
   - Report git log since commit 3dcca43 — what's landed since the last handover.
   - Check status of the prioritized next-engineer roadmap items: wiring ModelSignalStream<OnnxLstmProvider> into the backtest loop, streaming feature pipeline for quantctl predict, walk-forward quarterly retraining loop, CI/CD setup, Black-Scholes Greeks in the risk engine.

Be precise and cite actual file/function names as you go — I want to understand this well enough to explain it in an interview, not just get a narrative summary.