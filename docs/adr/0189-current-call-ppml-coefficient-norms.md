# ADR 0189: current-call PPML coefficient projection norms

Status: accepted for local development, 2026-09-25.

## Decision

Extend the current-call norm handoff in ADRs 0052–0053 to the admitted PPML
coefficient projector. Its input norms and mandatory independent residual
certificate already compute the quantities needed by canonical retained rank
construction. Transfer selected norms directly to their consumer instead of
scanning the same observation columns again. Initial fast construction takes
original reference and within norms; low-memory construction takes each tile's
own norms; compatible fused and scratch refreshes take current within norms.

The consumer supplies exactly the normalized weight slice used by the projector.
Selection follows the actual RHS axis, including response offsets, subsets and
duplicates. Validate output lengths, all selected positions and finite statistics
before ordinary result publication. This is a completed-call handoff, not a
last-result getter or a norm cache keyed only by sample/topology/weights.
Changed responses always produce fresh norms, even when FE geometry is reusable.

Take and clear geometry-generation authority before selection validation. Restore
it only at the existing delegated absorption entry, which immediately takes it
itself, and revoke it again during output validation/publication. An error or
unwind leaves authority absent. Successful completion restores only the generation
established by that independently certified absorption. Caller-owned norm outputs
are unchanged on returned errors. Internal selection iterators are trusted; an
unwind never authorizes reuse or public Stata publication.

## Scientific and memory boundaries

The solver, recurrence, current-weight geometry, convergence schedule and full
original-row projection certificate are unchanged. The handoff copies the same
stable scaled-sum norms previously recomputed by the consumer. Reference means,
rank policy, finite score, objective, model-space/reconstruction, exact final
Hessian, weights, separation, DoF and transactional publication retain their
existing authority and tolerances. No cancellation-prone Gram subtraction
replaces projected-column rank or Hessian construction.

Reuse the existing norm buffers and caller-owned outputs. No new bank, descriptor,
allocation allowance or automatic solver region is added. Ordinary OLS and generic
projection keep their routes. Existing alternate categorical normalization guards
remain; the fused coefficient-owner branch can also carry a categorical owner,
so categorical regression evidence remains necessary. This supersedes the earlier
multi-FE/slope exclusions only for the admitted current-call coefficient handoff.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-canonical-norm-handoff-local-2026-09-25/README.md)
records independent analytic weighted projection and Poisson/Hessian oracles,
rank deficiency, extreme scaling, changed RHS/weights, selected-column boundaries,
error/unwind revocation and recovery, fast/low modes and thread controls.
All 73 headline PPML baseline/candidate pairs have identical b/V/means; nine OLS
pairs have exact audited exports. There are 131 distinct focused Rust passes and
one previously documented baseline assertion failure, plus nine licensed fixture
executions. The allocation fixtures cover their established warmed iteration
window, not every canonical allocation or physical RSS.

Fresh slope median improvements are modest: six improve, one ties and one loses
4 ms. One predeclared same-input confirmation of that loss gains 1 ms; both blocks
remain evidence. The small, shared scan removal is retained without seed-specific
guards or additional tuning. Scratch and tracked peaks are unchanged; reported
selected bytes vary by -112 to +112 bytes and are not claimed bitwise identical.
Eight of ten fresh medians meet 0.90 times comparable stock; threaded two-slope
and narrow categorical PPML remain above stock. This is a useful bounded step,
not completion of the broader performance objective or qualification. Deferred
platforms, allocation-inventory drift and known constrained-fit failures remain.
