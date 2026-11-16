# Known issues during hardening

Baseline validation is in progress. Record failures, their owners, and retained resolution evidence here before advancing dependent work.

| Issue | Owner | State |
|---|---|---|
| Remote fetch fails in this environment | Maintainer/environment | See BLOCKED.md and phase 0 fetch evidence |
| Historical documentation disagrees with current live inference and runtime | Hardening maintainer | Phase 1 reconciliation pending |

## Baseline outcome

The restricted Rust run failed local WebSocket timeouts; the unchanged full suite passed outside the sandbox. The restricted npm installation failed, causing missing-module frontend failures; locked installation and all frontend checks passed outside the sandbox. These are environment failures, not weakened tests. Evidence is retained under `evidence/phase0/`, including both attempts.

Python, Rust formatting, Clippy, generated-contract, web unit, bridge, build and browser checks completed successfully. Current npm audit findings remain open: owner hardening maintainer, remediation during CI hardening. See the retained `npm-audit` output; do not describe the dependency baseline as clean.

## Dependency resolution

Compatible lockfile updates resolved the baseline npm advisories. Retained before/after evidence: `evidence/phase0/npm-audit-20261116T135805641408Z.txt` and `evidence/phase3/dependency-audit-20261116T140353005992Z.txt`. This records the audit result at execution time, not a permanent security guarantee.
