# ADR 0192: reject the accurate earlier accepted-point checkpoint schedule

Status: prototype rejected, 2026-09-25. Development evidence, not qualification.

## Decision

Retain the accepted two-pass canonical finalization schedule and resgate1 runtime
from `b8f04a23`. Reject **goalacceptedwitness1**, which runs strict canonical Newton
after the first ordinary convergence pass, accepts the more accurate point and
then constructs fresh final-Hessian products under its changed weights. Preserve
the patch, independently checked accuracy and complete timings in the
[source-bound closeout](../../benchmarks/reports/development/ppml-opportunity-witness-closeout-local-2026-09-25/README.md).

This materially differs from ADR 0191: the proposed new point becomes the actual
accepted state, with normal bounded objective line search, a fresh score, a real
iteration count and a second relative-deviance/score pass. The final Hessian uses
that state's exact `q*mu`. Previous-point products cannot authorize those weights.
The earlier independent accuracy failures are resolved by this prototype.

## Evidence

Final focused Rust: 54 passes and the one known baseline strict-rejection-count
failure. Three licensed explicit-design Poisson/Hessian fixtures pass, as do all
27 pilot public states, 18 baseline comparisons and nine direct stock comparisons.
Existing same-point product-reuse tests explicitly retain their own route; a new
analytic weighted-slope test covers the new accepted-point route in both memory
modes and T1/T4, including reporting and iteration-cap behavior.

Three public rotations show two-slope PPML worsening from median 77 to 103 ms,
narrow categorical tying at 37 ms, and disconnected low categorical worsening
from 97 to 106 ms. There is one paired win and eight losses. No fresh validation
seed follows this result. Memory has no new observation bank; recorded scratch
and tracked peaks match, with one selected-model delta of +48 bytes.

A separate full-size two-slope trace locates the principal regression: the first
ordinary convergence point still moves by 1.19664993469825731e-8 in strict Newton,
failing the unchanged point-consistency check. Existing bounded recovery tightens
the inner target and resumes, adding an iteration and expensive strict work.
Other pilot cases lose the saved ordinary work to exact changed-weight finalization.

## Consequences

Do not add size/seed/thread guards or tune point-movement thresholds to rescue
this schedule. Two variants of early-checkpoint finalization have now failed
their bounded screen for different reasons: older-point accuracy and accepted-
point public cost. This particular sequence is closed.

This does not prohibit redesigning the existing canonical construction or sharing
current-frame ownership. Such a redesign should keep the accepted convergence
order and independently establish current-weight projection, rank, finite-solve,
objective, score, reconstruction and exact final-Hessian authority. Prior solutions
may remain guesses. Product or geometry validity cannot cross a changed dependency
without the required current-state work. Unchanged scientific bounds and failure
atomicity remain mandatory.

The separate opportunity refresh reproduces a dense three-FE coefficient mismatch
with stock. Keep it unranked until independently adjudicated; neither speed nor
passing objective/mean comparisons resolves coefficient accuracy. No comparator
policy or production algorithm is changed by this ADR.
