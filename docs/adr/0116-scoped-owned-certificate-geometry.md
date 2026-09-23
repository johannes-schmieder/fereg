# ADR 0116: Scope owned-FE certificate geometry to one absorption

Status: accepted for local development, 2026-09-23.

## Decision

Generic automatic absorption with slopes may retain normalized, pivoted,
twice-reorthogonalized QR bases for its initial check, MAP pilot and CG
continuation. A private context binds an immutable design and current weights;
its bindings cannot be replaced while bases remain cached. Each new absorption
creates a new context. Residuals, reference scales, working responses, scores
and iteration state are never cached. Every scheduled check still validates
and reads the current residual, recomputes norms, span coefficients and raw
moments, and reduces them in the existing order. Rank, tolerances, stopping,
contraction, routing and publication rules are unchanged.

The existing QR implementation constructs both cached and uncached geometry.
Columns move into the retained basis without a second numeric copy. Direct
MAP/CG/IT/SQUAREM/LSMR entry points remain uncached. Categorical-only automatic
calls and invalid/empty configurations retain the original path. No seed,
benchmark identity, regressor-width or machine-specific guard is introduced.

The cache is optional under the existing joint execution/solver budget. Charge
its worst-case numeric banks and nested descriptors beside the existing
maximum-level QR scratch, covering both construction and solve. Reservations
are fallible; an allocation failure releases partial construction and continues
uncached. Cache construction time and admitted peak are retained in reports.
Release it before LSMR admission. Before CG continuation, keep it only if the
original admitted CG workspace plus retained cache fits; otherwise release it.
A failed or discarded attempt contributes its maximum peak, not the sum of
sequential attempts. An optional cache never displaces an affordable solver.

This reaches generic owned slope callers in fast and low modes. The existing
single-partition OLS cache and separate changing-weight ReusableMapAbsorber
keep their guards and implementation. Any shared generic call rebuilds bases
for that call's weights and design. There is no cross-IRLS, cross-generation or
final-Hessian reuse; PPML residual, score and Hessian certificates remain fresh.

## Evidence and tradeoff

The [local report](../../benchmarks/reports/development/certificate-geometry-local-2026-09-23/README.md)
records 4.7–21.1% paired median gains on eight automatic slope cases, with
0.67–4.54 MB additional tracked heap. All 39 final native pairs have identical
scientific exports and non-clock solver fields. Exact-budget tests exercise
cache refusal and release before CG without changing results or routing.
Independent explicit-design oracles, rank/scale tests and licensed fixtures
protect the numerical and transaction boundaries. Direct/PPML controls include
small losses. Stock still wins all five comparable timing rows. The weak stock
covariance discrepancy remains unranked with its failed oracle preserved.

Close this bounded cache experiment. Fresh certificate projection and raw
moments, and the repeated FE projection sweeps, remain substantial serial
costs. Investigate deterministic parallel group certification separately,
with explicit scratch admission and unchanged reduction order. Do not enlarge
cache scope to mutable weights or weaken checks to match comparator timing.
