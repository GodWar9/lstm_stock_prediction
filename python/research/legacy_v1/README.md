# Legacy Prototype Archive (legacy_v1)

This directory is an isolated, read-only reference archive of the historical prototype code.
It is excluded from CI and all production build paths.

All quantitative operations, feature calculations, point-in-time leakage checks, inference,
portfolio sizing, risk analysis, execution simulation, and backtesting are handled in the pure-Rust
engine (`rust/`). Python responsibilities are strictly restricted to PyTorch ML model definition,
training loops, and ONNX artifact export (`python/ml/`).
