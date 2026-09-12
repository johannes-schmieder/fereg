# ADR 0040: Stable PPML reference-pass scheduling and finite scratch reuse

**Status:** Implemented for user-authorized local development
**Date:** 2026-09-12

For retained fast PPML sessions with one categorical intercept effect, at least
65,536 active rows, eight stored generated columns and more than one installed
native worker, assign current-weight reference-statistic columns to the existing
team. Each column preserves the original row traversal, scaled sum-of-squares
norm and compensated weighted-mean arithmetic. Compute the compensated weight
denominator once in original row order. Smaller/narrower, single-thread, low,
generic/slope and multiple-FE reference refreshes keep their existing traversal.

Both schedules evaluate every raw value under the current weight generation.
The column schedule repeats weight normalization/root arithmetic per column;
its benefit is locality and concurrency. It adds no observation-sized buffer,
per-worker group bank or worker pool. Workers access only native data. It does
not skip a mathematical pass, reuse stale norms, or subtract raw cross-products.
Original reference scales and ADR 0020's rank/QR, generation, independent
certificate, final-Hessian and strict-transaction rules remain unchanged.

Frozen finite solves also reuse `candidate_positions` for QR identity scratch.
The caller temporarily owns `kept`; allocating into its emptied slot created
one disposable allocation per solve. QR scratch and inverse-certificate
positions have disjoint lifetimes and already share sufficient retained capacity.
The existing finite solver, fallback and capacity/admission rules are unchanged.

The [source-bound report](../../benchmarks/reports/development/ppml-reference-passes-local-2026-09-12/README.md)
isolates both changes. Scheduling improves three public primary command medians
by 10.7–14.8% in paired local measurements, with exact scientific exports and
unchanged primary native peaks. A larger full-dummy Poisson/Hessian/sandwich
oracle independently checks the parallel boundary. Graph, weight, output and
memory controls and the initial low-memory timing flag are retained.

The scratch fix changes real 16-iteration one-FE audit windows from 16 events to
zero in fast PW and low-memory FW, while the weak-graph profile changes 21 to
five. The latter still fails its whole-iteration allocation-free certificate;
its residual events are not accepted as an allocation-free route. This is a
local development decision, not a new solver region or qualification claim.
