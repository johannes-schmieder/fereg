# Checkpoint 04 — Certified OLS/FWL scientific core

This checkpoint adds the Stata-independent finite-dimensional OLS pipeline:

- joint certified FE residualization of the outcome and finite regressors;
- explicit FE-collinear and numerical-rank status for every canonical column;
- standardized symmetric pivoted Cholesky for the well-conditioned path;
- Householder column-pivoted QR fallback for weak or rank-deficient designs;
- identified-subspace bread without an unpivoted normal-equation inverse;
- structural residual and fitted-value reconstruction through FWL;
- centralized absorbed and finite degrees-of-freedom reporting;
- homoskedastic, HC1-style robust, and one- through four-way clustered VCE;
- tuple-coded cluster intersections and inclusion-exclusion;
- weighted fit statistics and detailed solver diagnostics.

Local validation before push:

- 42 Python contract/property tests passed;
- portable shell, Python, TOML, strict C, Clang, and GCC analyzer checks passed;
- independent Python numerical mirrors verified the QR and Cholesky coefficient and bread formulas against dense linear algebra.

The public Stata estimator remains fail-closed. This commit does not claim Stata weight-type parity, exact higher-way/slope DoF parity, FE coefficient recovery, prediction, licensed-Stata integration, or release performance. Those remain explicit later gates.
