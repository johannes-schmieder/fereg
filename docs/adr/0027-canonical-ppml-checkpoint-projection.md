# ADR 0027: canonical PPML checkpoint projection

**Status:** Accepted
**Date:** 2026-09-04
**Decision owner:** Johannes Schmieder

## Context

ADR 0025 assigned every initial, strict, and final-Hessian checkpoint to MAP.
That made solver identity a proxy for scientific authority. Local profiling on
the difficult two-FE chain and sparse high-cardinality three-FE designs showed
that this proxy was expensive: after joint LSMR removed most ordinary-frame
work, repeated MAP projection consumed the dominant share of runtime even
though the retained controls, response, weights, rank policy, and downstream
certificates were already owned by the canonical linear session.

A current-weight batch LSMR projection can compute the same FE quotient much
more quickly on those graphs. Its recurrence and preconditioner are not enough
to authorize a checkpoint, but the canonical session can independently
recompute the FE certificate and then run every existing rank, finite normal-
equation, bread, score, objective, predictor-reconstruction, and final-Hessian
check on the result.

## Decision

1. This decision supersedes decision 3 of ADR 0025. The canonical retained
   session still owns the first weighted-linear frame and active-basis freeze,
   but MAP need not compute that frame on the eligible route below. MAP still
   computes every checkpoint for MAP/auto and unsupported specialized fits,
   and remains the numerical checkpoint fallback. Separation and Mu work stay
   MAP-owned. `solver(auto)` remains MAP-only.
2. Scientific checkpoint authority belongs to the canonical linear session,
   its complete rank policy, and its independent certificates rather than to
   a particular projection recurrence. Strict convergence and final-Hessian
   synchronization remain distinct mandatory facts.
3. On explicit LSMR or CMG fits with exactly two or three categorical
   intercept effects, the initial full-width basis-freezing frame and the
   strict and final-Hessian checkpoints may project the authoritative response
   and every generated finite column with a fresh current-weight batch LSMR
   solve. The initial input is the untouched prepared weighted-linear frame;
   later checkpoints reconstruct weights, response, and residual inputs from
   the accepted PPML state. No checkpoint consumes a stale specialized-engine
   residual as evidence. Canonical all-width rank revelation, status ordering,
   active-basis freeze, and every downstream certificate remain unchanged.
4. The LSMR candidate is held outside the accepted session until a fresh
   canonical FE projection certificate passes at the exact requested
   tolerance. The recurrence residual and Schwarz preconditioner are
   diagnostic only. After admission, all existing independent finite and FE
   normal equations, rank and basis, coefficient, bread/inverse, score,
   objective, reconstruction, and final `q_i * mu_i` Hessian certificates are
   unchanged.
5. The LSMR input matrix remains untouched. If the recurrence returns a
   finite but uncertified candidate, MAP projects those original authoritative
   inputs and must pass the same canonical certificates. Allocation, invalid
   input/data, internal, and interruption failures remain fatal. Candidate
   transaction rollback and publication atomicity are unchanged.
6. The checkpoint LSMR solve builds an ephemeral current-weight preconditioner
   and batch executor. This is deliberately independent of ordinary-frame
   state and differs from the rejected retained all-pairs Schwarz policy for
   joint three-FE PPML. Memory admission charges its copied inputs, batch
   outputs, full Schwarz construction, and concurrent RHS recurrences while
   the selected ordinary engine remains live. Initial, strict, and final
   checkpoints are serial and have the same maximum response-plus-all-controls
   width, so one full-width transient peak admits all three without double
   counting mutually exclusive work.
   It is enabled at every positive retained sample size on this explicit
   route. The September 5 local expansion removes the former 16,384-row
   admission cutoff: sparse smaller graphs can have much more expensive MAP
   checkpoints than larger well-connected graphs. Memory admission uses the
   same eligibility predicate as initial, strict, and final execution.
   A failed fresh canonical certificate permits one tighter independent batch
   at `max(0.01 * requested_tolerance, 1e-14)` if this is strictly tighter.
   Both batches use the untouched authoritative inputs and execute serially;
   no rejected candidate is retained during retry construction. The canonical
   certificate tolerance is unchanged. A second numerical rejection still
   falls back to MAP. Failed work and retry counts remain observable, and
   allocation, input, internal, and interruption errors remain fatal.
7. Result wire 1.2 remains 4x22. Computed initial, strict, and final-Hessian
   checkpoint projections appear on the `lsmr_fused` row. No-work MAP
   certificates and MAP fallback work appear on the MAP row. Checkpoint
   retries are included in the existing retry column, with all attempt
   iterations and setup, solve, and certification time retained. Optional
   private tracing separates first and tighter batches and their actual
   recurrence and canonical tolerances.
   `checkpoint_family` is LSMR when the accelerator is enabled. A numerical
   LSMR-to-MAP checkpoint fallback is separately counted and named. Requested,
   selected, concrete ordinary, effective ordinary, checkpoint, retry, and
   fallback identities remain visible.
8. Saved FE recovery and every published product use the accepted canonical
   final-Hessian state. This decision does not weaken sample, separation,
   singleton, weight, offset/exposure, rank/status, VCE, DoF, prediction,
   deterministic ordering, or transaction contracts.

## Consequences

- The checkpoint algorithm may evolve independently of checkpoint authority,
  provided the complete canonical evidence is reproduced.
- MAP remains a certified numerical fallback and the sole checkpoint route for
  MAP/auto fits and unsupported specialized topologies.
- Current-weight batch construction is admitted only because its full
  transient peak is preflighted; reuse across changing-weight checkpoints
  requires a separate measured design that preserves independence.
- Auto calibration, SCC campaigns, qualification, and release policy are not
  changed by this decision.
