# ADR 0076: narrow weighted OLS Schur admission

**Status:** Accepted for local development
**Date:** 2026-09-20
**Decision owner:** Johannes Schmieder

## Context

Johannes requested measurement of weighted dense two-FE designs and smaller
regressor counts before widening ADR 0075's guards. The local baseline shows
large discontinuities at the existing admission boundaries. In separate phase
measurements, weighted K=13 spends 94 ms in solver setup/RHS construction and
441 ms in initial absorption; the admitted prototype spends 22 and 208 ms.
Its final residual projection remains small for ordinary IID requests.

## Decision

Permit the existing initial OLS Schur attempt at 3–15 actual RHS columns on
at least eight requested workers, for unit or validated supplied weights.
The 16–64-column route retains its existing thread policy. With an outcome
and explicit constant, the new lower limit admits one finite regressor;
zero-regressor fits remain excluded. The existing probe's middle-column
selection coincides with its first-regressor selection at the minimum width.
It remains a cost heuristic followed by independent full-width certificates.

All other admission conditions remain: automatic fast OLS, exactly two
intercept effects, at least 16,384 observations, certified continuation,
more than 72 allowed iterations, joint memory admission and the eight-step
actual-weight probe with contraction at most 0.01. Rejected probes retain the
legacy route and charge their work. The 64-step cap, continuation, reference
scales and direct FE/OLS/inference certificates remain unchanged.

Weight domains, sample rules, AW/PW normalization, FW multiplicity, rank, DoF,
VCE, saved-effect normalization and transaction behavior do not change.
Weighted loading, weighted final-projection admission, counted-edge caching,
outcome overlap and graph recovery keep their existing guards. No kernel or
heap owner is added. Explicit solvers, low memory, slopes and PPML keep their
existing routes.

## Evidence and tradeoffs

The [local report](../../benchmarks/reports/development/two-fe-weighted-small-local-2026-09-20/README.md)
preserves measurements made before the guard change, paired prototype timings,
profiles, source identities and failed harness/reference comparisons. At
N=983,040 and sixteen workers, all nine ordered unit/AW/PW fit and inference
cells beat default stock fixest in every measured pair. Native fitting gains
are about 25–42%; native peaks fall rather than rise in the newly admitted
successful Schur cases. Twelve focused solver checks, three Stata fixtures,
weighted dummy/expansion oracles and independent output audits pass.

This is conditional performance evidence. Three shuffled holdouts improve
over native baseline but remain slower than fixest. Weighted full outputs
remain slower than default fixest, whose output accuracy also misses the
registered tolerance; separate tight-reference accuracy passes. The rejected
dispersed-weight and sparse controls cost approximately 8 and 7 ms, respectively
(median paired penalties 3.5% and 7.1%). Their scientific bundles and allocation
peaks remain exact. The one-FE K=1 control changes from 74 to 76 ms; its results
and allocation peak are exact. These measured costs accompany the development
decision; no universal speedup or qualification claim follows.

Further work should target the remaining narrow Schur operator cost and
weighted strict final projection/output recovery. A cheap rejection filter
would need separate graph/weight evidence before replacing any probe work.
