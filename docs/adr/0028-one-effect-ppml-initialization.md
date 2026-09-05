# ADR 0028: group-optimal one-effect PPML initialization

**Status:** Accepted
**Date:** 2026-09-05
**Decision owner:** Johannes Schmieder

## Context

The user-approved local optimization round includes analytic one-effect PPML
work without a second estimator or weaker publication evidence. With exactly
one categorical intercept effect, its group intercepts have a closed-form
conditional optimum. Beginning at that point can avoid ordinary IRLS work,
particularly when retained groups are small or unbalanced.

## Decision

This decision supersedes ADR 0016 decision 13 only for cold publication fits
with exactly one categorical intercept-only effect, after separation and
retained-sample rebasing. At zero finite slopes, initialize the predictor as

```text
alpha_g = log(sum_{i in g} q_i y_i)
          - log(sum_{i in g} q_i exp(offset_i))
eta_i = offset_i + alpha_g
```

Use a deterministic compensated weighted mean when the offset is constant
within a group and the raw moments and ratio are representable. The offset
then cancels analytically. Otherwise use compensated log-sum-exp evaluation;
raw overflow or underflow must not silently lose a positive term. The
structural constant retains its existing coordinate normalization; absorbed
intercepts span the group-specific adjustment. The seed uses the already
admitted initial predictor buffer and finalized memberships, not a dummy
matrix or additional observation-scale storage.

A group without a positive weighted outcome, or an unrepresentable or
out-of-bounds seed, makes the seed unusable. Overwrite the entire provisional
predictor with ADR 0016's global initialization. Do not clip values or alter
separation behavior. Explicit warm starts, Mu exploration, absorbed slopes,
no-FE fits, and multi-FE fits retain their existing initialization.

The seed is not scientific evidence. The first canonical full-width WLS solve
still freezes rank and the active basis. Ordinary IRLS and coefficient-plus-
predictor backtracking are unchanged. Two consecutive deviance-plus-independent-
score checks, strict convergence, exact final `q_i * mu_i` synchronization,
independent projection and normal equations, objective, bread, reconstruction,
VCE, prediction, and saved-effect certificates remain mandatory, including
for K=0. No accepted session or public result exists while constructing the
seed.

## Consequences

This is a starting-state optimization, not an analytic replacement for the
production estimator. It neither changes the public solver surface nor enables
an auto-routing region. Further profiled line-search changes require an
explicit decision and separate local evidence.
