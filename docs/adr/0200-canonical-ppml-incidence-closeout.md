# ADR 0200: close the canonical PPML incidence experiment

Status: rejected for production, 2026-09-25. Local development, not qualification.

## Decision

Retain **e3ef1757 / slopetopology1** runtime. Reject both current-weight
incidence prototypes and restore `ols/canonical_products.rs` exactly. All 558
accepted computational inputs match. The
[source-bound report](../../benchmarks/reports/development/ppml-canonical-incidence-closeout-local-2026-09-25/README.md)
preserves the implementations, independent tests, failed routing assertion,
public timing distributions and closing diagnostic. No fresh validation seed
is consumed and no candidate machinery remains in production.

## Hypothesis and implementation

The existing two-categorical-FE canonical builder first eliminates one
partition. When its direct-factor work exceeds the admitted work allowance,
it solves grounded, diagonally scaled Schur systems by CG. The accepted
lifecycle ledger assigns about 4.4 ms to geometry and 7.7 ms to RHS work across
the two canonical calls of the principal disconnected development case.

The prototypes replace dense pair-matrix construction on that iterative route
with the exact current-weight incidence operator

```
A = diag(m_b / roots^2) - C'C
C[g,h] = B[g,h] / sqrt(m_a[g]) / roots[h].
```

Here B contains group-pair weight sums. Grounded coordinates are omitted.
Positive prefix/suffix sums form the diagonal scaling without subtracting
nearly equal group masses. These quantities are proposals/preconditioners,
not acceptance certificates. Affordable Cholesky calls retain their route.

The first implementation borrows group membership, stores normalized
observation incidence and solves all disconnected free coordinates together
for each RHS. That couples component stopping and repeatedly resolves row
indices and grounded coordinates during products.

The one predeclared correction combines repeated group-pair weights, packs
non-anchor coefficients and local column indices, groups row descriptors by
component, and restores independent component/RHS recurrences and their old
iteration caps. Each component applies two sparse incidence products. No
geometry, solution or authority crosses a weight frame or solver invocation.

Both prototypes use the existing fallible phase budget. The second retains at
most N coupling coefficients, N indices, Q diagonal slots and P three-index row
descriptors, plus the existing Krylov banks. Complete coexistence is charged
before allocation. Nothing enlarges the existing allowance. The direct-product
residual certificate, finite rank/QR policy, score, reconstruction, objective,
strict comparison and exact final q*mu Hessian remain unchanged. No public
solver policy or tolerance is changed.

## Evidence and rejection

The first implementation loses all nine principal low/fast/serial categorical
pairs. Baseline/candidate medians are **111/116, 102/109 and 140/171 ms**.
Reported inner work increases **69→89, 56→76 and 63→81** while outer counts
remain four. Lower setup cost is overwhelmed by solve cost.

The corrected version restores those baseline iteration counts. Its separate
three-rotation B/C/stock block gives **107/105/98 ms** low categorical,
**100/95/101** fast categorical, **144/141/245** serial categorical,
**39/42/34** narrow, **89/89/129** wide and **72/72/82** slope controls.
There are eight wins, six losses and four ties. Low and serial each retain a
losing pair; narrow has two losses and a tie. No new family reaches 0.90 stock.
Do not pool the two blocks or interpret excluded-path clocks as mechanism gains.

The closing same-artifact trace confirms about 1.7–1.9 ms lower aggregate solver
setup and 2.2–2.4 ms lower aggregate solver execution on the large low/fast cases.
Those intervals are diagnostic solver totals, not an additive decomposition of
public time or a separate stock comparison. This improvement is too limited
relative to the main 14–25-ms gaps, the narrow loss and added representation.

Each prototype passes twelve distinct canonical tests, including independent
dense elimination across scalar extremes, nonzero weighted residual truth,
changing weights, threads, rank and budget/refusal boundaries. Each also passes
six licensed explicit Poisson/Hessian, weight/inference and separation/output
fixtures. All 116 public states, 76 baseline-anchored pairs and 36 direct stock
pairs pass their scientific audits across both pilots and the closing trace.

One existing routing test fails in both prototypes and passes in the accepted
baseline. The smaller canonical workspace admits its cold solve, so the test's
expected fallback work history is absent. A trace confirms the new cold reduced
solve. This is an unresolved integration obligation, not a test to silently
weaken or an all-green claim. Numerical truth assertions reached before that
failure pass; later assertions in that test were not reached.

Corrected-candidate tracked peaks fall about 2.12 MB on large low/fast cases,
fall about 37 KB serial, and rise about 248 KB narrow; wide and slope controls
are unchanged. Scratch is unchanged and selected telemetry deltas range from
-112 to +48 bytes across the captured comparisons. No warmed-allocation, physical-cap,
new OLS or qualification campaign is claimed.

## Consequences

Do not tune this candidate's operator crossover, packing, or component thresholds
on these cases. The corrected sparse operator is mathematically viable but has
not earned production integration. Preserve it as evidence, not another dormant
solver layer. Initial/strict construction, finite rank/verification and the
complete narrow command lifecycle remain larger unresolved boundaries.

Proceed to the narrow whole-command lifecycle milestone using the existing
nonoverlapping ledger. A new candidate must remove demonstrated repeated work
across stages and have a plausible budget for a substantial public saving.
Do not substitute another isolated canonical matvec experiment. Carry the
previous OLS slope timing concern and all scientific, memory-inventory,
constrained-fit and platform gaps forward. The broad multi-FE 0.90-stock goal
remains active and unfinished.
