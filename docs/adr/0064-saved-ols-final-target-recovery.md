# ADR 0064: saved OLS effects from the certified final target

**Status:** Accepted for local development
**Date:** 2026-09-13
**Decision owner:** Johannes Schmieder

## Decision

Allow the public one-shot OLS caller to recover saved categorical effects from
its certified final target when an accepted initial Schur solve omits retained
FE coefficient products. Shared weighted-linear callers default to requiring
those products. A rejected Schur probe preserves the original retention policy
and legacy route. Keep the actual weighted-operator probe, 0.01 contraction
threshold, eight probe steps, bounded 64-step attempt, shared continuation budget,
reference scales and independent certificates. Explicit solver and low-memory
eligibility remain unchanged.

Retain ADR 0063's separate final-output precision requirement, including the
1e-14 final projection. Output intent does not depend on whether an initial
coefficient product exists. Recover and normalize the requested channels through
the existing certified recovery chain and transactional publication boundary.

Before intercept-only recovery, admit the actual attempted route against the
existing joint execution/solver budget. Charge retained-product finishing,
retained CG coefficient banks, SD/LSMR overlap, normalization, certificates and
requested outputs. A route that cannot fit is skipped; the existing cheaper MAP
recovery remains available if admitted. PPML and absorbed-slope recovery keep
their existing policies. This does not close their broader accounting work.

## Evidence and tradeoffs

The [source-bound report](../../benchmarks/reports/development/saved-schur-adoption-local-2026-09-13/README.md)
binds five changed inputs against accepted `bc5a6ae`. All 23 paired commands
pass full science and requested-output comparisons. Across the three original
seeds, unit/AW/PW median paired command reductions are 73.5%/17.7%/26.8%; median
native peaks fall 54.9%/31.2%/30.9%. Full attempts and continuations are included.
The rejecting AW seed slows 3.851 seconds/4.62%; this is a real disclosed cost,
not explained away by the few microseconds of recovery planning.

Three CMG-selected controls run 30–39% faster but use 6–17 MB more native memory.
An isolated allocator diagnostic reproduces their exact candidate science and
locates each whole peak in final LSMR absorption, not saved-effect recovery.
At 35,148,872 usable bytes, four original graph controls instead use the existing
admitted final CG continuation: peaks are 15.2–17.3 MB, with independent full
b/V and saved-output checks passing. Accept the measured unrestricted tradeoff
with functioning bounded alternatives; do not impose a seed-specific guard.
Public execution-base reporting remains smaller than the whole-action bound.

Validation includes 124 focused core checks, 95 serial plugin tests, 48 independent
saved dummy fits, four omission cases, exact/one-byte-short initial/recovery/CG/
generic public boundaries, and universal/static/native arm64 ABI checks. The
independent graph oracles now include the normalized intercept and full covariance;
the harness dimension failure and completed earlier captures are preserved.

This is a local development adoption. Earlier standalone recovery-admission
cost flags remain historical evidence; the combined comparison establishes net
benefit without claiming to explain those flags. No cluster, cross-platform or
release qualification is implied.
