# ADR 0137: concentrated one-FE Poisson in low memory

Status: accepted for local development, 2026-09-23.

## Decision

Extend the concentrated one-intercept-FE Poisson owner from ADRs 0127 and 0132
to low execution. After the unchanged canonical initial rank decision, borrow
the prepared model's original finite columns through their existing numeric
views or generated-column evaluator. Do not read the retained scratch matrix
by random group membership or make a second N-by-K matrix. Fast execution
continues to borrow its retained dense representatives. The short design view
is monomorphized; fallible generated-column evaluation propagates errors.

The session validates sample/topology generations, intercept-only topology and
the frozen canonical active basis before lending original column identities.
No view survives session mutation. Original and group-shifted columns represent
the same concentrated model: subtracting a group-specific vector from X merely
adds its product with b to the analytically optimized group intercept. Every
step still recomputes current means, group-centered moments, Hessian and finite
direction. Neither representation asserts current-weight orthogonality.

This supersedes only the low-storage exclusions in ADRs 0127 and 0132. Actual
one-intercept-FE, MAP-owned publication PPML and full optional-memory admission
remain required. Generated and noncontiguous active columns use the same rank
decision. Zero identified controls retain the analytic group-total result.
Synthetic no-FE, slopes, multiple effects, other engines, mu separation and OLS
keep their existing routes. No case-name, seed or new size threshold selects
this engine. An existing bounded numeric binding is a column-access choice,
not a numerical routing guard.

## Acceptance and failure behavior

The original-input outer scores, two consecutive convergence passes, objective
and line-search rules are unchanged. The concentrated finite solve is checked
against its unfactored current Hessian. Strict canonical WLS independently
establishes current-weight projection, full rank, finite solve, bread and
model-space reconstruction before publication.

ADR 0132's final-product reuse now also applies to admitted low sessions. The
strict result must agree with the accepted predictor and identified coefficients;
weight bits, scale and sample/topology/response/weight generations must match.
Every Hessian weight must still equal q times the accepted mean. A failed
identity or consistency check invokes the existing canonical final solve.
Numerical refusal abandons the optional profiled owner; other errors preserve
transactional failure. Inference, normalized output and saved effects retain
their independent checks. No tolerance, sample, rank, weight, DoF or separation
policy changes.

## Memory and evidence

The low route additionally admits the existing profile workspace: three N-sized
vectors, group totals/shifts, active coordinate identities, finite-system buffers
and fixed group-block moment banks. These remain live through strict solving and
are released before inference. Numeric column binding is stack-only and generated
terms use the existing evaluator. There is no additional finite-design bank.
Budget refusal keeps the previous canonical route. An explicitly requested
allocation audit separately admits cold raw-input storage outside its measured
iteration window; it does not become ordinary runtime storage.

The [source-bound report](../../benchmarks/reports/development/ppml-one-fe-low-local-2026-09-23/README.md)
records independent original/group-shifted design checks, changing weight frames,
small/wide numeric bindings, full-dummy Poisson and sandwich oracles for unit/FW/PW
weights and generated terms, stale-final-authority rejection, real allocation
audits, excluded-path controls, timing distributions and remaining stock losses.
This is a substantial low-mode improvement, not completion of the whole-command
0.90-times-stock objective or existing memory/platform qualification gaps.
