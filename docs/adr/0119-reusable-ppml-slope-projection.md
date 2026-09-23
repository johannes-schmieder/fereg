# ADR 0119: Reusable parallel PPML slope projection

Status: accepted for local development, 2026-09-23.

## Decision

Extend ADR 0118's group-coefficient/row kernel to the generic reusable MAP
owner of a public PPML fit. Admit it only for the resolved MAP engine, at least
16,384 final observations, a slope-bearing design, positive RHS capacity and
more than one resolved worker. The existing PPML session installs that explicit
team for every linear frame. Categorical specialized projectors, serial/small
fits, other PPML engines and Mu separation exploration retain their execution.
Mu exploration has a separate parent separation envelope; this admission does
not establish permission to enlarge it.

Retain immutable topology and optional scratch capacity from session creation
through its last canonical Hessian solve. Every effect projection clears and
recomputes the group coefficients and raw weighted Gram from its current
weights and RHS. No weighted geometry, working response, residual, coefficient
solution or certificate becomes reusable scientific state across frames.
Observation/channel arithmetic, rank/pivot rules, MAP sweeps and checks,
convergence thresholds, score tests, final-Hessian synchronization, outputs
and transaction boundaries are unchanged.

The optional bank is `8*(L*C*R + T*C*C) + T*sizeof(ProjectionWorker)` bytes,
using maximum effect levels L, channels C, session RHS capacity R and resolved
workers T. Fast R includes response and all generated columns; low R is the
existing bounded projection pool. Retain the original serial workspace.
Admit the additional bank beside the complete selected fit plan and any
mandatory solver storage before allocation. Insufficient budget or size/
allocation refusal keeps serial projection; an optional bank cannot veto the
already admitted route. Actual buffer capacities must fit the bank bound.

A separate `reusable_projection_bytes` field charges its full lifetime in
preparation, weight refresh, ordinary/strict linear work and line search.
The session releases it before inference, recovery and publication. Fresh
telemetry reconstruction must re-admit this charge. Solver slack excludes
this resident bank. Gram, coefficient and worker capacities enter the exact
session capacity fingerprint; no size-dependent buffer grows during IRLS.
The existing runtime/stack headroom and whole-action proof limitations remain.
This is no replacement for the failed static allocation inventory.

## Evidence and closeout

The [local report](../../benchmarks/reports/development/reusable-ppml-projection-local-2026-09-23/README.md)
records paired median gains of 27.8–37.8% on four eligible cases, including
fresh wider T8 and low-memory fits. Added tracked heap is 35–656 KB. All 26
native pairs including pilots have exact scientific exports and non-clock
solver fields; all 20 saved-mean pairs are exact. Independent explicit Poisson,
Hessian, rank and public-state checks pass. The strict, unmodified stock
comparator still wins slope timings by substantial margins. Small excluded
control losses are retained.

Sixty-five distinct Rust tests, actual-plugin ABI/panic checks and four
licensed fixtures pass. The separate hidden PPML transaction fixture fails
on both baseline and candidate at its stale minor-version assertion (2 versus
current 3); it remains unchanged and is not counted as passing.

Close this scheduling change without tuning thresholds, group chunks or
seeds. Profiles retain 1,476/1,240 projections on the two diagnostic cases;
projection time falls by about half while certificates are unchanged. The
next useful PPML investigation is reducing repeated certified slope MAP work,
including an appropriately scoped accelerator and fresh certificate costs,
with explicit changing-weight ownership and independent validation.
