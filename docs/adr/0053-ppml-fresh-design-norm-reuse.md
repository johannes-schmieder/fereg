# ADR 0053: Fresh retained PPML design norm reuse

**Status:** Implemented for user-authorized local development
**Date:** 2026-09-13

Extend ADR 0052's same-call certified norm transfer to fresh fast PPML retained
design construction. Select RHS norms using the existing stored generated-column
mapping, including gaps and the actual intercept position. Validate selection
length and bounds before projection or caller-output mutation. The fused
trailing-column call shares this helper without allocating an index buffer.

Reuse requires one categorical intercept FE and exact agreement between the
frame's solver divisor and the projector's maximum raw weight. Multi-FE handoffs
and alternative divisors retain the original stable frame-specific norm scan.
Generic/slope and low-memory paths remain unchanged. Failed projection never
publishes selected norms, and no last-result cache accessor is exposed.

Fresh construction keeps its in-place matrix compaction but omits duplicate
per-value within-norm arithmetic and two K-element scratch allocations when
reuse succeeds. It retains raw reference norms/means, the initial warm/raw
certificate, final certificates and strict rank/QR semantics. The rejected
fresh-input certificate shortcut remains absent. No observation-sized state,
automatic solver region or public interface is added.

The [source-bound report](../../benchmarks/reports/development/ppml-fresh-norms-local-2026-09-13/README.md)
records 15 pairs. FW and PW primary median paired command gains are 2.4% and
5.1%; complete unweighted command time is flat despite lower projection cost.
Native peaks remain exact and admitted, sampled RSS moves both ways and the
private audit retains zero allocation events/live/peak growth. Twenty-nine
focused tests and both independent Poisson/sandwich oracle arms pass. These are
local development findings, not qualification or a claim that every unchanged
phase's timing difference is caused by norm reuse.
