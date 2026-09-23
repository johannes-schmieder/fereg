# ADR 0132: shared one-FE Poisson representatives and final products

Status: accepted for local development, 2026-09-23.

## Decision

The concentrated one-intercept-FE PPML owner borrows the canonical session's
retained finite columns instead of making a second selected N-by-K copy.
A short immutable view carries the active column positions, allowing omitted
and noncontiguous coordinates. The session checks sample/topology generations,
intercept-only topology, dense ownership and the frozen canonical basis before
lending those rows. No Stata interface, solver routing or tolerance changes.

If a retained column is X_j minus a group constant c_gj, then profiling the group
intercept changes alpha_g by c_g'b and leaves the predictor, objective, finite
score and centered Hessian invariant. The retained columns are used only as
representatives of this equivalence class. They need not be orthogonal under
new weights and carry no current-weight certificate into the profiled step.
Every step still reconstructs current means and centered moments. Original-input
outer scores and strict canonical WLS independently check the resulting point.

## One canonical construction at the accepted final frame

After the existing two objective/score convergence passes, strict canonical WLS
still reconstructs the current-weight within design, full canonical rank, finite
solve, inverse and model-space decomposition. It must certify and agree with
the accepted profiled coefficients, predictor and objective. As in ADR 0127,
keep that accepted profiled point unchanged when strict WLS confirms it.

The strict solve's within design, rank and bread depend on the weights and
finite/FE design, not on whether its response is the Newton working response or
eta minus offset. Use those already certified products for final inference only
when the session's response and weight generations equal the accepted frame,
every owned weight bit and bread-scale field matches, and every weight is exactly
q times the accepted mean. Recheck the accepted predictor against the canonical
finite-plus-absorbed decomposition at the existing final synchronization bound.
The current session output must retain its valid design generations and complete
certificates. A failed identity or consistency check uses the existing final
predictor-response WLS; it cannot revive stale authority.

This supersedes ADR 0127's mandatory second predictor-response solve on this
narrow route. It does not replace strict Newton verification with an old inverse,
reuse changed-weight products or certify speculative iterations. The accepted
state, its independently checked objective and outer score remain unchanged.
Final product consumption again checks the exact Hessian frame and generations;
inference and public output retain their independent inverse, score, covariance,
mean, reconstruction and saved-effect checks. The strict WLS residual certifies
its own working-response equation and is not used as a Poisson score residual.
Count the canonical checkpoint once rather than reporting a fictitious second
linear solve. No additional state bank or response-history cache is introduced.

## Ownership, memory and scope

The profiled owner no longer allocates 8*N*K bytes for selected finite rows, nor
retains the vector descriptor. Its group totals, trial predictors/means and finite
moment buffers remain admitted and capacity-checked. Short borrows end before
session refresh or mutation. An explicitly requested allocation audit separately
admits a cold raw-input matrix and cold owner outside its measured window; that
witness compares original inputs to borrowed representatives under current
weights. The ordinary engine does not allocate that matrix.

Eligibility stays actual one-intercept-FE, fast, MAP-owned publication PPML.
Multi-FE, slopes, low memory, synthetic no-FE, other engines and mu-separation
keep their routes. Insufficient optional memory and numerical refusal preserve
the canonical alternative. Safe Rust computation, caller-thread SPI/interrupts,
the guarded plugin call and transactional publication remain unchanged.

## Evidence

The [source-bound development report](../../benchmarks/reports/development/ppml-one-fe-shared-products-local-2026-09-23/README.md)
contains artifact identities, distributions, independent oracles, allocation
checks, excluded controls and remaining losses. Tests compare full-dummy WLS,
raw versus group-shifted/noncontiguous columns at changing weights and T1/T4,
canonical fallback versus reuse with weighted clustered inference and saved
effects, and deliberate stale generations, changed weight bits/scales and
corrupted predictor consistency. This milestone does not close the public
0.90-times-stock objective or existing platform/memory qualification gaps.
