# ADR 0230: Exact coefficient certificates for categorical PPML separation

Date: 2026-09-27. Status: accepted for local development.

## Decision

For admitted categorical multi-FE separation in the MAP-owned PPML family,
propose a recession direction from a two-intercept submodel and carry its
explicit integer FE coefficients through the generation-bound proposal. Replay
that coefficient product independently through immutable group membership.
Require exact equality with the proposed observation direction. The remaining
model coefficients are zero, so this proves full-model span membership, with
zero distance, without estimating another regression of a known FE product.
The existing independent interior, boundary, strict-margin, finite-value,
stable-row and method checks still govern every deletion.

This supersedes the rejected graph-plus-generic-projection organization in
ADR 0229. Its graph proposal was mathematically valid, but its second weighted
linear solve could fail on a response entirely in the FE span. The new private
proof is not a relaxed finite-score tolerance or an alternative rank policy.
No finite solve is needed to verify this exact product. Arbitrary numerical
directions retain their certified full-model projection check.

## Construction and acceptance

Positive-outcome rows equate the two endpoint potentials. Zero rows impose
left-potential <= right-potential. Union the positive endpoints, construct dual
CSR adjacency, and collapse directed cycles. Component order supplies integer
potentials whose left-minus-right differences are nonpositive on zero rows and
zero on positive rows. The detector retains the original-level potentials.

Acceptance reconstructs independently by traversing the original group-member
lists, rather than replaying the quotient graph. Level dimensions and integer
representability are checked; all integers and differences are exact in f64.
The proof is private, bound to the prepared generation, and cannot accompany
a no-separation report or another method. Tampered directions, coefficient
coordinates and generation metadata fail before ledger mutation.

A submodel direction proves deletions only. After the existing atomic cascade
and rebase, the requested full-model chain runs again. Remaining numerical IR
and direction checks use separately admitted retained-product CG with the same
current-weight rank, FE, finite-score and reconstruction certificates. Terminal
IR replay is mandatory. A graph without a strict direction cannot certify
absence of separation. Publication IRLS keeps its original solver policy and
all final score, objective, Hessian, inference and transaction checks.

Positive prior weights and offsets do not change the homogeneous recession
cone (ADR 0017). They retain their original sample and inference semantics.
OLS, absorbed slopes, single-effect models, fits without zero outcomes,
unrequested IR, explicit non-MAP families and refused optional memory retain
their previous routes. This does not repair the separate nonretaining-CG issues.

## Memory

Generation zero admits the complete detector bound before allocation. Later
generations only shrink. Graph traversal charges 96*L + 32*N + 4096 bytes,
where L is the sum of levels in the two selected partitions. The DSU bank becomes
the retained coefficient product; its independent replay uses an N-integer bank.
All reservations are fallible. Graph, proof and proposal storage are released
before subsequent numerical projection or rebase work.

The alternative CG phase separately charges solver storage, five coefficient
banks, two full RHS banks and proposal overlap, including in low mode. The
admitted detector amount is the maximum of these sequential phase bounds.
Detector storage does not overlap publication IRLS storage. The memory plan
records and reconstructs both authorities separately, and the graph also
requires an explicit admission flag; an unrelated solver budget is insufficient.
Admission refusal preserves the old route. No physical-memory reduction is
claimed; previously failing fits now use an additional budgeted solver.

## Evidence

The [source-bound report](../../benchmarks/reports/development/separation-coefficient-proof-local-2026-09-27/README.md)
records 40 separation tests, 54 memory tests, an explicit Poisson/Hessian/sandwich
oracle over 12 weighted/output configurations, three categorical/OLS/slope
controls, both original SCC failure inputs and four predeclared fresh cases.
The original final samples match the independent integer proof plus singleton
cascade exactly. This is a bounded local repair, not Linux qualification, a
full performance sweep, or automatic timing eligibility against references
that retain a different sample.
