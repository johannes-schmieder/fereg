# OLS scientific fixtures

These licensed fixtures compare the installed estimator against independent
explicit-dummy regressions, pinned reference expectations, and known regression
targets. They cover sample/DoF, factors, slopes, weights, inference, outputs,
postestimation and memory modes. No legacy plugin is required.

Run the `fereg-regression` profile through `ci/run_stata_ci.sh`, or use
`scripts/run_stata_tests.py` for mapped working-tree fixtures. A successful
Stata process alone is insufficient: each fixture must emit its rc=0 sentinel.
See `docs/development_workflow.md` and `docs/SPEC.md`.
