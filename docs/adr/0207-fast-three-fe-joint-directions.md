# ADR 0207: reject unconditional coefficient joint directions for fast three-FE PPML

Status: rejected implementation experiment, 2026-09-25. Retain ADR 0029 routing.

## Decision

Do not replace automatic fast three-categorical-FE ordinary joint LSMR with the
existing coefficient Schur joint engine solely on the basis of FE count and
workspace admission. The source-bound [report](../../benchmarks/reports/development/ppml-fast-joint-closeout-local-2026-09-25/README.md)
preserves both implementations, diagnostics, all measured attempts and failures.
No experimental runtime changes are retained. Accepted runtime inputs match
df0472d6 / generatedscan1; two expanded Stata fixtures are retained.

Fast two-/three-FE automatic ordinary directions already use
`ReusableCategoricalJointLsmr`. Absence of calls to the coefficient joint helper
does not establish full-control projection on every ordinary step. The experiment
compared two joint formulations, not a joint solver against repeated demeaning.

## What the experiment established

On identical frozen large three-FE generated-control frames, coefficient calls
took 23–26 ms versus 80–126 ms for LSMR setup/solve/check, before additional
diagnostic cross-checks and initial coefficient-owner construction. This justified
a public implementation trial, not an unconditional routing decision.

The first timed public candidate improved the development factor median from 843
to 640 ms (stock 725 ms), but regressed fresh serial, sparse and disconnected cases
by 37%, 82% and 164%. Their outer iteration counts did not increase. A dispatch
defect sent fused strict projection through the generic coefficient projector
despite retaining a borrowed categorical canonical owner. A red/green test
identified this defect. Fixing it restored canonical costs but left development
sparse and disconnected medians 14% and 60% slower than the accepted engine.
The corrected candidate was therefore rejected without another fresh campaign.
Its timings cannot inherit the earlier candidate's factor win.

Any future coexisting direction/canonical owners must honor the canonical owner
in both fused response/control and nonfused projection. Ordinary directions have
no rank, inverse-within-design or final-Hessian authority. This is an ownership
invariant, not a reason to retain the rejected hybrid implementation.

## Scientific and memory boundaries

Neither candidate changed tolerances, samples, weights, rank, DoF, score/objective
acceptance, reconstruction or exact final q*mu Hessian requirements. Previous
solutions remained guesses; current-weight factors and independent original-input
certificates remained authoritative. No check was removed or rescheduled.

Admission conservatively charged the full original LSMR envelope plus optional
certificate geometry, coefficient workspace, prior response, canonical descriptors
and joint finite payload. Optional refusal restored LSMR. The experiment did not
clone persistent FE topology. Added modeled memory reached 77.3 MB; measured peak
changes were heterogeneous. A future replacement must justify its full lifetime
accounting rather than infer safety from the smaller ordinary owner.

The rejected native validator extension admitted MAP ordinary/LSMR checkpoint
identity only for the precisely admitted automatic fast three-intercept route.
Positive/negative native tests and licensed publication tests exercised it.
Neither that extension nor a result-wire change is retained. The existing Stata
compiled validator already supported the family identities.

## Retained tests and next design boundary

The categorical final oracle now covers explicit LSMR, explicit MAP and automatic
two-/three-FE fits, unit/FW/PW, T1/T4, robust/cluster inference and reconstruction,
with independent explicit Poisson and q*mu Hessian/sandwich checks. Automatic
three-FE publication failures must restore the prior estimate and leave requested
output names absent. The transaction fixture's stale minor-version assertion now
matches the already accepted result protocol 1.3; no production wire changes.

The next coherent hypothesis is to improve finite-control conditioning inside
existing joint LSMR while preserving its FE preconditioner and independent
certificate. The coefficient engine's full finite block and FE-equivalent control
representatives may explain its advantage on the hard generated case; this is an
inference, not a measured causal decomposition. Compare such a change on identical
frames before public admission. Do not tune routes to the rejected fresh seed,
benchmark names or thread counts, and do not revive the unchanged residual-space
extrapolation probe.
