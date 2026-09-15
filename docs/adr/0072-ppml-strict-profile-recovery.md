# ADR 0072: preserve strict PPML refinement and bound profile recovery

**Status:** Implemented for user-authorized local development
**Date:** 2026-09-15

An ordinary tolerance schedule must preserve a tighter requirement established
by a rejected strict checkpoint. The nominal scheduling floor applies to newly
chosen targets; it must not loosen the previous target. Persist checkpoint
refinement into ordinary iterations. Strict profile, objective, score, rank,
reconstruction and final-Hessian acceptance tolerances remain unchanged.

If three strict checkpoints on the same basis at the actual projection floor
fail profile consistency without reducing the discrepancy by half, perform one
canonical rebuild. Strict checkpoints already require two accepted objective
and score passes. Preserve progress and reset the stagnation window when the
basis changes. Following the rebuild, allow the rebuilt checkpoint and at most
two further rejected strict corrections; persistent failure returns r(430)
without publishing a result.

The rebuild rematerializes untouched generated inputs into the existing dense
or scratch design. Low mode uses the admitted projection block and the same
scratch artifact; fast categorical response refresh resets the existing response
banks to the current authoritative response. The caller polls interruption
between bounded numeric tiles; workers use Rust data only. Reprojection and all
canonical certificates still authorize the candidate. Reference norms, basis
identities, weights and generation ownership remain unchanged. Count recovery
in the existing warm-projection fallback and checkpoint retry diagnostics.

Private solver tracing adds bounded worst-row/profile diagnostics. It is
observational and cannot authorize publication. This extends ADRs 0018, 0020
and 0027 without changing the public model, sample, solver-family or scientific
acceptance policy.

The [source-bound local report](../../benchmarks/reports/development/german-ppml-checkpoint-recovery-local-2026-09-15/README.md)
records the red scheduler regression, an analytical Poisson reproducer, bounded
recovery and interruption tests, fresh-session and storage parity, licensed
controls, command costs and remaining full-data validation.
