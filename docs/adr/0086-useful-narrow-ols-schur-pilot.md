# ADR 0086: retain useful narrow OLS Schur pilot work

**Status:** Accepted for local development
**Date:** 2026-09-20
**Decision owner:** Johannes Schmieder

## Decision

For already-admitted two-FE OLS calls with 3–8 actual RHS columns, at least
16,384 observations and at least eight workers, advance every RHS during the
eight-step Schur pilot. The original selected columns (0, 1 and width/2),
weighted operator, arithmetic order and contraction threshold still determine
admission. The same-call owner retains the original matrix, full graph,
current recurrence state and independent original-value norms. Accepted safe
state continues with the existing capped PCG implementation and independent
observation-space certificate. Rejected probes return the original matrix
unchanged and release graph storage before the established alternative route.

Additional columns cannot change the selected-column admission decision.
Independently ready additional columns are inactive; invalid additional-column
state disables warm continuation and preserves the original accepted solve.
Selected-column or structural errors keep the former probe behavior. A lane
stopped by the probe's relative-energy cutoff can restart from its current
residual when it has not met the stricter full-solve readiness check.

Count retained pilot steps once. The existing allowance of eight pilot plus
64 solve steps remains 72 total; continuation charges all work against the
unchanged overall budget. The same preflight admits the existing full owner;
no graph or observation-sized bank is added. Shared/PPML calls, wider OLS,
explicit solver choices and final one-RHS reuse retain their prior paths.

## Evidence boundary

The [local report](../../benchmarks/reports/development/two-fe-useful-pilot-local-2026-09-20/README.md)
records 232 successful application calls, 92 native pairs without allocation
peak increases, seventeen independently audited output pairs, eighteen
independent graph/weight oracles, sixteen focused Schur tests and three
final-artifact Stata fixtures. All sixteen three-pair headline cells beat
default fixest; the worst median paired ratio is 0.948. Three initially slower
controls remain recorded and their fresh three-pair medians are below 1.05.
The broad repeated primary/holdout and native-control gates remain separate.
