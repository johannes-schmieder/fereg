# ADR 0093: evaluate direct automatic LSMR on exact cycle graphs

**Status:** Experiment; not accepted for production routing
**Date:** 2026-09-21
**Decision owner:** Johannes Schmieder

## Proposed decision

Evaluate a bounded structural guard before public fast two-intercept OLS
selects its existing solver. The guard admits only a single cycle in the
smaller-axis Schur adjacency, with three physical observations per larger-axis
group, at least 256 smaller-axis levels, a cardinality ratio of at least ten,
and at least 16,384 observations. Supplied positive finite weights must have
minimum/maximum ratio at least 1/8. The experiment covers 8–64 actual RHS
columns and either one or at least eight workers. It requires the existing
certified fallback and an iteration allowance greater than eight plus 64.

The detector records at most two distinct smaller-axis neighbors per vertex.
A larger-axis group with more than two distinct destinations, or a third
neighbor at any smaller-axis vertex, rejects the guard. Every smaller-axis
vertex must have degree two. Walking one cycle then proves that the graph is
connected. This is an exact structural predicate, invariant to observation
order, bijective group relabeling and swapping the two absorbed effects.
It does not use dataset names, sampled locality, elapsed-time probes or saved
tuning state. Higher-dimensional, slope, explicit, shared and low-memory
solver policies are outside its scope.

The two-neighbor bank requires eight bytes per smaller-axis vertex. Check the
joint solver budget before allocation, check actual capacity, release it
before constructing a solver, and include its peak and elapsed setup in the
report for both accepted and rejected probes. Rejected probes preserve the
existing route, including early successful Schur returns. A selected route
uses automatic LSMR's existing recurrence tolerance, memory admission, budgeted
continuation and fresh numerical certificates. Selecting this first route is
not reported as a solver failure or fallback.

## Motivation and evidence boundary

The [direct-LSMR diagnostic](../../benchmarks/reports/development/direct-ols-lsmr-local-2026-09-21/README.md)
finds cycle paired ratios of 0.5291–0.7673 across unit/AW, K6/K50, T1/T16 and
robust/no outputs or two crossing clusters/full outputs. Weighted threaded
rings instead regress by up to 16.7%, rejecting a blanket switch. Descriptive
locality alone also confounds these graphs with weak graphs where the old
route wins. Exact cycle recognition is a narrower, mathematically explicit
candidate for evaluation; the three-row and moderate-weight restrictions
must not be widened without additional evidence.

The implementation lives only in the captured-source experiment under
`benchmarks/one_fe_optimization/cycle_ols_lsmr.*`, enabled by the isolated
builder. Production core routing is unchanged. Before adoption, finish
independent scientific and output audits, training timings and excluded-route
controls; freeze the candidate; then run untouched routing validation seeds
and the planned Linux confirmations. Preserve the old route wherever the
required cross-platform evidence is missing. This proposal does not accept
the rule, close the broader optimization milestone, or change public syntax.
