# ADR 0124: PPML weight-frame geometry and ordered parallel certificates

Status: accepted for local development, 2026-09-23.

## Decision

Extend the admitted multipartition PPML slope Schur owner from ADR 0123 with
explicit weight-frame geometry authority and an independently optional fresh
certificate result bank. Neither change alters the solver recurrence, stopping
schedule, numerical tolerances or scientific acceptance requirements.

The linear session owns the response and weights and supplies its sample,
topology and weight generations. Identical generations authorize reuse of the
local rank-aware weighted QR bases across compatible absorption calls. Response
and finite-regression basis generations do not change these FE-only bases.
Changing any of the three dependencies rebuilds geometry. Advancing a weight
generation conservatively rebuilds even if the raw weights happen to be equal.
The prepared sample and FE design cannot be rebound in place.

An unframed call never authorizes reuse. Every call takes and clears prior
geometry authority before validation or other fallible work, and restores it
only after successful independently certified absorption. Errors and unwinds
therefore leave no reusable authority. A generation identifies owned state;
this is not pointer equality, an unchecked hash, or a license to mutate weights
without advancing their owning session generation. This supersedes the
always-rebuild restriction in ADRs 0121 and 0123 only for the admitted
coefficient solver called through the PPML slope session.

## Fresh certification

Residual norms remain freshly recomputed. With admitted result storage, each
FE group computes its rank-aware span projections and raw moments from the
current residual, weights and geometry. A bounded 16-RHS tile shares row loads,
weight roots and raw-channel products. Each scalar entry keeps its original
observation order, divisions, compensated summation and FMA sequence. Groups
write disjoint slots on the existing native team. The caller merges every
relative result in the original group/RHS order, preserving the RMS FMA
recurrence and raw maximum. There is no parallel floating-point reduction.

No residual, score, coefficient, projected design norm or acceptance result is
cached by this mechanism. Original-RHS reconstruction, finite solve, model
space, objective, score, final-Hessian, inference and transactional publication
checks remain at their existing owning boundaries. The cheaper coefficient
recurrence still only schedules fresh reconstruction and certification.

## Admission and boundaries

The ordered result bank requires `8*sum(FE levels)*(maximum RHS+1)` bytes.
Its vector descriptor belongs to the existing coefficient workspace, and the
geometry generation descriptor is charged with the geometry bank. All payload
reservations are fallible and actual capacities enter the existing fingerprint.
The bounded local arrays need at most 640 bytes per executing group, within the
existing worker stack envelope. No inner iteration allocates or grows storage.

Admit the coefficient solver first. Then attempt to increase its fit-lifetime
reservation for the certificate bank. Budget or allocation refusal keeps the
Schur solver and its existing serial fresh certificate. Optional scheduling
must not displace an affordable numerical solver. The bank shares the existing
linear-session lifetime, including strict checkpoints, and is released before
inference/recovery. Memory telemetry re-admits the combined reservation.

Categorical PPML, one-effect routes, mu separation, explicit non-MAP families
and public OLS routing are unchanged. Existing generic calls without generation
authority retain call-scoped geometry. This decision does not repair the static
allocation inventory drift or qualify constrained-memory or cross-platform
execution.

## Evidence

The [source-bound development report](../../benchmarks/reports/development/ppml-frame-certificates-local-2026-09-23/README.md)
records the certificate-only and combined
candidates, independent projection and Poisson/Hessian checks, exact native
exports, budget boundaries, phase attribution, fresh rotations and excluded
controls. The broader target of 10% lower public runtime than comparable stock
fixest remains separate from acceptance of this bounded milestone.
