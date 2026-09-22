# ADR 0095: bound serial retained OLS LSMR transport

**Status:** Implemented in local development checkpoint; Linux acceptance deferred
**Date:** 2026-09-21
**Decision owner:** Johannes Schmieder

## Scoped policy

Process one RHS at a time inside the original-input LSMR fallback of a wide
public serial two-FE OLS Schur attempt that requests retained coefficients.
Require 16–64 actual RHS columns and both requested and installed thread counts
equal to one. Successful Schur solves, reconstructed-candidate CG continuation,
narrow fits, final single-RHS recovery, explicit solvers and shared callers
retain their existing transport and admission policy.

Public OLS enables retained wide fallback products only when saved effects are
requested and final-target recovery is allowed. The shared fit boundary clears
that request before this Schur solve. The new policy is selected at that
existing private boundary; it is not inferred from thread-local state, exposed
as a public option, or selected by timing probes.

Pass the same private batch policy through automatic admission, any retained
operator preparation, cold transport and preconditioner fallback. The memory
model charges one live recurrence and the complete returned coefficient and
residual banks. Windowing changes neither the copied input bank nor the
retained operator model. Preserve the generic estimator's default model.

All windows use one fixed operator. Append results in their original RHS order.
Any later-window error drops the earlier results; no partial batch reaches
certification or publication. Preserve automatic LSMR's recurrence tolerance,
iteration budget, preconditioner fallback, fresh residual certificate and
original-input CG fallback. Charge the full setup and solve cost.

## Evidence and remaining acceptance

The [generic serial experiment](../../benchmarks/reports/development/serial-lsmr-window-local-2026-09-21/README.md)
found 122–136 MB lower tracked peaks with exact native results and essentially
unchanged paired runtime. Its 1 GB usable ring fit passed where the earlier
unwindowed route had timed out at 1.5 GB. That experiment changed generic
transport and does not establish acceptance of this narrower implementation.

Before accepting the scoped change, check exact transport equivalence and
later-window failures, the independent MAP forced-cap oracle, unchanged default
memory estimates, measured whole-fit peaks, constrained admission, paired
runtime and excluded-path controls. Confirm Linux behavior as required by the
milestone. A smaller modeled peak alone is not an allocation proof or a reason
to weaken an existing admission guard.

The scoped `nrt2` build passes the actual ABI check, exact cold/prepared
transport comparisons, later-window error checks, default/adapted memory
estimates and the forced-cap independent MAP fixture. The memory fixture
explicitly allows an unchanged peak when preconditioner construction dominates;
its earlier overly strict assertion remains recorded.

In the six-call `g21owmem1` constrained diagnostic, the unit ring K50/T1 saved
fit succeeds at 1 GB usable with 523,613,047 tracked allocation bytes and
independent inference/output checks. The previous `in1` artifact rejects at
that budget. Both weighted 1 GB calls and both new-artifact 300 MB calls
reject with r(909). A visible-error follow-up confirms the existing joint
automatic preflight rejects the weighted 1 GB request; both artifacts time
out at 1.5 GB usable. This scoped policy does not resolve that weighted
constrained case. Diagnostic timings are excluded from performance comparisons.

The [scoped paired screen](../../benchmarks/reports/development/scoped-ols-window-local-2026-09-21/README.md)
completes thirteen cells and 78 calls with clean isolation. All 39 native pairs
preserve exact numerical exports, samples and saved vectors. The exercised
unit ring K50/T1/all fit saves 121,868,720 tracked bytes with paired runtime
ratio 0.9948; other peaks are unchanged. All paired runtime ratios are within
0.9908–1.0171. Independent inference, repeated outputs and focused memory,
transport and MAP checks pass. Final-block and Linux acceptance remain open.

The 2026-09-22 closeout in [ADR 0101](0101-ols-optimization-round-closeout.md)
commits this scoped implementation with the existing graph improvements as a
local development checkpoint. Historical final Mac blocks cover their captured
`owf1` artifact; they do not certify the later combined source. Linux acceptance
remains deferred. No new transport guard or memory allowance is introduced by
the closeout.
