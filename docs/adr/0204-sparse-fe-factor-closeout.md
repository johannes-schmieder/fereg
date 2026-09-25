# ADR 0204: reject the sparse categorical FE-factor prototypes

Status: rejected for production, 2026-09-25. Development evidence, not qualification.

## Decision

Retain **3e1d9120 / higherdim1** computation and routing. Neither **sparsepublic1**
nor its bounded correction **sparsecanonical1** enters production. Both were
implemented in isolated source snapshots; no production solver is removed or
changed. The [source-bound report](../../benchmarks/reports/development/ppml-sparse-fe-factor-closeout-local-2026-09-25/README.md)
preserves implementations, actual-frame captures, independent oracles, public
distributions, memory observations and the failed initial instrumentation attempt.

The prototypes factor the grounded full bipartite categorical FE Laplacian with
sparse LDL. Immutable component membership, ordering and symbolic fill persist
within one fit. Numerical factors are rebuilt under each changed current-weight
frame. Compatible RHSs share factors; no previous-weight factor establishes
current-weight authority. Static degree ordering and grounding the largest-degree
vertex are diagnostic choices, not an accepted routing or rank policy.

## Evidence and bounded correction

Seven actual initial/ordinary/strict weight and response/control frames pass an
independent explicit-design weighted projection oracle for 41 RHSs. The large T4
kernel requires about 3.4 ms per numeric-factor/projection call, with 7.3 ms of
symbolic setup. This excludes several public consumers. Comparable whole-command
work, rather than the kernel clock, governs adoption.

The first integration replaces ordinary joint directions and fused canonical
construction with full WLS through the categorical projector. It preserves the
existing fresh projection certificate but worsens large-case median time from
106 to 155 ms and narrow time from 37 to 43 ms. Its measured projection and finite
work increase substantially despite unchanged outer iteration counts.

The correction preserves the ordinary joint owner and the existing fused
canonical products, direct residual Gram, original-input certificate and output
reconstruction. Only the canonical coefficient proposal changes. The full signed
FE system receives zero RHS on the eliminated partition and the centered group
RHS on the retained partition. Its returned retained coefficients feed the
existing reconstruction; numerical gauge differences do not authorize a result.

Three baseline/candidate/stock rotations produce **107/108/97 ms** for the large
disconnected low case, **38/40/32 ms** narrow and **71/73/81 ms** for excluded
T8 slopes. The correction wins three and loses six of nine native pairs, with
all three narrow pairs losing. Large strict-solve telemetry improves about
3.4 ms, but projection/setup worsens about 5.2 ms. Tracked native peaks increase
approximately 6.68 MB large and 3.15 MB narrow; scratch is unchanged. The added
factor lifetime is not justified by the measured public result.

Both prototypes pass the three selected licensed independent scientific fixtures.
Across both public pilots all 54 states, 36 baseline-anchored pairs and eighteen
direct stock comparisons pass. This establishes the bounded scientific behavior
observed, not complete production admission. Their allocation, capacity,
numerical-refusal and fallback lifecycles remain incomplete. No fresh validation
seed, production memory qualification or additional tuning follows rejection.

## Consequences

Close this sparse-factor organization after its one correction. Do not fit
ordering, fill, size, thread or phase thresholds to these cases. Retain the
current exact-weight joint/canonical engine and independent acceptance schedule.
This does not establish that every sparse factorization is unsuitable; another
proposal would need materially different organization and a new measured public
opportunity, including setup and all consumers.

The remaining deficits require whole-command work removal or a larger measured
opportunity in another family. Current mutable priorities belong in STATUS.md.
Existing disconnected scientific discrepancies, OLS timing losses, static
allocation-inventory drift and constrained-fit/nonretaining-CG failures remain
unresolved. No tolerance, sample, rank, DoF, weight, inference, separation,
certificate or failure-atomicity policy changes. The broad multi-FE performance
goal remains active.
