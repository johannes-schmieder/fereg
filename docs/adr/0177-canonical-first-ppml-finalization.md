# ADR 0177: canonical-first PPML finalization

Status: accepted for local development, 2026-09-24. Not qualification.

## Decision

For publication PPML with at least two absorbed partitions, first try one strict
canonical Newton checkpoint after the existing two accepted objective/score
convergence passes. Use the existing current-weight absorption, canonical rank,
finite regression, inverse, reconstruction and strict publication comparisons.
If this independently certified checkpoint confirms the accepted point and basis,
retain the accepted predictor and finite coefficients. Its canonical products then
have exactly the accepted q*mu weights. Independently validate product authority,
weight bits and scale, frame generation, every reconstructed predictor row and
q*mu before allowing those products to supply final inference. Recompute the final
outer score. Refusal retains the full final-Hessian synchronization procedure.

This generalizes ADR 0174's already implemented exact-state product boundary;
conditional intercept optimization is not necessary for its mathematical validity.
Keep the ordinary directions and line search unchanged. Do not expand the two-
categorical-partition conditional trial update to slopes or additional partitions.
One-FE and separation exploration retain their existing behavior.

If the canonical strict checkpoint moves the point or basis, permanently revoke
this shortcut for that fit. Process the changed checkpoint through the existing
accept/backtrack/resume transaction, as required by ADR 0016. Subsequent eligible
strict attempts use the existing joint witness plus canonical predictor finalization
from ADR 0131 and its categorical successors. No extra recovery attempts or altered
publication tolerances are introduced. Existing rebuild, bounded refinement, rank
loss, interruption and rollback policies remain authoritative.

## Why the fallback remains necessary

An independent disconnected slope truth probe has component prior weights that
differ by 1e13. At requested outer and score tolerance 1e-12, always requiring the
canonical Newton reconstruction to confirm the point stagnates at approximately
1.07e-10 predictor movement, despite a very small aggregate score. The established
joint witness and canonical predictor finalization converge with truth error below
1e-10. Both certify the required current-weight mathematical conditions; their
reconstruction arithmetic has different conditioning. A fresh canonical Newton
witness cannot be assumed numerically superior on every frame.

The successor therefore does not remove a still-needed numerical fallback or fit a
guard to a weight threshold, seed, sample size or benchmark. The existing strict
comparison itself decides refusal. After refusal the changed strict state still
receives the mandated transaction and continuation. Tests exercise this path at
T1/T4, require the unchanged independent truth bound, and confirm exact final q*mu.

A broader conditional-intercept trial experiment initially exposed this boundary.
Revoking its trial update only at canonical rebuild did not fix it; that experiment
is not adopted. Its source, failed probes and the subsequent checkpoint-only
experiment remain distinct evidence. A default-tolerance low extreme-component
probe also misses a newly imposed 1e-10 truth bound in the unchanged baseline
(error 1.00696e-10); this decision does not claim to repair it.

## Memory and architecture

One fit-local Boolean records whether the shortcut retains eligibility. Reuse the
existing strict transaction, candidate buffers and product checks; add no heap
workspace or memory-plan credit. The complete prior solver/fallback allowance
remains admitted. Actual tracked peaks can change with the ordering and lifetime
of existing products; report those measurements separately from modeled peaks.
No physical-memory qualification or allocation-inventory repair is implied.

The one guarded plugin call, safe Rust computation, main-thread SPI and transactional
publication are unchanged. Samples, separation, weight semantics, rank policy,
DoF, covariance, scores and objective rules are unchanged. No public option is added.
The old finalization is retained because independent boundary evidence requires it,
not as an unmeasured overlapping accelerator.

## Evidence

The [source-bound development report](../../benchmarks/reports/development/ppml-canonical-first-finalization-local-2026-09-24/README.md) records the current-source opportunity screen,
isolated whole-command profile, failed experiments, independent oracles, application
and comparator failures, uncontended paired distributions, memory and excluded
controls. The historical baseline checkpoint-count assertion remains failing and
was reproduced in the isolated accepted artifact; its assertion is not relaxed.
A separate successor test verifies the mathematical dynamic-range result.
