# Two-FE Auto structural gates

## Gate v2 (public default)

Public OLS Auto/Auto with exactly two categorical-intercept FEs uses gate v2
(`routing_two_gates_v2`): gate v1 below, plus three earlier decisions. At local
steps 4, 8 and 12 of the diagonal Schur recurrence, a packed or outcome
projection builds the CMG hierarchy when some active lane's preconditioned
energy still exceeds `0.5^(2 age)` times its initial energy, that is, when its
average residual contraction per step since the pilot began is at least one
half. Such a lane would need far more than the 16-step pilot to reach a 1e-8
target. The energies are the recurrence's own serial canonical reductions and
activity state, so the rule keeps no bank, scan or clock and decides
identically at every worker count. A lane without valid history (an underflowed
start or an invalid energy) leaves that age undecided. Final and unknown roles,
local step 16 and everything after it follow gate v1.

The threshold was frozen from one development measurement on 2026-10-27: no
routing family that finishes without a hierarchy exceeded 0.33 at any age, the
two synthetic Veneto-shaped AKM panels measured 0.57 and 0.59 at age 4, and the
weak routing families crossed at age 8 or 12. The hidden v2 injection keeps the
public two-FE ownership and deferred-cell activation, so it equals uninjected
Auto on every design.

## Gate v1

The qualified OLS Auto/Auto policy selected gate16 `feature16 <= 0 → Build` and
gate32 `Defer`. Within the separately validated packed/outcome/final action scope,
this builds the first eligible packed or outcome action at step 16 and defers a
final residual. Regressor-only and one-off first episodes close Unknown; they
must not pass merely because `final == false`.

The runtime preserves owner-level closure, current-key eligibility, readiness,
positive remaining allowance and existing direct-route semantics. The selected
pair needs no progress banks, proxy scans, historical action counters or economic
clocks. No elapsed-time routing or construction-work cap is introduced.

Keep cutoff 5, one sampled tree, seed zero, eligibility, small-direct selection,
certification, restart/fallback and the cumulative iteration budget. Changes in
solver trajectories must satisfy SPEC's independent scientific witnesses and
stable discrete decisions. Explicit developer injections retain their contracts.

The fixed Mac/Linux qualification passed all 84 protected bounds per platform;
public/hidden activation passed after the documented transport-accounting
amendment. Ownership and deferred cell construction subsequently passed their
separate qualification. See [current validation](VALIDATION.md).
