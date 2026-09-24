# ADR 0169: phase ownership for canonical PPML products

Status: accepted for local development, 2026-09-24. Not qualification.

## Decision

Replace ADR 0168's all-allocations-live admission with an enforced phase peak
for the same low two-categorical-FE canonical product engine. Release temporary
owners when their consumers finish, use separate RHS and certificate reduction
banks, and transfer the normalized-weight allocation to the returned response
after every weight-dependent check has passed. This admits wider response/control
systems within the existing projection reservation. No budget is increased.

The production change is confined to `ols/canonical_products.rs`. The weighted
Schur factor, grounding, scaling, column normalization, arithmetic order, direct
original-input Gram and FE certificate, scratch reconstruction and rank consumer
remain those of ADR 0168. Initial and strict changed-weight canonical boundaries
use the path when admitted. Ordinary joint directions, other FE engines, OLS,
weight/sample/separation policies and final publication retain their behavior.

## Enforced ownership and bound

A private ledger charges actual Vec capacity before initialization. Its release
operation consumes the Vec, drops it, and only then credits that capacity. Clear,
truncate and logical disuse grant no credit. Every reservation remains checked
and fallible; an allocation error remains an atomic fit error. Descriptor and
source-binding charges receive no lifetime credits.

Let N, P, Q, W and T have their ADR 0168 meanings: rows, eliminated levels,
retained levels, response-plus-control width, and min(requested workers, P).
With 8-byte doubles and J-byte machine indices, the conservative numeric/index
payload peaks are:

```
setup = 8 [N + P + 3Q + 2Q²] + J [4Q]
solve = 8 [N + P + 2Q + Q² + PW + QW + 2W + TQW] + J [Q]
check = 8 [N + P + Q + PW + QW + 3W + W²
           + T(W² + QW + 5W)]
bytes = max(setup, solve, check) + 32W + 2048.
```

All component products and sums are checked before allocation. Free-system
size is at most Q; the bound uses Q even for disconnected systems. The setup
matrix and graph scratch are released after Cholesky, before column banks.
The RHS bank contains only TQW doubles. Its merged cross bank overlaps it,
but the RHS bank ends before the solved bank starts. Since T >= 1 and free
size <= Q, the solve expression also covers cross plus solved coordinates.
The factor and cross bank end before right coefficients; solved coordinates,
scales of the factor and free indices end before the certificate bank.

The final certificate simultaneously owns original weights, both mass banks,
left/right coefficients, scales, means, norms, Gram and full private reductions.
After its unchanged tolerance/finite checks, reductions and mass banks end.
Each normalized-weight cell is then overwritten by the independently
reconstructed response. Original public weights and their owner are untouched.
Returned coefficients, response and moments occupy less than the check peak.
The ledger enforces the bound across each actual allocation, in addition to this
phase derivation; unexpected excess capacity fails rather than escaping the
reservation.

Admission still requires the unchanged structural work bound and
`bytes <= 16*N*pool - live_projection_block_capacity_bytes`. Existing retained
metadata, scratch I/O, finite workspace and strict rollback state stay separately
charged. Budget/work refusal precedes scratch mutation. There is no cross-call,
cross-weight or cross-generation cache and no new N-by-K original panel.

## Scientific authority

This changes memory ownership and route availability, not checking frequency or
tolerances. A newly admitted direct solve can differ from the previous MAP
recurrence; acceptance still requires every independent original-design FE,
finite/rank/span/inverse, residual, score, objective, reconstruction and final-
Hessian condition. Geometry is freshly constructed in each current weight frame.
On previously admitted or excluded paths the exported scientific results in the
fresh screen are byte-identical. Bitwise identity is diagnostic, not authority
for newly admitted solves.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-canonical-ownership-local-2026-09-24/README.md)
records identities, declarations, distributions, independent oracles, memory,
failures and controls. Seed 202609380 was declared before generation and is
consumed/frozen. With robust inference and fitted means, five rotations show
fresh wide parallel medians 459→306, 904→586 and 314→208 ms, versus comparable
pinned stock fixest's 449, 830 and 267 ms. Every native pair improves. A serial
wide case improves 1782→1700 ms versus stock 3968. These are development results,
not a universal 10% advantage or a qualification claim.

Private peak requirements shrink without raising the whole-action model. Newly
admitted direct solves use 0.83–3.03 MB more tracked whole-action peak than the
previous iterative fallback; they remain inside the unchanged model. Previously
admitted cases use less memory. Scratch sizes do not change. A wider K98 fresh
case still refuses memory and remains slower than stock; disconnected work
refusal and the unbalanced loss remain. Those need a different computational
organization, not a benchmark-specific admission threshold.
