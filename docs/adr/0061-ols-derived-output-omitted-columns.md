# ADR 0061: omitted generated columns in OLS derived outputs

**Status:** Accepted for local development
**Date:** 2026-09-13
**Decision owner:** Johannes Schmieder

## Context

OLS preserves a generated-column identity for columns omitted by FE collinearity
or numerical rank deficiency, with no fitted coefficient value. The derived-
output selector accepted only value-bearing generated columns or syntax-omitted
slots without generated identities. A valid fit could therefore return r(498)
when asked to save fixed effects or other derived predictions.

## Decision

An absent value with a generated identity contributes no finite fitted term
when its existing status is `CollinearWithFixedEffects` or
`NumericallyRankDeficient`. Keep the column's canonical slot, omission status,
zero public coefficient/covariance coordinates and all existing rank decisions.
Keep rejecting inconsistent fitted-value identities. No estimator, inference,
normalization, memory-planning or transaction policy changes.

The selector adds no allocation or observation pass. Its existing consumers
construct xb, absorbed targets, saved effects and combined predictions from
the same identified finite terms. OLS errors and partial stores retain their
existing protected publication behavior. PPML is unchanged.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ols-derived-output-omissions-local-2026-09-13/README.md)
records a closed-form failing regression and passing fast/low checks, 95 plugin
tests, four independent unit/AW/FW/PW omission/output oracles per artifact,
48 ordinary saved-output fits, and successful original slope/rank-deficient
warm/full requests. Both architecture builds and native arm64 ABI checks pass.

Eight cost pairs preserve exact science, solver decisions and native/base/scratch
peaks. Two PW increases of 819/1961 ms lie predominantly in unchanged initial CG;
their cause remains unresolved. Four predeclared follow-ups keep identical-binary
A/A separate from A/B; subsequent A/B differences are +48 ms/1.35% and +9 ms/0.25%.
The repair is adopted for correctness, with those timing limitations preserved.
No speedup, universal memory bound or qualification is claimed. The separate
saved-Schur/recovery prototype remains unadopted pending its remaining controls.
