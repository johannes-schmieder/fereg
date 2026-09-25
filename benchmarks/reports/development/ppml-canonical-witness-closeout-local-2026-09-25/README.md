# Early canonical convergence witness: rejected accuracy experiment

Local Mac arm64 development evidence, 2026-09-25. No production optimization,
headline timing, qualification or release. Decision: [ADR 0191](../../../../docs/adr/0191-early-canonical-witness-closeout.md).

## Exact source and experiment

Baseline: `b8f04a232ea693d8ac90542c1dc72f00aa93e473`. The candidate is that commit
plus [rejected-implementation.patch](rejected-implementation.patch), SHA-256
`81b79eb201e3f450768fcd0fa3248a60db0503c9a7a1eab0b07d0a82312663c9`.
Only `ols/linear_session.rs` and `ppml/prepared_irls.rs` differ. The
[identity receipt](identity.json) binds those inputs, the two test binaries and
the raw/sanitized test protocols. The original work root is
`target/ppml-canonical-witness-20260925`.

After the first ordinary objective/score convergence pass at point B, the
candidate computes a strict canonical Newton point C. It checks C's fresh
objective and score, the registered full-step and relative-deviance rules, and
the existing B/C strict comparisons. If these and final-Hessian checks pass,
it counts C as the second witness while retaining B for publication. This
retention leaves the canonical products at the published point's exact weights,
but advances the existing accepted-point policy by one ordinary iteration.

The source also contains test-only enable/refusal counters for the unfinished
prototype. No new tests or relaxed assertions were added before its initial
existing-test screen failed. The patch is evidence, not a supported implementation.

## Independent accuracy rejection

Both runs use `cargo test -p fereg-core prepared_irls --lib`, with distinct Cargo
target directories. The candidate run finished before baseline restoration and
recompilation. The baseline reproduces the known count assertion failure; it
passes all four additional tests that fail with the candidate.

| Existing test suffix | Baseline | Candidate |
|---|---|---|
| `canonical_final_preserves_slope_truth_current_hessian_and_reuse_refusal` | Pass | Fails predictor truth `<1e-10` |
| `explicit_three_fe_solver_roles_are_permutation_invariant_and_certified` | Pass | Fails predictor truth `<=2e-9` |
| `low_categorical_joint_three_fe_preserves_truth_nested_rank_and_zero_control` | Pass | Fails predictor truth `<1e-10` |
| `canonical_final_preserves_accuracy_on_a_small_dynamic_range_model` | Pass | Fails with predictor error `3.13320702716168853e-10` |
| `refined_checkpoint_preserves_accuracy_on_a_small_dynamic_range_model` | Known failure | Same count assertion: expected one strict rejection, observed zero |

Totals: **baseline 53 passes / one failure; candidate 49 passes / five failures**.
Both select 54 tests and filter out 1,046. Full sanitized protocols are
[baseline](rust-irls-baseline.txt) and [candidate](rust-irls-candidate.txt).
The independent mathematical tests establish rejection; comparator agreement is
not used as an accuracy substitute. The baseline failure is not repaired or hidden.

Source inspection localizes the unsafe assumption: C's fresh score and a
permitted B/C difference do not imply that publishing B meets the established
truth envelopes after only one ordinary convergence pass. The tests demonstrate
that the proposed schedule is insufficient; this is not a proof that every
earlier strict checkpoint, or a design publishing C with a fresh C-weight
Hessian, must fail.

## Closeout and limits

Preserve the patch and protocols; remove both production-file edits. The
computational source is again exactly the baseline. No Stata or stock fit,
performance ranking, allocation audit, physical-memory test or fresh validation
seed was used for this candidate. There is no measured speedup or memory claim.

The latest accepted public timings remain those of
[resgate1](../ppml-residual-gate-local-2026-09-25/README.md); this closeout does
not transfer their claims to a new algorithm. Seeds 202609428/429 remain reserved.
Current implementation priorities belong only in STATUS.md.
