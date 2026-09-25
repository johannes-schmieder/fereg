# ADR 0208: current-weight finite-block conditioning for joint PPML LSMR

Status: accepted for local development, 2026-09-25; qualification remains separate.

## Decision

Replace the ordinary joint LSMR finite diagonal with a full standardized
current-weight Gram factor when that factor is numerically usable. Preserve
joint LSMR, the FE core-pair/Schwarz policies, automatic routing, original-input
certificates and canonical rank/final-Hessian authority. This is a finite
preconditioner change, not the rejected coefficient-engine switch in ADR 0207.

For original active controls X and current Hessian weights W, set H = X' W X,
S = diag(H)^(-1/2), and factor C = S H S = L L'. The finite preconditioner
applies S L'^(-1) L^(-1) S using forward/backward substitution. Its forward and
adjoint apply are the same symmetric positive-definite map. Canonical kept
columns remain unchanged; this factor has no rank, bread or inference authority.
The FE block is unchanged. No special benchmark, seed, term-name or thread guard
selects the factor.

## Lifetime, numerical refusal and memory

The fit-owned active design and FE topology already have immutable identities.
A single K-by-K double bank is fallibly allocated with the admitted joint owner.
Its checked size is included in the complete joint-LSMR memory forecast. Actual
allocation failure retains the existing fatal allocation semantics; it does not
silently change the admitted solver. K=0/1 and absent diagnostic factor storage
retain the diagonal operation.

Every solve refreshes the existing diagonal and then the full factor from its
current weights. Disjoint Gram rows run on the installed team, retaining each
entry's observation order; no extra N-by-K bank or worker reduction bank is
introduced. All recurrence attempts within the call borrow the same factor.
No weight-frame authority is reused across calls, strict resets or basis changes.

Nonfinite geometry or a standardized Cholesky pivot at or below 32*epsilon
numerically declines to the existing finite diagonal. This is a preconditioner
safeguard, not the canonical rank threshold. It cannot drop a control. The fresh
original-input FE and finite normal-equation certificates still govern every
successful direction, together with recurrence convergence and existing bounded
continuations. Outer score/objective/model-space/reconstruction checks, samples,
weights, DoF, inference, separation and transactional publication are unchanged.

## Evidence and tradeoff

The [source-bound report](../../benchmarks/reports/development/ppml-finite-block-local-2026-09-25/README.md)
binds finiteblock1 to fb785e45 plus the single categorical_lsmr.rs overlay.
Identical frozen development frames show that full finite conditioning reduces
later hard-factor iteration counts from 31/30 to 14/13 and call medians from
126/110 to 56/52 ms. Numeric frames retain their iteration counts and pay for
additional Gram preparation. This measured tradeoff also appears publicly.

The original hard three-FE factor public median improves 850 -> 726 ms, against
732 ms comparable pinned stock. Frozen fresh three-FE factors improve
1067 -> 867 ms. Fresh two-FE factors improve 696 -> 432 ms versus 535 ms stock;
all five paired candidate/stock ratios are 0.790–0.814. These gains justify the
small direct implementation and K-squared memory, despite observed 2–4% numeric
and wide-control median regressions. The hard fast three-FE case is near parity,
not 10% ahead; low and large-N numeric gaps remain open.

33 distinct Rust tests, 102 frozen-frame certified solves, five focused licensed
fixtures and both candidate explicit-design Poisson/Hessian/sandwich checks pass.
All 264 public-state checks and 86 pilot/fresh baseline/candidate pairs pass.
Small baseline coefficient failures and eight sparse stock comparisons remain
unranked. All 12 OLS export pairs are exact; 1–2 ms timing losses remain visible.
No tolerance was changed to obtain a win. This is not platform or full-suite
qualification, and it does not repair deferred allocation/constrained-fit gates.

## Remaining design boundary

Full finite conditioning carries over to two FE without replacing the FE engine.
The next coherent candidate is canonical FE-equivalent controls in the joint
operator, with original controls retained for independent certification and
explicit handling of basis/representative changes and warm starts. It must not
inherit the rejected hybrid's fused canonical dispatch bug. Test identical
frames before public admission; do not tune on fresh seed202609439. If that
representation fails to repay its costs, move to canonical weighted-frame
construction and the large-N pass budget instead of adding fitted routing guards.
