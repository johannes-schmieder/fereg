# ADR 0194: independently adjudicate disconnected PPML mean accuracy

Status: accepted for local development, 2026-09-25. Not qualification.

## Decision

Extend the independent coefficient/predictor accuracy instrument to bounded
two-categorical-FE graphs with multiple components. Keep its original connected
ring entry and its domain checks. The new entry uses a grounded dense reduced
solve only to propose FE potentials; the existing outward-rounded residual,
curvature and Newton-ball calculations establish accuracy independently.

Construct a spanning tree in each component. Its FE divergence balances
independently, and its tree-flow energy bounds that component's inverse-Laplacian
energy. The full bound is the sum across components. All roots must have interval
loads enclosing zero; never transfer a component's residual load to another.
The projected-control error energies enter the same finite-curvature and full
Newton-decrement bounds as in ADR 0029. Positive certified curvature and the
existing exponential-curvature condition remain mandatory.

This is an offline diagnostic, not a production solver, new comparator exception
or publication tolerance. The graph proposal is bounded to 1,200 second-partition
levels within the existing 75,000-row/50-control domain. Its explicit workspace
estimate additionally charges the dense matrix copies. This is an allocation
model for the diagnostic, not a physical-RSS guarantee for Python/SciPy.

## Evidence and attribution

The two disconnected seed429 cases from ADR 0193 have byte-identical input data
but different fereg memory modes. A separately implemented grounded full-design
Poisson Newton solve supplies an approximate optimum. The generalized 60-digit
outward certificate bounds its log-mean error by **1.50227e-11** and its finite
coefficient radius by **3.02829e-14**.

Comparisons against that enclosure prove every baseline/candidate mean inside
the existing `rtol=1e-7, atol=1e-9` envelope in all ten repetitions/mode scopes.
They prove six stock means outside it in every scope: observations 4921–4926,
all in the same first-partition group. The maximum reference error ratio has a
lower bound **1.54294**, so rounding of the oracle cannot explain the mismatch.

Pinned unmodified fixest reproduces the exact original means at outer tolerance
1e-10 and FE tolerance 1e-11. It stops at five IRLS iterations. Outer tolerance
1e-12 takes a sixth iteration and resolves the discrepancy; tightening FE
tolerance alone to its supported 3e-12 still stops at five and does not resolve
it. `predict(type="response")` equals the fit's stored means. This identifies
outer objective-based stopping, rather than post-fit prediction reconstruction,
as the cause on these exact inputs. It is not a general claim about fixest.

## Comparable measurements

Keep the original failed pairs and their clocks unranked. Do not reinterpret
them through ADR 0029's coefficient/covariance exception, which does not grant a
mean exception. Add an explicit `fixest_glm_tol` field to the private development
surface. It is permitted only with strict comparison enabled and may only
strengthen the existing 1e-10 outer tolerance. Omission preserves the old worker
behavior, and FE tolerance stays 1e-11. The exact field is captured with the
controller and cell declarations; no frozen definition changes.

Five prospective rotations on each of the exact frozen inputs compare accepted
baseline, accepted canonicalcg3 and the same stock library at outer tolerance
1e-12. All 30 public states, 20 baseline-anchored comparisons and ten direct
candidate/stock comparisons pass. Low-mode medians are **136/107/102 ms**;
fast-mode medians **111/90/103 ms**. Fast meets the 0.90 target in this new block;
low remains a loss. These reruns are corrected-accuracy evidence on existing
validation data, not new fresh validation or performance tuning.

The [source-bound report](../../benchmarks/reports/development/ppml-mean-adjudication-local-2026-09-25/README.md)
preserves the optimum enclosure, oracle, reference probes, failed initial probe,
full timing distributions and a separate accepted-code cost ledger. It also
corrects prior report prose: the timing harness sets OpenMP to the requested
thread count, while BLAS is one. The archived controller/environment and effective
thread receipts were correct; the statement that all OpenMP work used one thread
was not. Historical containers remain unchanged.

Twenty-two focused Python tests pass, including exact-rational forest-flow
checks, disconnected nonzero-residual truth, perturbation/budget refusal,
connected ring and covariance/adjudication controls, and fail-closed comparator
settings. The isolated profile has 12 passing public states and six matching
native pairs. No Rust/ado estimator source or production algorithm changes in
this milestone. The older dense three-FE coefficient discrepancy remains open.
The broad performance objective is unfinished; current priorities belong in
STATUS.md.
