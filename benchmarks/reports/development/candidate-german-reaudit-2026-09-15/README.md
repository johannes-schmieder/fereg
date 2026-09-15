# Saved German output re-audit

Independent audit source `e6ae50e` closes the native PPML model-space gap without
rerunning either estimator. These are development checks on earlier native
artifacts, not exact-candidate qualification. `validation.json` binds sources,
artifacts, saved outputs, receipts and scheduler outcomes. The calibration and
bounded singleton-heavy scaling are in the
[audit implementation report](../candidate-audit-local-2026-09-15/README.md).

The saved 5% native PPML mean passes coefficient binding, fresh all-row model
space, objective replay and scores. Its forest witness has relative residual
1.58e-16 and normal error 6.17e-18. ADR 0026's existing reported-objective
identity and native no-worse tests pass. The reference mean fails objective
replay and stationarity; grounded, degree-scaled LSMR finishes after 1,345
iterations but leaves relative residual 1.64e-5. That reference state remains
indeterminate and supplies no additional feasible objective witness. No
tolerance or scientific policy changed.

The full native PPML mean also passes, on all **88,876,264 retained rows**:
relative model-space residual 1.74e-16, normal error 9.28e-18, maximum finite
score 5.12e-14, and passing likelihood/deviance replay. The successful saved-output
audit takes 94.61 seconds and peaks at 16.32 GB RSS. The first audit's scientific
result passed, but its time collector failed on a denied system query; both its
result and failed completion are preserved. A single collector-only retest
passes. The stopped full reference remains stopped and provides no final fit.

Both saved OLS arms pass prediction feasibility, score and RSS replay checks,
but their original pointwise discrepancy remains: 699,592 rows outside the
existing envelope, maximum 6.37e-5. Of 119,607 connected components, 6,055 have
discrepancies. The largest component contains 3,281,991 rows and 680,992 of the
failures. A 100-digit explicit-dummy projection checks twelve complete small
components separately conditional on each arm's published beta: native residuals
pass all twelve (maximum error 5.11e-12); reference residuals fail all twelve.
This diagnoses those components only and does not adjudicate the large component
or the global finite optimum. The OLS comparison remains blocked pending the
separately authorized single tighter-reference check. The LSMR-specific reference
exception is not extended to CG.

The initial SCC smoke failed before audit because the frozen manifest was
omitted from deployment. Adding that exact-source test input allows all 35
Python checks and the 9,000-row smoke to pass. The empirical audit then passes
its required diagnostic checks in 245 scheduler seconds. Its `comparison_accepted`
field deliberately remains false because the OLS gate is unresolved. Every
original failed campaign receipt remains unchanged.
