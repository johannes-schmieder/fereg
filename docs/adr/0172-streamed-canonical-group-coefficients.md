# ADR 0172: streamed group coefficients in canonical PPML products

Status: accepted for local development, 2026-09-24. Not qualification.

## Decision

Choose the coefficient lifetime from the complete current-call memory plan in
the low two-categorical-FE canonical product engine. Retain the eliminated-group
bank when all coexisting phase banks fit the existing allowance. Otherwise
compute one group's normalized centers or coefficients in worker-local scratch,
consume them for that group's observations, and reuse that scratch. In streaming
mode, construct the full bank needed by retained-row writing only after the
independent certificate releases every worker reduction.

Both storage choices share RHS assembly, factors, original-input certification
and retained-row writing. There is no duplicated old solver. Retention avoids
repeated original-column passes whenever affordable; streaming opens the route
when retention would exceed the declared memory allowance.

This is one organization of the existing grounded component Schur solver.
Current-weight matrix construction, maximum-mass grounding, global pivot guard,
factors, all-RHS triangular solves and full original-input acceptance remain.
No second numerical engine, threshold multiplier, reduced tolerance or larger
memory allowance is introduced. Fast execution, slopes, one/three-plus FE,
ordinary joint iterations and one-shot OLS retain their implementations.

## Computation and scientific authority

Original normalized column norms still use the stable scaled-square recurrence.
RHS workers own complete eliminated groups. Each group's weighted center is
computed with compensated summation, then used to accumulate its original-row
Schur RHS. Original means use compensated group sums inside each worker and a
compensated merge. This replaces row-range RHS partitioning; arithmetic order can change across
worker counts. Streaming additionally replaces the retained P-by-W center lookup.

After solving the retained FE coefficients, each certificate worker loads or
computes one eliminated group's coefficients from original columns, current
normalized weights and the solved retained coefficients. It independently reconstructs
that group's residuals and accumulates the direct residual Gram plus moments
against both original FE partitions. This retains the mathematical purpose and
exact thresholds of ADR 0168; no factor-equation residual substitutes for the
original-input check. The coefficient calculation and moment replay remain
separate observation passes.

Retained centers are updated once by subtracting the freshly solved right-effect
average, using the same averaging helper as streamed coefficient calculation.
The certificate uses those retained coefficients or group-local equivalents.
The independent nonzero-residual oracle requires identical coefficients, means,
response and Gram values between the two storage choices.

Once every certificate passes, release the reductions. A streaming call
recomputes the full eliminated coefficient bank using the same pure coefficient helper and unchanged
immutable columns, weight bank, group masses and retained coefficients. The
helper preserves the order of compensated original centering and ordinary
retained-effect subtraction. No input can be refreshed between certificate
replay and reconstruction. The retained-row writer and within-response builder
therefore use the same coefficients that were certified. A late calculation,
allocation, I/O or finite-check error still prevents publication.

Streaming costs two additional original-column/group-center passes and one
additional retained-effect averaging pass relative to retaining every center.
This trades inexpensive bounded recomputation for avoiding overlap of large
banks. No cross-call weighted authority is introduced. Original samples,
weights, separation, rank, finite solve, span/inverse, residual, score, objective,
strict final Hessian, inference, DoF and transactional publication remain.

## Enforced memory phases

Let N be observations, P eliminated levels, Q retained levels, W the full
response/control width, and T=min(requested workers,P). Let M=sum Q_c² and
F=sum (Q_c-1)² describe component matrices/factors, and J be index bytes.
Let C=PW for retention and C=0 for streaming. The conservative phase payloads are:

    setup = 8[N + P + 3Q + M + F] + J[10Q]
    solve = 8[N + P + 2Q + F + C + QW + 2W + T(QW + 3W)] + J[6Q]
    check = 8[N + P + Q + C + QW + 3W + W² + T(W² + QW + 6W)]
    reconstruct = 8[N + P + Q + PW + QW + 3W + W²]
    bytes = max(setup, solve, check, reconstruct) + 32W + 2048

The initial topology preflight uses the minimum streaming plan. After topology
fixes M and F, choose retention only when its complete phase peak fits the full
existing allowance; otherwise the streaming peak must fit. This can reclaim an
obsolete projection panel only through ADR 0171's complete admission boundary.
Reported workspace is the selected layout's bound. A budget below that bound
may select streaming; only a budget below the minimum complete plan must refuse.

The RHS bank includes a group center and two compensated-mean vectors.
Certificate workers add one coefficient vector to their existing Gram, FE-score
and diagnostic arrays. Solved coordinates occupy at most QW; the solve bound's
T(QW+3W) term covers their overlap with the cross bank after RHS reductions end.
All component descriptors are charged at actual capacity, even after compaction.

The ledger still charges fallible reservations at actual capacity and credits
only consumed, dropped owners. In streaming mode, the full PW allocation occurs
after dropping certificate reductions, with weights, masses, norms and Gram still
charged. Retention charges that same bank throughout RHS and certificate work,
with no duplicate bank. Only after reconstruction may masses end and weights
become the within-response bank. Existing scratch I/O, canonical finite products and
rollback reservations stay separately charged. Work admission and obsolete
projection-block reclamation retain ADR 0171. There is no outer budget increase.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-streamed-products-local-2026-09-24/README.md)
identifies the frozen artifact, current baseline, independent weighted and
Poisson oracles, public rotations, memory and losses. Fourteen distinct focused
core tests and five licensed fixtures pass. All 180 final fresh public states,
120 baseline pairs and 60 direct candidate/stock pairs pass; six fits also pass
the independent wide explicit-design Poisson/Hessian/sandwich oracle. The 24
OLS control applications and 12 comparisons pass.
The new 100-control nonzero-residual WLS construction has paired observations
sharing every explicit FE/control coordinate and reciprocal-weight errors.
It tests serial/eight-thread execution and two weight frames, old-budget refusal,
new admission, the one-byte boundary, correct coefficients/residuals and
reproduction of certified products by the reconstructed coefficient bank.

Fresh seeds 202609387/388 are consumed/frozen. Wide N28800/K102/T4 and
N53760/K106/T8 improve 694→557 and 1128→759 ms; comparable pinned stock
is 870 and 1016 ms. Already-admitted unbalanced performance is unchanged at
109 ms. Newly admitted whole-action peaks rise by 1.32 and 2.77 MB inside
the unchanged model; scratch sizes do not change. Narrow, fast and two
memory-refused wide losses remain.

The full PW bank is still needed for retained-row writing. Extremely wide
models or large worker-private QW reductions can still refuse admission.
The always-streaming intermediate passed all scientific checks and reduced new
wide target times substantially, but regressed already-admitted cases. That
storage policy is rejected in favor of complete-budget-based retention. Its
receipts and frozen seeds remain separate from final-candidate confirmation.
No universal speed or memory advantage follows from either representation.
