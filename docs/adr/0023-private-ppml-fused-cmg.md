# ADR 0023: private PPML fused-CMG development path

**Status:** Historical development decision; public-admission clauses superseded by ADR-0025
**Date:** 2026-09-02
**Decision owner:** Johannes Schmieder

ADR-0025 supersedes this ADR's private wire, environment gate, and MAP-only
public-admission clauses. Its fused-CMG numerical, retry, and fallback decisions
and the evidence below remain authoritative historical input.

## Context

ADR 0022 established a doubly gated PPML solver laboratory while leaving
public `feppml` and OLS unchanged. The shared CMG implementation was reachable
there only through the generic weighted-linear session. That route projects
generic batches, runs observation-space CG polish, and cannot exploit the
frozen finite basis or retry only the failed columns of a PPML working frame.

IRLS changes the Hessian weights on every frame. Without an upstream CMG
numeric-refresh API, a hierarchy built for one frame is not valid for the
next. A useful no-upstream-change tranche must therefore optimize work within
each frame, report hierarchy construction separately, and avoid claiming
cross-frame numeric reuse.

## Decision

1. PPML request wire 1.4 adds `fe_cmg_mode`. Its only nonempty value is
   `fused`, it requires `fe_solver=cmg`, and it remains unavailable through
   public `feppml`. Both the Stata adapter and Rust core require
   `FEREG_PPML_EXPERIMENTAL_SOLVERS=1`.
2. The envelope remains `memory(fast)`, exactly two or three categorical
   intercept-only absorbed effects, no saved FE, and no allocation audit. A
   three-FE problem is admitted only when the non-base nuisance block has at
   most 64 columns. Ineligible topology, resource, protocol, and data errors
   fail closed.
3. MAP remains the canonical authority. It solves the first frame, freezes the
   active finite basis, owns Mu exploration, and solves every strict and
   final-Hessian checkpoint. Fused CMG is used only on ordinary publication
   IRLS frames.
4. Each ordinary frame constructs the CMG hierarchy for that frame's current
   Hessian weights. The PPML engine projects the frozen active controls and
   working response together, omits generic observation-space CG polish, and
   independently certifies every projected right-hand side.
5. If any projection column fails, only those columns are retried at
   `max(1e-14, scheduled_tolerance * 0.01)`. Two-FE retry reuses the same
   frame hierarchy. Three-FE retry rebuilds the hierarchy because its nuisance
   Schur factor was solved at the original tolerance. Failure after that retry
   is numerical nonconvergence.
6. The projected controls and response feed a deterministic, row-blocked
   weighted FWL reduction. Block totals are merged with compensated sums, and
   the standardized finite system uses the shared pivoted-Cholesky kernel.
   The resulting residual must pass a fresh combined certificate over every
   active control and every absorbed-FE level before IRLS can accept it.
7. A numerical CMG failure triggers a same-frame certified MAP solve and
   permanently disables CMG for the rest of that fit. The route fallback is
   explicit in telemetry and trace. Unsupported topology, memory failure,
   invalid input, interruption, and internal errors never become MAP fallback.
8. Fereg-owned active-control, projected, retry, finite-system, certificate,
   residual, and predictor buffers are retained across frames and charged
   before construction. The CMG constructor then performs its topology-aware
   memory preflight before hierarchy allocation. This tranche makes no
   allocation-free or cross-frame hierarchy-reuse claim.
9. Private trace schema 1 adds `fused_fwl_cmg` with separate setup, graph
   solve, and finite-solve time; RHS and batch counts; nuisance width; retry
   counts; recurrence backward error; projection and combined certificates;
   and the explicit fact that observation-space CG polish was skipped.
10. The existing OLS `CmgAbsorber::build` and `project_matrix` behavior remains
    intact, including CG polish and OLS fallback. The new unpolished seam is
    crate-private and is called only by the PPML-owned engine.

## Local development evidence

Focused Rust tests cover typed admission, two- and three-FE agreement with the
production MAP PPML oracle, selective single-RHS retry, and permanent MAP
fallback after injected numerical failure. Existing CMG/OLS unit tests remain
green. The universal macOS plugin build passed architecture, dependency,
export, loader, and panic-containment checks.

The complete local one-repetition, 400-observation science matrix passed all
eight registered topologies with no scientific, native-MAP, explicit-dummy,
process, or trace failures. Its fused-CMG projection geometric-mean ratio was
0.584 relative to MAP and every projection cell was faster, but launch-dominated
end-to-end ratio was 1.085; the harness therefore labels the result
`reuse-engineering`, not activation evidence.

Two diagnostic 25,000-observation, one-thread, K=10 screens illustrate why
future routing must be conditional. Balanced two-FE medians were 0.147 seconds
for MAP, 0.129 for fused CMG, and 0.441 for pinned default `ppmlhdfe`.
Balanced three-FE medians were 0.211, 0.368, and 0.528 seconds respectively.
All arms matched the registered scientific outputs. These dirty-worktree local
measurements are neither frozen performance claims nor qualification.

## Consequences

- The strongest no-upstream-change CMG/FWL design can now be measured without
  changing public PPML or OLS behavior.
- The next CMG round should measure topology regions and profile hierarchy and
  nuisance-Schur construction. Cross-frame acceleration would require a
  separately reviewed upstream numeric-refresh design.
- MAP remains the only production PPML route. No automatic rule, SCC run,
  holdout, memory qualification, thread qualification, or release claim is
  authorized by this ADR.
