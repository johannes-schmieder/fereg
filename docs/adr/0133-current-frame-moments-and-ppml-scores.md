# ADR 0133: current-frame means and independent PPML score columns

Status: accepted for local development, 2026-09-23.

## Decision

Canonical retained weighted-linear assembly borrows raw finite-column means
already computed under the current sample, topology and weight generation.
The session checks those generations before lending the short view. The view
checks unique ordered source coordinates, dimensions, selected-coordinate
presence and finite values. Changed weights still require the existing fresh
raw norm/mean calculation; response changes alone do not change mean(X).
No cross-call cache or observation-sized bank is introduced.

Reported-intercept assembly uses this view instead of rescanning every original
control for the same means. Raw residual and outcome means, reported-intercept
normalization, rank, finite solve, bread and inverse checks remain. Ordinary
one-shot OLS keeps its existing independently computed means. This changes
retained canonical PPML assembly in fast and low modes; it does not alter
canonical basis selection, weight semantics or publication authority.

PPML outer scores still read original controls, outcomes, prior weights and
current means on every required check. Reuse the existing numeric-column
binding for eligible numeric models, and compute independent column moments
concurrently on the explicitly admitted team. Each column retains the original
observation order, compensated accumulators and arithmetic. Merge the column
results in canonical order. FE moments and the independent score-scale scan
remain fresh and retain their existing order. No projected control, Newton
score or previous-weight statistic substitutes for this check.

The existing binding covers at least 4,096 active observations and at most 64
numeric/intercept model columns. Small, wider and generated-column models keep
the generic evaluator. These are shared representation bounds, not benchmark
identities. The fixed score-pair bank occupies 1,024 stack bytes. The workspace
holds a handle to the existing bounded thread team, not another pool or a model
cache. Workers touch only Rust-owned slices; SPI and interruption stay on the
caller thread. Current memory admission, safe-core/FFI and failure atomicity
are unchanged.

## Scientific boundary

This decision changes calculation ownership and scheduling, not the acceptance
conditions or their frequency. Both consecutive objective/score passes, current-
weight projection and model-space checks, strict canonical rank/finite solves,
accepted predictor reconstruction, final-Hessian checks and inference remain.
There is no tolerance, sample, DoF, separation or public interface change.

Tests compare the generic and numeric score routes across column-boundary
widths, T1/T4/T8 inside a foreign thread pool, common weight scales from 1e-30
to 1e30, and FE intercept/slope scores. They compare every returned certificate
field and FE moment. Retained-session tests cover changing frames, stale
authority, rank deficiency and strict commit/rollback. Licensed full-dummy
Poisson/Hessian and WLS oracles remain independent of fixest agreement.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-current-frame-moments-local-2026-09-23/README.md)
records artifact identities, diagnostics, public rotations, fresh cases,
allocation audits, OLS controls and remaining losses. Tracked/modelled peaks
are unchanged on the measured cases, and real warmed allocation audits remain
zero-event/zero-growth. This is not a physical-cap or platform qualification.

The initial investigation considered removing unused initial inverse products.
On the larger one-FE diagnostic, the whole moments/factor/inverse phase was
about 3.9 ms per canonical call; raw mean scans were about 5.8 ms each and
independent outer score checks totalled 29.1 ms. Those observations justified
this broader scan reduction, not a new rank-only initialization engine.
The whole-command 0.90-times-stock objective remains open. Historical reference
discrepancies and static allocation-inventory drift are not resolved here.
