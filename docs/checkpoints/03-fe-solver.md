# Checkpoint 03 — Certified FE solver layer

Checkpoint status: **incomplete**. The source-level solver components below are
present, but the end-to-end Phase 3 scientific, parity, and integration gates
have not been satisfied. Estimation actions remain disabled.

Commit scope:

- weighted MAP absorption with independently recomputed FE certificates;
- pinned `within` modified-LSMR and additive-Schwarz integration;
- deterministic automatic solver routing;
- certified fallback between MAP and modified LSMR;
- basis-invariant weighted FE-span projection certificates rather than
  recurrence-only or coordinate-normalized claims;
- explicit solver, convergence, fallback, timing, and warning reports;
- regression tests for categorical and absorbed-slope designs.

Local validation completed before push:

- portable repository/source checks;
- Python contract and property tests.

The first native run exposed a certificate-scaling edge case on nearly exact
fits. The certificate scales by the larger of the weighted original-RHS and
returned-residual norms, so floating-point roundoff is not promoted to an
order-one relative violation.

A later independent audit exposed a separate basis-dependence bug: normalizing
each raw coordinate of `D' W r` can falsely pass a residual that lies in the FE
span when local intercept and slope channels are nearly collinear. The repaired
gate now measures the weighted residual projection onto each effect-level
column space using rank-revealing QR. Rank selection is tied to machine
precision, not the user's convergence tolerance; raw `D' W r` is diagnostic
only and cannot fail an otherwise valid projection certificate when its
unscaled magnitude is not representable. Regression coverage includes the
concrete near-collinearity counterexample, equivalent basis transformations,
redundant channels,
exact-rank edges, multiple right-hand sides, and an independently implemented
explicit-dummy Householder-QR oracle.

This is a certificate safety repair only. It does not complete or certify Phase
3, and it does not alter projection, MAP, LSMR, singleton, OLS, PPML, or
separation algorithms.

Native Rust compilation, Clippy, unit tests, and platform plugin builds are run
through the documented local and target-native commands with retained evidence;
this project does not use GitHub Actions. Licensed-Stata parity remains a later
release gate.
