# ADR 0025: public PPML solver families and reporting

**Status:** Accepted
**Date:** 2026-09-02
**Decision owner:** Johannes Schmieder

## Context

The changing-weight PPML engine now has certified MAP, fused and joint LSMR,
and prepared fused-FWL CMG implementations. ADRs 0022--0024 deliberately made
those alternatives private while they were developed. Keeping their private
spellings beside a new public surface would create two policy planes and make
solver use impossible to interpret from stored results.

## Decision

1. Public `feppml` accepts exactly `solver(auto|map|lsmr|cmg)`. Omission means
   `auto`. Solver values are case-normalized but not abbreviated. Unknown
   values are syntax errors with `r(198)`.
2. `auto` selects MAP until a separate calibration decision changes it. MAP,
   LSMR, and CMG are explicit public families. LSMR selects one concrete fused
   FWL or joint incremental engine once per fit. CMG means only the prepared
   fused-FWL engine; there is no legacy or bare private CMG interpretation.
3. MAP owns the initial basis-freezing frame and all strict and final-Hessian
   checkpoints. LSMR and CMG operate only on ordinary publication IRLS frames.
   Mu separation remains canonical MAP.
4. A numerical CMG nonconvergence retries the same frame with certified MAP and
   permanently disables CMG for that fit. Allocation, unsupported topology,
   invalid data, interruption, and internal errors do not become fallback.
   LSMR has no silent MAP fallback.
5. Request wire 1.5 replaces `finite_solver`, `fe_solver`, `fe_preconditioner`,
   `fe_lsmr_mode`, `fe_lsmr_policy`, `fe_cmg_mode`, `solver_trace`, and
   `solver_experiment` with one required `solver` field. Those retired keys are
   not aliases. Exact older request versions fail with `r(499)`.
6. The first public LSMR envelope is two or more categorical intercept
   effects; CMG accepts exactly two or three. Both support the ordinary PPML
   result and saved-fixed-effect surface through final MAP-owned recovery and
   require fast-memory execution.
   `memory(auto)` may select fast; explicit `memory(low)` and allocation audit
   are not yet supported for these families. Recognized requests outside the
   envelope return `r(498)`. A supported selected route that cannot fit its
   checked workspace returns `r(909)`.
7. Result wire 1.2 adds a fixed 4x22 solver matrix. Rows are `map`,
   `lsmr_fused`, `lsmr_joint`, and `cmg_prepared`. Columns are, in order,
   `requested_family`, `selected_family`, `selected_engine`,
   `effective_family`, `effective_engine`, `checkpoint_family`, `row_engine`,
   `ordinary_frames`, `checkpoint_frames`, `retries`, `fallback_count`,
   `fallback_reason`, `iterations`, `setup_seconds`, `solve_seconds`,
   `certification_seconds`, `absinf_max`, `relinf_max`, `rel2_max`,
   `normal_equation_max`, `projection_absinf_max`, and `backward_error_max`.
   Request/selection/effective/checkpoint identities repeat on every row;
   unobserved work counts and times are zero and inapplicable certificates are
   Stata missing.
8. Family codes are auto 1, MAP 2, LSMR 3, and CMG 7. Engine codes are MAP 1,
   fused LSMR 2, joint LSMR 3, and prepared CMG 4. Fallback code 0 means none;
   code 1 means CMG numerical nonconvergence.
9. `e(feppml_solver)` publishes the matrix. The macros `e(solver)`,
   `e(feppml_solver_requested)`, `e(feppml_solver_selected)`,
   `e(feppml_solver_engine)`, `e(feppml_solver_effective)`,
   `e(feppml_solver_engine_effective)`, `e(feppml_solver_checkpoint)`, and
   `e(feppml_solver_fallback_reason)` publish named identities;
   `e(feppml_solver_fallback_count)` publishes the count. The existing
   `e(feppml_linear_route)` remains a compatibility alias for checkpoint MAP,
   not the ordinary-frame effective route.
10. The solver shadow is fully validated before `ereturn`. It is stored before
    the incomplete receipt; runtime is penultimate and the receipt COMPLETE
    cell remains the final native store. No solver failure may publish partial
    results. OLS policy and wires are unchanged.

## Consequences

- The public interface names stable solver families without exposing tuning,
  preconditioner, trace, or engine-policy controls.
- Concrete engine and fallback reporting make performance comparisons
  reproducible while allowing internal LSMR optimization.
- Auto-routing remains deliberately uncalibrated. Development should optimize
  the three explicit solver families locally first, then use small parallel SCC
  samples before any frozen calibration or qualification campaign.
- Historical private reports remain evidence for their exact source and
  manifest. Their private command spellings do not survive as compatibility
  code in the current request wire.
