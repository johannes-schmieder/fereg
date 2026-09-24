# ADR 0156: component-aware PPML warm checkpoint acceptance

Status: accepted for local development, 2026-09-24.

## Decision

Apply ADR 0143's component scaling to the zero-iteration shortcut as well as
its checkpoint recurrence. This affects only a categorical projector already
configured for component-scaled LSMR checkpoints. Production admission remains
low-storage, three categorical effects and finite controls. No fast categorical
route, ordinary solver, public option or OLS policy is added.

Compute the existing temporary component-scaled weights before the early return.
When the original-weight initial certificate passes and there are disconnected
components, also check the unchanged residual under component-scaled weights,
using its freshly computed norms and the unchanged tolerance. Restore original-
weight reference/residual norms before either returning or continuing. Return
without iteration only when both checks pass; otherwise use the existing scaled
checkpoint solve, fresh original-weight certificate, bounded retry and fallback.
Connected designs and projectors without this option retain their original
acceptance rule. No speculative residual changes during the additional check.

Scaling each independent FE block by a positive constant preserves its weighted
projection. This fact applies to absorption; it does **not** authorize rescaling
the full finite/FE regression objective independently by component. Finite scores,
rank, inverse/bread, likelihood, inference, DoF and final Hessian remain governed
by original weights. All current tolerances and publication requirements remain.

Reuse the already admitted component weights and existing certificate scratch.
There is no additional bank or admission formula. Component preparation now runs
before an otherwise successful initial check, and a disconnected initial pass
requires an extra scaled certificate and norm restoration. These costs are
necessary to prevent the shortcut from bypassing the recurrence's accuracy aid;
this decision is a correctness prerequisite, not a claimed speed improvement.
Allocation errors remain fatal and public publication remains transactional.

## Diagnosed failure

ADR 0155's rejected fast generic joint integration did not lose its final accuracy
in the last ordinary coefficient solve. Its fifth ordinary predictor had roughly
9e-14 cold-component error. A canonical strict warm projection then accepted
without iteration under a global reference norm and replaced that state with
1.2343253e-9 error. The high-mass component hid the smaller component's drift.
Component scaling was enabled but occurred after this early return.

A 60-digit independent grounded explicit-design WLS calculation confirms the
captured-frame error. Forcing the same checkpoint through scaled solving removes
it. The adopted two-check shortcut likewise passes the unchanged extreme truth
envelope in an isolated reproduction of the rejected source; its final cold error
is about 9e-14. An independent paired-contrast regression test fails on the parent
and passes here, covering two/three effects, changing component masses, zero RHS,
T1/T4, small drift and exact zero-iteration inputs.

The baseline's explicit LSMR configuration separately exhausts the six-iteration
bound on this extreme fixture. Baseline MAP passes. This decision does not repair
or relabel that LSMR result. The earlier generic fast integration remains rejected
for its whole-command serial losses even after its recorded truth failure is
explained; no routing or wrapper changes from it are reinstated.

## Evidence and next design

The [source-bound report](../../benchmarks/reports/development/ppml-component-warm-boundary-local-2026-09-24/README.md)
contains captured states, independent high-precision WLS results, red/green tests,
licensed oracles, failures, source identities and matched cost controls.

The next solver experiment should compute coefficient-space Newton increments
without the generic absolute-WLS reporting wrapper. Preserve the canonical
response history and use original current-frame controls for independent scores.
This finding removes a specific scientific obstacle; it does not establish a
performance case for the rejected integration or complete the broad PPML target.
