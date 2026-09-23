# ADR 0139: implicit current-weight one-FE PPML products

Status: accepted for local development, 2026-09-23.

## Decision

Admitted low-memory, MAP-owned publication PPML with one actual categorical
intercept effect retains current-weight group centers instead of a projected
N-by-K scratch matrix. This includes genuine categorical interaction partitions,
numeric columns and generated terms. For group g and column j, the row view is
`X_within[i,j] = X_original[i,j] - weighted_mean[g,j]`. The response has its own
current-weight center. There is one complete candidate-column axis, including
columns whose rank status has yet to be decided.

The prepared sample, model recipes, topology and optional cluster metadata have
one immutable shared owner. Its allocation is charged at generation preparation;
its nested buffers remain charged once. A linear session checks object identity
before accepting this owner. Certified products retain it through inference,
without borrowing a generation that is returned alongside them, copying the
finite design, rebinding pointers or adding unsafe code. Ordinary one-shot OLS
keeps its existing ownership.

Canonical Gram/RHS construction, guarded Cholesky, retained-row QR, direct
dropped-span checks and inverse certificates consume the implicit rows. QR
already streams observation rows and needs no materialized N-by-K fallback.
Final robust/clustered inference consumes the same certified row view. Selected
column compaction preserves capacities and the response-center block. Ordinary
concentrated Poisson steps borrow original columns through the existing
ADR 0137 interface; they do not interpret earlier weighted centers as current.

This supersedes ADR 0020's physical representation requirements in decisions
6–11 only for this admitted low one-FE route. Canonical rank/status policy,
basis transactions, generation checks and the remaining scientific requirements
are unchanged. Fast execution retains its contiguous dense representation:
the measured dual-mode prototype reduced projection cost but increased narrow
fast finite-moment cost enough to lose overall. Other FE families, other engines,
Mu separation and ordinary OLS retain their existing numerical routes. No seed,
benchmark name, width or observation threshold selects the new representation.

## Current-frame authority and independent checks

Every refresh first revokes validity. It checks dimensions, positive finite
weights and normalization, then recomputes group weight totals, scaled
compensated group means, raw/within weighted norms and raw weighted column means.
Nothing numerical is accepted from a prior weight frame. The enclosing retained
session binds sample, topology, weight and response generations and revokes
inverse authority until the new finite solve certifies.

A separate acceptance pass replays original values minus proposed group centers.
It recomputes weighted residual group moments rather than reusing construction
value sums or a recurrence residual. Fresh raw/within norms supply the existing
relative certificate scale. This check establishes current-weight FE
orthogonality; the implicit difference is in the declared FE model space by
construction. Failed arithmetic or certification leaves the view invalid.
Immutable ownership prevents mutation of codes, membership or column recipes
while a view is borrowed.

No required check is removed or loosened. Finite residual and score checks,
canonical rank and both inverse sides, original-input Poisson scores, two outer
convergence passes, objective checks, reconstruction, strict final Hessian,
separation, weight semantics and transactional publication remain authoritative.
The direct group update reports its actual one-projection work; equal recurrence
bits or zero-versus-one work diagnostics are not scientific requirements.

## Memory and refusal

Admission conservatively adds group state without crediting the old dense/scratch
plan or its projection buffers. For admitted model width M, G groups, T workers
and bound-column descriptor size B, the additional upper charge is

```
8*G*(M+1) + 8*G + 48*(M+1) + M*(8+B) + T*M*(8+B)
```

It covers control/response centers, group totals, six-double column statistics,
identities, finite/inference bindings and worker rows. The existing N-sized
response envelope remains charged. Products retain group storage through
inference; immutable prepared buffers are shared, not duplicated. All size
products and reservations are checked. G may approach N, so this is explicitly
G-by-K resident geometry, not a claim of a universal N-by-poolsize bound or a
smaller peak for every design. Refusal preserves the ordinary certified route
at its charged shared-owner budget.

A public one-byte-below-admission test changes observed scratch from zero to
65,536 bytes and still passes an independent Poisson fit. Measured large-case
tracked peak allocation falls, while the deliberately conservative modeled peak
rises. Neither observation qualifies a physical memory cap or repairs the static
allocation inventory's documented drift.

## Evidence

The [source-bound report](../../benchmarks/reports/development/ppml-implicit-one-fe-local-2026-09-23/README.md)
contains exact source/artifact identities, independent projection/WLS and
Poisson/Hessian/sandwich checks, changing/extreme weights, rank-deficient and
zero-control boundaries, failed refresh invalidation, ownership/drop checks,
interaction partitions, real allocation audits, memory refusal, fresh cases and
excluded-path controls. It preserves the rejected fast adoption, interrupted
runs and small/serial timing losses. The low-memory improvement is useful but
does not complete the whole-command 0.90-times-stock objective.
