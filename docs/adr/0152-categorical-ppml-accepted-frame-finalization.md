# ADR 0152: categorical PPML finalization on the accepted frame

Status: accepted for local development, 2026-09-24.

## Decision

Extend ADR 0131's shared strict/final procedure to publication fits that already
own the categorical joint LSMR engine. This includes admitted auto/explicit LSMR
with two or three intercept effects, including zero active finite controls.
Pending profile rebuild or checkpoint refinement retains canonical checkpoint/
resume. Existing one-FE, low MAP, slope, CMG and OLS routing is unchanged.
This supersedes the fast categorical exclusion in ADRs 0131 and 0136; it does
not give ordinary products canonical rank or covariance authority.

After the existing two objective/score convergence passes, refresh the working
state from the accepted predictor. The existing LSMR owner solves a fresh Newton
correction at the unchanged strict checkpoint tolerance using current outcome,
means and raw Hessian weights. It recomputes its operator, target, reconstruction
and independent finite/FE normal moments. Its preconditioner and previous solution
remain computational aids, never acceptance evidence. No new solver, tolerance,
heap bank, dimension threshold or public option is introduced.

Require strict predictor, objective, active-rank and identified-coefficient
agreement with the accepted state. As on the existing categorical checkpoint path,
compare only active finite slots: the inherited absorbed intercept is a
normalization coordinate. The final canonical fit independently normalizes it.

Keep the accepted point unchanged. Prepare the final response `eta - offset`;
this operation changes no workspace weights. Refresh the canonical session once
from that response and the same weights read by the witness, then perform the
existing canonical strict transaction. It independently establishes all-width
rank/basis, projection, finite solve, inverse/bread and reconstruction. Require
unchanged basis/rank and the original strict objective/predictor/coefficient
bounds before commit. The final public boundary still checks every raw `q * mu`
weight and both response/weight generations. Inference uses canonical products.

## Response history and refusal

The external LSMR witness does not project the canonical session response. Do not
refresh that session before the witness and again before final construction:
its retained response-delta seed must stay paired with the last projected
response. The three-FE test caught this error in the first implementation; that
implementation is not adopted. The coefficient joint route continues to refresh
before its session-owned witness because that witness updates its projection state.

A numerical LSMR witness refusal resets its warm start and returns to canonical
checkpoint/resume. Predictor movement, changed basis or failed final agreement
likewise declines the shortcut; a staged canonical candidate rolls back. Fatal
allocation, domain and other errors preserve fail-closed publication. Existing
canonical-injection tests explicitly retain that path; separate shared-finalizer
injections exercise witness refusal, movement and basis rollback.

The accepted predictor, mean and coefficients never change during this shortcut,
so the independently established outer score/objective remain current. Both
speculative reconstructed predictors receive new objective evaluations. Neither
a prior ordinary certificate nor approximate equality of weight frames can
substitute for these facts. Samples, weight semantics, rank/DoF, separation,
tolerances and transactional Stata publication remain unchanged.

## Diagnostics and memory

Successful strict LSMR work appears on its existing engine row as checkpoint work,
including its iterations and setup/solve/certification times. The independent
canonical final projection is also reported. Numerical witness refusal is traced;
the enclosing strict/public command clock includes its cost. A failed solve does
not supply a successful per-engine certificate or complete per-engine iteration
count. Raw summed inner iterations mix joint recurrences and batch maxima and
must not be interpreted as equivalent computational work.

Reuse the admitted LSMR owner, workspace candidates and canonical transaction.
No new heap payload or admission formula. All 74 measured PPML pairs retain the
same modeled peak and scratch bytes. Tracked peaks match in 58; the other changes
range from -549,071 to +594,042 bytes and remain unattributed. This is not a
universal memory reduction or physical-cap qualification. Static inventory drift
and the previously open constrained-fit/CG boundaries remain unresolved.

## Evidence and next boundary

The [source-bound report](../../benchmarks/reports/development/ppml-categorical-finalization-local-2026-09-24/README.md)
contains 79 distinct Rust tests, four licensed fixtures, 234 public applications,
independent Poisson/Hessian/weighted oracles, fixed-input loss follow-ups and six
separate diagnostic fits. All final scientific checks pass. Fresh gains include
23.2% for unbalanced two FE and 8.0%/5.5% for dense two/sparse three FE; narrow
and weak cases tie at their medians. Small-fit initial losses do not reproduce;
an excluded slope follow-up remains 1 ms slower. No universal non-regression claim.

The new synthetic exact-fit three-FE unit probe also exposed an ordinary LSMR
normal-equation refusal before this shortcut, including with the shortcut disabled.
That diagnostic is preserved and remains open; this change does not repair it.

The broad <=0.90-times-stock objective is unfinished. In the wide development
case, four canonical batch calls and about 53 ms persist despite removing a rank
construction. A fresh certificate can reject one column just above tolerance and
trigger an independent full-batch restart. The next coherent target is current-
weight checkpoint batching with reusable setup and selective failed-column
refinement, preserving full fresh acceptance and explicit memory admission.
