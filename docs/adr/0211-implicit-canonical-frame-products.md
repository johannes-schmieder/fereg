# ADR 0211: implicit two-FE canonical frame products

Status: accepted for local development, 2026-09-25.

## Decision

Admit the existing full-frame two-categorical-FE canonical product pipeline when
its dense reduced geometry does not fit the current workspace allowance. Use a
grounded, diagonally scaled implicit Schur operator in the same independently
stopping component/column CG recurrence. Every affordable dense direct or dense
iterative route retains its previous arithmetic. No public solver option or
ordinary IRLS engine is added.

The previous path declined large reduced shapes and returned to observation-space
construction, with repeated normalization, projection, materialization and finite
product passes. This change makes the complete certified product handoff possible
at those shapes. It differs materially from rejected ADR 0200: that experiment
replaced only the operator inside already admitted iterative construction.

Combine current-weight group-pair masses once. Eliminate the larger partition;
apply `diag(m_b / root^2) - C'C` in grounded component-local coordinates. Positive
prefix/suffix sums construct the Schur diagonal without subtracting nearly equal
group masses. Packed incidence and component descriptors are shared by all RHS.
Each component/column stops independently under the existing CG tolerance and
iteration cap. Dense and implicit operators share one recurrence implementation.

Topology and numerical geometry belong to the current call. Changing weights,
sample or design requires fresh construction; no weighted state is cached across
PPML generations. The existing pair-assembly work preflight remains. Eligibility
depends on complete storage bounds, never benchmark identity or seed.

## Accuracy and failure behavior

CG is a proposal. The existing independent acceptance reconstructs residuals from
original inputs and computes every current-weight FE group certificate and the
finite Gram on those residuals. Reference means/norms, canonical rank, finite
solve, original score, model space, objective, reconstruction and exact final
q*mu Hessian checks remain. The retained dense/scratch bank is written once after
the certified products are available. No tolerance, sample, weight, DoF,
separation, VCE or publication policy changes.

Numerical or iteration refusal returns to the existing certified path. Fatal
errors propagate. Existing strict transaction/rollback and response-anchor
ownership remain authoritative, including refusal after physical retained-bank
writes. Allocation failure cannot publish a partial fit.

Independent tests compare the operator with explicit weighted elimination and
the complete frame with analytic weighted contrasts. They cover repeated pairs,
disconnected components, changing weights, extreme common weight/column scales,
FE-collinear columns, threads, dense/scratch equality, exact and one-byte-short
workspace, and unfinished solves. Affordable dense construction remains covered.

One routing test's former topology became newly admissible. Change only its
degree/Q from 6/160 to 4/158, preserving N and every existing assertion: cold
dense-CG and implicit storage both refuse, while observed-work-admitted dense
direct construction fits. This retains the work-history, late reclamation,
changed-weight, rank and rollback boundary rather than weakening its checks.
The original failed attempt remains in the receipts. Frozen benchmarks are
unchanged. A new small repeated-pair development topology makes independent
Poisson/Hessian tests exercise implicit admission, not just the dense control.

## Memory and scope

Implicit preparation reserves at most N incidence values, N indices, P
three-index descriptors and Q diagonal values, in addition to the existing
group/RHS and three-vector Krylov workspaces. Count the entire coexistence plan,
including actual capacities, before reclaiming an obsolete projection panel.
Use the existing temporary allowance; add no N-by-K bank or separate worker team.
Geometry and solve scratch are dropped before certificate/product reconstruction.
The normalized-weight allocation still becomes the response.

The path serves initial and strict/final canonical PPML construction. Three FE,
slopes and one-shot OLS retain their engines; categorical ordinary IRLS retains
its existing joint solve. Safe-core, main-thread SPI, FFI and transactional
publication boundaries do not change.

## Evidence

The [source-bound report](../../benchmarks/reports/development/ppml-implicit-canonical-frame-local-2026-09-25/README.md)
records production identities, separate diagnostic builds, complete distributions,
independent science, memory and interrupted/contended attempts. Local development
evidence does not constitute qualification or a universal performance claim.


Large narrow two-FE PPML improves 18–23% on development/fresh cases, with all 91
completed production B/C science comparisons passing. Fresh737280 fast reaches
762/759 ms against comparable stock; low remains836/765. Two fresh families meet
0.90 in every comparable pair, three remain slower than the target and three
stock comparisons are scientifically unranked. Four-slope and serial controls
show small timing losses, retained in the report. All 12 protected OLS scientific
export pairs are exact. Forty-four focused Rust tests, five licensed fixtures
and 18 independent Poisson/Hessian/sandwich arm checks pass; the known baseline
report-flag failure remains visible. No source or guard tuning follows seed 441.

Move to prepared-sample/final-result ownership and replay: production preparation
and publication still cost roughly 232–250 and 124–129 ms on the million-row case.
The implicit operator and combined canonical certificates alone cannot supply
the remaining223–324 ms needed for0.90 stock. Low retained finite-rank scans remain
an additional measured cost. Do not reopen the standalone rejected incidence
crossover or weaken independent acceptance to force parity.
