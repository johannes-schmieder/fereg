# ADR 0107: narrow PPML Schur checkpoints

**Status:** Accepted for local development
**Date:** 2026-09-22

Extend ADRs 0067 and 0106 to 2–15 actual RHSs on an already admitted two-FE
categorical checkpoint-LSMR session with retained Schur storage and at least
16,384 observations. This fills the gap between the accepted single-residual
and wide-batch routes. Ordinary engine selection, OLS routing, slope sessions,
one-/three-FE sessions and low-memory owners without this storage are unchanged.

Pass selected columns explicitly to the shared probe. A PPML batch of up to
three RHSs probes every available column; in particular, width three includes
its third RHS rather than probing its second twice. Larger PPML batches and
the separate OLS entry retain their established selection and arithmetic.
Reject invalid selections before mutating workspace state.

Immutable topology and admitted buffers belong to the existing session.
Current group weights, RHS statistics and recurrence state are rebuilt on
every call. No weighted geometry, residual, score or Hessian certificate is
reused across frames. The fresh initial certificate, eight-step probe,
contraction threshold 0.01, bounded attempt of at most 64 iterations and fresh
canonical residual certificate remain mandatory. Probe or numerical rejection
retains untouched-input LSMR, its tighter retry and MAP fallback; allocation,
interruption and invalid-state errors remain fatal.

One transient candidate uses the existing checked copy allowance in place of
larger admitted LSMR banks and is dropped before fallback. There is no new
matrix bank, persistent cache, memory mode or relaxed budget. The allocation
inventory's documented source-drift failure remains open.

The [source-bound report](../../benchmarks/reports/development/ppml-narrow-batch-local-2026-09-22/README.md)
records roughly 10–34% paired gains across selected dense, weak, disconnected
and unbalanced PPML cases, 37% on a serial control and 19% on a PW/cluster
control. Sparse confirmation loses about 1.4%; excluded controls are mostly
flat. Most stock-comparable rows still lose to fixest. All 129 headline calls,
51 native scientific pairs and 27 direct candidate/stock checks pass. Validation
includes 391 distinct Rust tests, ABI containment and four licensed fixtures.
The new explicit-LSMR fixture proves actual narrow admissions under changing
weights and checks dummy-Poisson coefficients, fitted means and independently
formed current-Hessian sandwiches for unit/FW/PW, T1/T4 and multiple widths.

Accept and close this bounded experiment. Do not tune admission to favorable
seeds or suppress the sparse probe cost. A near-null third RHS can reject a
tiny batch; the existing certified fallback is preferable to a new heuristic
without a separate broadly useful case. This is local development evidence,
not a universal speed, constrained-memory or qualification claim.
