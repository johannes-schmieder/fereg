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

The first native run exposed a certificate-scaling edge case on nearly exact fits. The corrected certificate still recomputes every true weighted FE moment, but scales it by the weighted norm of the original right-hand side rather than the nearly zero returned residual. This prevents floating-point roundoff from being promoted to an order-one relative violation.

Native Rust compilation, Clippy, unit tests, and platform plugin builds are run
through the documented local and target-native commands with retained evidence;
this project does not use GitHub Actions. Licensed-Stata parity remains a later
release gate.
