# ADR 0203: admit coefficient and joint solves for fast higher-dimensional PPML

Status: accepted for local development, 2026-09-25; not qualification.

## Decision

Extend the existing publication MAP coefficient/joint owner to fast categorical
PPML with more than three FE partitions and finite controls. ADR 0136 already
admits this organization in low storage. The existing categorical checkpoint
engines cover at most three partitions; fast higher-dimensional calls previously
fell through to repeated generic observation-space MAP. This capability boundary,
not a benchmark name, seed, group count or measured crossover, defines eligibility.

Reuse the established coefficient Schur/CG canonical projector, current-weight
group geometry, joint finite/FE ordinary directions and strict-final transaction.
No solver layer, public option, dependency or tolerance is added. The production
change is the admission condition; focused tests exercise the newly admitted path.
One-FE, fast two-/three-categorical-FE, existing slopes, low storage, explicit
non-MAP families and OLS keep their routes. In particular, ADR 0136's rejected
all-coefficient two-/three-FE canonical experiment remains rejected.

## Computation and scientific authority

Ordinary joint directions avoid constructing the complete residualized RHS bank
and finite inverse at every IRLS step. Canonical boundaries use weighted group
geometry prepared for that frame and coefficient-space products instead of
repeated generic projections of observation residuals. Initial canonical rank,
strict current-weight Newton validation and independent final construction keep
their distinct duties. Changed weights invalidate numerical geometry; previous
solutions are guesses only. No authority crosses a sample or weight generation.

The independent current-weight projection, finite/FE score, model-space,
rank, finite-solve, reconstruction, objective and exact final q*mu Hessian
requirements are unchanged. Sample, separation, weight, inference, DoF and
transactional publication semantics are unchanged. No check scheduling or
threshold relaxation is part of this decision. Existing check redesigns remain
governed by their own ADRs.

## Memory and fallback

Admission charges the complete existing certificate geometry, coefficient banks,
previous-response storage and joint finite payload before allocation. Optional
coupling retains its separate admission. Budget refusal preserves the previous
generic route; required allocation failures remain transactional. A focused test
forces refusal and independently checks the resulting means and final Hessian.

For successful matched fast cases, additional tracked native peaks range from
9,575,928 to 23,776,720 bytes across the pilot and fresh blocks. Selected-memory
increments range from 10,986,392 to 24,624,664 bytes; scratch is unchanged.
These are tracked/model telemetry, not physical-RSS or constrained-cap results.
T1/T4 production-plugin audits of the new route each pass sixteen warmed
iterations with zero allocation events/current growth, stable capacities and
fresh/warm equivalence. Existing static-inventory drift remains open.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-higher-dimensional-map-local-2026-09-25/README.md)
binds higherdim1 to d46b584e plus two source overlays, all 565 computational
inputs and the measured plugin. Independent tests include paired analytic
Poisson truth, explicit-dummy q*mu Hessian/sandwich calculations, unit/FW/PW,
robust/cluster inference, offsets, nested/rank-deficient designs, generated
terms, disconnected extreme masses, T1/T4, fast/low and budget refusal.

Fresh seed202609435, five B/C/stock rotations, gives ranked candidate/stock
medians of 169/435 ms (four dense FE), 394/10805 (four sparse FE), 112/250
(five unbalanced FE) and 154/205 (larger four unbalanced FE). Every pair in
those four cases clears 0.90 stock. The sparse baseline fails all five warm-ups;
it has no baseline full-fit clock. The dense baseline median is 15029 ms.
The pilot retains a 725-ms candidate outlier on its larger dense case.

The disconnected five-FE case stays unranked: all baseline calls time out, while
candidate and stock disagree on 475 fitted means despite passing individual
state checks and agreeing on finite coefficients, covariance and objectives.
A separate tighter stock diagnostic reduces that count to fifteen, without
resolving it. This is evidence of reference stopping sensitivity, not proof of
the remaining error's ownership. No validation seed is used for source tuning.

Excluded-path losses remain visible: narrow PPML pilot medians are 36/38/32 ms
(B/C/stock); low four-FE fresh medians are 199/202/434. All nine OLS pairs have
exact scientific exports, but the slope control loses two ms in every pair.
This checkpoint does not establish nonregression for every other subsystem or
complete the broad multi-FE <=0.90-stock goal. Existing scientific discrepancies,
constrained-fit failures and platform deferrals remain open.
