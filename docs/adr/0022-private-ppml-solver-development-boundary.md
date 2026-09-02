# ADR 0022: private PPML FE-solver development boundary

**Status:** Accepted for development; no route activation
**Date:** 2026-09-02
**Decision owner:** Johannes Schmieder

## Context

ADR 0018 pins production PPML to MAP until a changing-weight route passes its
own scientific, memory, and end-to-end gates. The shared weighted-linear
engine also contains CG, Schwarz-LSMR, and CMG implementations, but the former
PPML boundary could not exercise them without editing production policy. That
made it impossible to distinguish a promising inner projection from a route
that is merely compiled and qualified for OLS.

## Decision

1. Public `feppml` remains unchanged. `fesolver()`, `fepreconditioner()`, and
   `solvertrace` are accepted only by `_fereg_estimate_ppml` outside
   `publicmode`; public-mode use fails before the substantive plugin call.
2. Non-MAP routes and tracing require
   `FEREG_PPML_EXPERIMENTAL_SOLVERS=1` independently at the Stata adapter and
   Rust core boundaries. This tranche introduced PPML wire 1.3 and names
   `fe_solver`, `fe_preconditioner`, `fe_lsmr_mode`, `fe_lsmr_policy`,
   `solver_trace`, and `solver_experiment`. The typed LSMR modes are `legacy`,
   `fused`, and `joint`; their state policies are `diagonal`, `reuse`, and
   `rebuild`. ADR 0023 extends the current request to wire 1.4 with a typed
   fused-CMG mode. Production requests carry no development solver state.
3. The experiment is limited to fast memory, two or three categorical
   intercept-only absorbed effects, no saved effects, and no allocation
   audit. Schwarz is admitted only for LSMR. Auto still canonicalizes to MAP;
   no PPML automatic threshold or LSMR fallback is added. ADR 0023 separately
   permits an explicit private fused-CMG-to-MAP numerical fallback.
4. `legacy` uses the shared weighted-linear session. `fused` projects every
   frozen active control and the current working response in one categorical
   LSMR batch, then solves the small weighted FWL system locally. `joint`
   applies LSMR to one augmented control-plus-categorical-FE Newton operator
   and may warm-start the correction vector. Fused Schwarz state may be
   retained or rebuilt; joint state has explicit staleness, rebuild, and
   certificate-continuation rules. Route-specific workspace is checked
   against the selected PPML memory plan before construction. The private
   engines make no steady-state allocation claim in this tranche.
5. The outer algorithm remains PPML IRLS. The selected route changes only the
   inner weighted least-squares FE projection. Controls remain finite columns;
   absorbed effects remain native FE operators.
6. Fused and joint LSMR are allowed only on ordinary publication IRLS frames.
   The production MAP weighted-linear engine freezes the active basis on the
   first frame and owns every strict and final-Hessian checkpoint. A
   checkpoint requires the canonical rank and basis, full linear predictor,
   objective, score, and every estimable frozen-basis coefficient to agree.
   Nonestimable normalization coordinates do not veto convergence; the
   canonical strict state replaces them before publication. Mu separation
   remains entirely canonical.
7. Every experimental ordinary-frame result is checked by a fresh exact
   combined normal-equation certificate over controls and absorbed FE levels.
   A recurrence stop is never sufficient. Fused and joint may make one tighter
   recurrence pass after an uncertified result; the reuse policies rebuild
   Schwarz state for that pass. Both still fail closed if the original
   requested certificate does not pass. There is no silent public fallback.
8. Private trace schema 1 records topology and the mixed checkpoint boundary:
   MAP `fresh`, `strict`, `final_hessian`, and `fit_end` events plus the
   `fused_fwl` or `joint_incremental` LSMR events. It includes requested and
   used preconditioner, generation, scheduled/effective tolerance,
   setup/solve time, state reuse/rebuild, exact certificate norms, Hessian
   range, and strict-profile differences. Trace runs are untimed and cannot
   qualify a route.
9. The non-qualifying development manifest uses an exact pinned `ppmlhdfe`
   closure and a separately implemented explicit-dummy oracle. Its small-cell
   science gate checks the estimation sample, control coefficients, robust
   control VCE block, total model and derived residual degrees of freedom,
   objective, convergence, and native-route agreement with MAP. Native routes
   must also preserve MAP's exact rank/absorbed-DoF decomposition. This
   distinction is necessary because `ppmlhdfe` counts the normalized constant
   in absorbed DoF while fereg exposes it as an omitted finite coordinate, and
   `ppmlhdfe` does not publish residual DoF for robust VCE. Every timed arm runs
   in an isolated Stata process after a warm-up; trace runs are separate. Stata
   process status is insufficient without a terminal sentinel and a complete
   result file.
   The default pinned `ppmlhdfe` comparison is verified at runtime as
   conjugate-gradient acceleration of symmetric Kaczmarz; it is not LSMR. A
   second `ppmlhdfe, accel(lsmr)` arm measures forced LSMR explicitly.
10. Screening, scale/thread, holdout, and SCC development work may proceed only
   for a route that passes every small science cell and the complete
   projection-potential gate. A failed route remains available only through
   the doubly gated private boundary for diagnosis.

## Local development evidence

The original eight-cell, 400-observation science matrix rejected the direct
shared-session CG, LSMR, and CMG candidates described in the prior tranche.
After adding the fused and joint engines, a fresh local one-repetition matrix
over the same eight topologies passed without sample, coefficient, VCE,
objective, DoF, convergence, native-MAP, independent-dummy, process, or trace
failures for both Schwarz-reuse engines. The matrix is a correctness screen;
its launch-dominated timings reject both engines under the frozen performance
gate and do not justify activation.

Bounded 25,000-observation, one-thread local screens then isolated the useful
regions. On balanced two-FE/easy/K=10, medians were 0.165 seconds for MAP,
0.167 for fused reuse, and 0.162 for joint reuse; the pinned `ppmlhdfe`
default and forced-LSMR medians in the matched comparison were 0.465 and 0.525
seconds. On the deliberately long-connectivity K=10 cell, medians were 3.810
seconds for MAP, 2.296 for fused reuse, and 2.521 for joint reuse; matched
comparator medians were 25.112 and 65.856 seconds. At K=50/easy, medians were
0.841 for MAP, 0.844 for fused, 0.593 for joint, 4.167 for `ppmlhdfe` default,
and 2.702 for forced LSMR. At K=50/long, three-run candidate medians were 6.549
for fused and 6.372 for joint, against a one-run 12.877-second MAP diagnostic.
All reported fereg screens matched MAP estimates and objective at displayed
precision. These are targeted dirty-worktree development measurements, not
frozen, cross-platform, scale, memory, or release claims.

The evidence supports continued topology/control-width experiments, not a
public rule. MAP remains the sole production PPML route. No holdout, SCC,
route-region selection, allocation qualification, or activation run was
performed.

## Consequences

- PPML solver reachability can be measured without weakening public behavior
  or silently borrowing OLS routing decisions.
- Failure evidence is route-specific: one candidate cannot veto or validate
  another, and incomplete trace/process output fails closed.
- The next useful engineering targets are localized: remove remaining
  per-frame owned-weight and projected-output allocation in the fused engine,
  reduce joint certificate retries, and calibrate a measured MAP/fused/joint
  region over topology and control width. None may affect OLS routing.
- ADR 0018 remains controlling for production and qualification. This ADR
  narrows only its prohibition on exercising explicit PPML routes by adding a
  doubly gated, non-public development path.
