# ADR 0026: PPML comparator objective adjudication

**Status:** Accepted
**Date:** 2026-09-03
**Decision owner:** Johannes Schmieder

## Context

The public PPML solver campaign exposed a distinction between the terminal
objective reported by the exact pinned `ppmlhdfe` fit and the public mean
reconstructed later by `predict, mu`. On an ill-conditioned sparse three-FE
case, the reported coefficients and objective agree closely with fereg, while
the reconstructed public mean neither replays that objective nor passes the
registered stationarity and model-space diagnostics. A matrix-free projection
can establish a nearby feasible representation, but a small normal residual
does not prove that the original predicted state is outside the model space.
The correct classification of that public state is therefore indeterminate,
not infeasible.

Requiring every comparator diagnostic state to become an independently
certified objective witness would make a post-fit reconstruction artifact in
the comparator veto an otherwise fully certified candidate. Conversely,
accepting any replayed predicted objective without checking feasibility and
stationarity could compare fereg with a state that the declared model cannot
attain or that is not a solution.

## Decision

1. For cross-estimator PPML adjudication, the authoritative reference
   objective is the exact pinned `ppmlhdfe` fit's reported `e(ll)` and
   `e(deviance)`. The pair must be finite, have a valid deviance, and satisfy
   the registered Poisson objective identity on the common sample. Fereg's
   certified objective must be no worse than this pair within the registered
   comparison tolerance.
2. The comparator's public `predict, mu` output, its independently replayed
   likelihood and deviance, KKT scores, and control-plus-FE model-space
   projection remain mandatory diagnostics. A mismatch among these values and
   the reported fit is recorded and explained; it does not by itself replace
   or invalidate the reported objective.
3. A comparator predicted state may become an additional objective witness
   only when independent checks certify both that its log-mean lies in the
   declared control-plus-FE model space and that its full control, constant,
   and per-FE-level score satisfies the registered stationarity tolerance. A
   projected state alone is not a witness, because projection establishes
   neither that the original prediction was feasible nor that the projected
   state is stationary. Fereg must also be no worse than every eligible
   additional witness within the registered comparison tolerance.
4. A model-space result above the feasible threshold remains indeterminate,
   not proof of infeasibility. An indeterminate or nonstationary comparator
   predicted state stays diagnostic and does not fail an otherwise valid
   comparison against the authoritative reported objective.
5. Fereg receives no corresponding relaxation. Its published mean must replay
   its reported likelihood and deviance, lie in its declared model space,
   satisfy every registered score and solver certificate, and agree with its
   published sample, rank, degrees of freedom, coefficients, VCE, and saved
   results. Any fereg self-consistency or certification failure remains fatal.
6. The existing narrow coefficient-only stopping adjudication remains; a
   coefficient mismatch outside that path is fatal. Sample, status,
   separation, VCE, rank, degrees-of-freedom, route, fallback, and all other
   non-objective mismatches retain their fail-closed policies. The comparator
   remains on its pinned supported conjugate-gradient/symmetric-Kaczmarz route;
   no forced `ppmlhdfe` LSMR arm is introduced.
7. The failed 2026-09-03 development campaign remains immutable evidence under
   the policy and harness version that produced it. Before another campaign,
   the development manifest, parser, adjudicator, and focused tests must be
   versioned together to encode this decision. No historical receipt is
   reinterpreted or rewritten.

## Consequences

- Cross-estimator comparison follows the objective the pinned estimator
  actually reports while retaining enough diagnostics to expose a difference
  between its terminal fit and reconstructed public prediction.
- A genuinely feasible and stationary comparator prediction can still reveal
  a better attainable objective and veto the candidate.
- Fereg continues to meet the stronger requirement that its reported and
  public states are the same fully certified solution.
- This decision changes evidence adjudication, not estimator numerics, public
  solver syntax, automatic routing, or any previously captured result.
