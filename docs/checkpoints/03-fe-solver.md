# Checkpoint 03 — Certified FE solver layer

Commit scope:

- weighted MAP absorption with independently recomputed FE certificates;
- pinned `within` modified-LSMR and additive-Schwarz integration;
- deterministic automatic solver routing;
- certified fallback between MAP and modified LSMR;
- true weighted orthogonality diagnostics rather than recurrence-only claims;
- explicit solver, convergence, fallback, timing, and warning reports;
- regression tests for categorical and absorbed-slope designs.

Local validation completed before push:

- portable repository/source checks;
- Python contract and property tests.

Native Rust compilation, Clippy, unit tests, and all platform plugin builds are delegated to the GitHub Actions matrix attached to this checkpoint commit. Licensed-Stata parity remains a later release gate.
