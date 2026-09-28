# ADR 0238: prepare a converged outer point at the strict inner target

Status: accepted for bounded development, 2026-09-28. Not qualification.

## Hypothesis

After ADR0236's necessary coefficient-accuracy repair, several ordinary PPML
directions remain accurate enough for the outer score and objective but too
loose for the independently checked finite coefficients. The existing schedule
uses one tenth of the larger outer diagnostic, subject to its staged and prior
bounds. After the first outer convergence pass that can still be around 1e-9,
where the final current-weight solve requires 1e-12. On the original zero case
this leads to a rejected strict correction at iteration four, two more ordinary
passes and a repeated canonical checkpoint. Those traces identify unnecessary
linear error as a plausible avoidable cost, not a proof of a general speed gain.

## Decision

When both finite, nonnegative accepted-state diagnostics meet their existing
respective outer bounds, set the next ordinary weighted-linear target to the
existing strict scheduling floor. A tighter previously required target remains
binding. Otherwise preserve the accepted staged/progress schedule; invalid
diagnostics retain its existing strict refusal behavior. The criterion is the
existing convergence condition, with no new numerical constant or sample,
thread, topology, memory, coefficient-value or benchmark guard.

This changes the accuracy requested of an ordinary solve. It does not invoke an
early checkpoint or publish a previous point. Two accepted ordinary objective/
score passes and the independently constructed canonical strict Newton witness
remain required. Current-weight projections, rank and finite solve, scaled
coefficient movement, model-space reconstruction, fresh score/objective and exact
final-Hessian authority remain unchanged. Rejection retains the existing bounded
refinement, fallback and transaction. The ADR0177 fallback and ADR0192 rejection
of an earlier checkpoint schedule are not superseded.

Use the shared route-independent schedule, including low memory and the test-only
dense oracle. No new buffer, allocation, public option or memory-model allowance.
OLS never calls this schedule. Tight user tolerances and both separate outer
bounds remain binding. This is stronger inner work in anticipation of publication,
not relaxed scientific acceptance or a general coefficient forward-error bound.

## Validation and limits

The [source-bound report](../../benchmarks/reports/development/ppml-publication-target-scc-2026-09-28/README.md)
records independent small Poisson/Hessian and multipartition-slope fixtures,
original positive and zero-outcome coefficient intervals, strict/rebuild/rollback
boundaries and OLS controls. Mac and Linux pass 65 focused Rust tests and four
licensed fixtures each; Linux passes PPML smoke. Original fast fits retain the
certified coefficient envelope with four rather than six outer iterations.

Uncontended initial and fresh rotations complete 153 applications, 51/51 native
science pairs and 49/51 stock pairs. Initial weak stock SE failures remain
unranked. Fresh small-fast, wide, weak and strong-d2 PPML gain 24–25%; serial small
gains 36%. Repeated strict finalization disappears. Modeled memory is unchanged.

Retain the broad improvement without fitting exceptions to unfavorable cases.
Low mode loses 2.7% initially and nearly ties fresh; fresh three-FE is 7% slower,
with unchanged reported work counts. Low-memory slopes include a 20.7% paired
loss and wide timing dispersion. These are retained limitations, not proof of
regression freedom. A new one-FE control exposes an existing 3.5-times-stock loss.
Core scaling and the global stock-relative performance targets remain open.
