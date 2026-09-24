# ADR 0170: componentwise canonical Schur factors

Status: accepted for local development, 2026-09-24. Not qualification.

## Decision

Use packed independent Cholesky factors for connected components of the
current-weight two-categorical-FE Schur system in the canonical PPML product
engine. This replaces the global dense factor and triangular solves of ADR
0168. It retains the same weighted pair arithmetic, normalization and original-
input residual/Gram/FE certificate. All RHSs share this call's component factors.
No weighted or sample-dependent authority survives the call.

A bounded immutable-membership pass discovers components before any weighted
numeric allocation. Its topology supplies deterministic component-contiguous
coordinates. Current weights still choose the maximum-mass retained anchor in
each component.
Original retained-level order is preserved within each block. Components with
one retained level have no free coordinate or factor; reconstruction uses their
zero retained contribution and the eliminated group means as before.

Factor storage is `sum_c (Q_c - 1)^2` doubles and Cholesky/triangular loops visit
only their corresponding component. The component descriptors start with Q
entries for indexed construction, then retain only nonempty blocks without
claiming capacity credit. The assembly bank contains `sum_c Q_c^2` doubles.
Both matrix packing and factor/solve loops visit only their corresponding component. The assembly
neighbour scratch becomes component membership after its last weighted-pair
use. There is one solver organization for connected and disconnected systems,
and no topology cache survives the call.

## Work and memory admission

Preserve the finite-Gram work allowance `N*W*(W+1)/2`, where W includes the
response. Before topology allocation, require the conservative pair assembly
bound `sum_g choose(min(members_g, Q), 2)` to fit. After discovering components,
add `sum_c floor(Q_c^3/3)` and refuse if total work exceeds the allowance.
Neither weighted numeric banks nor factors exist at that refusal point. A
connected system retains its prior factor-work bound. This changes actual
factor work, not just a routing threshold.

Let M = sum Q_c² and F = sum (Q_c-1)². Relative to ADR 0169, setup uses M+F
matrix/factor doubles instead of 2Q²; solve uses F instead of Q². Five Q-index
arrays plus Q five-index component descriptors give 10Q machine indices in
setup. Free coordinates plus component descriptors give 6Q indices in solve.
The certificate payload is unchanged. All products/sums are checked before
allocation; descriptors are charged at actual capacity even after compaction.

The initial plan sets M=F=0, retaining the topology and all mandatory nongeometry
payloads. Its admitted bank bounds the first membership-only pass. Once topology
supplies M and F, the complete maximum phase plus binding/descriptor allowance
must fit the caller's existing projection reservation. The ledger changes its
limit only at this explicit second admission and preserves its already charged
live capacity. Only then are weight vectors, matrix and factor allocated.
Every actual reservation remains fallible and charged. Budget/work refusal
still precedes retained-scratch mutation; allocation failure remains atomic.
No whole-action budget increase or uncharged retained topology is introduced.

## Scientific authority

The maximum-mass anchor and global pivot guard `128*epsilon*max(1, free_levels)`
are unchanged; smaller blocks do not weaken numerical admission. Factorization
is only a proposal. The full original-input FE moment check under current
weights, direct residual Gram, rank/QR/span/inverse checks, finite solve,
reconstruction, scores, objective, strict final Hessian, inference, DoF, sample,
separation and transactional publication retain their existing requirements.
Dropping algebraically zero cross-component arithmetic can change the numerical
recurrence; independent mathematical and public Poisson oracles supply
acceptance, not bitwise agreement with the old solve.

## Evidence and limitations

The [source-bound report](../../benchmarks/reports/development/ppml-canonical-components-local-2026-09-24/README.md)
records the accepted `canonicalcomponents2` artifact. Four fresh disconnected
cases improve 15–22% against the accepted baseline. All 20 paired comparisons
improve; candidate/stock median ratios are 0.970, 0.896, 0.632 and 1.083.
The second case's paired stock median is 0.901, so its marginal 10% advantage
is borderline. Work-refused and certificate-memory-refused losses remain.
An extra topology pass is paid even on some subsequent refusals; the large
work-refused control rises 144→148 ms. This bounded cost does not justify
retaining a second global-factor implementation.

Focused independent contrasts cover 24 unequal components,
interleaved labels, zero-free-coordinate components, both FE role orders,
changing weights and serial/parallel execution. Existing extreme-scale/rank/
rollback tests remain. One historical refusal test was actually disconnected
and became valid under the new work organization; its original failing test
receipt is preserved, the exact design is now an admission control, and a
separate connected cycle still must refuse. No tolerance or sample was changed.

Thirty-six focused core tests, a plugin budget/publication test, five licensed
fixtures, an explicit-design Poisson/Hessian/sandwich oracle, 180 selected public
PPML calls and 24 OLS controls pass. The original packed timing blocks are
unranked: the monitor missed an active Stata GUI. Corrected-monitor rechecks and
an identical-data supplement provide adoption timings; original failures,
overlaps and missing calls remain visible. Seven focused monitor tests pass.

The component matrices and full certificate reduction banks remain dense
within their dimensions; work or memory may still refuse them. This decision
does not claim the broader fixest performance goal is complete or resolve
prior reference discrepancies and constrained-fit gaps.
