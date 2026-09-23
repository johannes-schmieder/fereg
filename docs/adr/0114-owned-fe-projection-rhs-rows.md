# ADR 0114: Contiguous RHS arithmetic in owned-FE projections

Status: accepted for local development, 2026-09-22.

## Decision

Bind the generic owned-FE projection's observation row and corresponding
channel-moment row once, then update contiguous independent RHS entries.
Reconstruct fitted values in fixed tiles of 32 RHS entries, traversing each
channel's contiguous solved coefficients before a single residual subtraction.

Each moment retains its observation order and weighted-basis fused multiply-add.
Each fitted value starts at zero, retains channel FMA order and is subtracted
once. Raw Gram formation, partial-pivot elimination and its rank cutoff remain
unchanged. Input and workspace validation still precede projection. The old
scalar implementation remains under tests as an arithmetic-order reference.

A fixed 32-value tile adds 256 bytes of stack payload. There is no new heap
allocation, cached geometry, memory-admission rule, worker team or public API.
All operands are read from this call's effect, weights and values. No product
survives into another PPML weight frame, sample or generation. Fresh residual,
score, weighted-rank and final-Hessian certificates remain mandatory.

The kernel serves existing generic MAP/accelerator projections. It does not
remove the single-partition slope cache's guards or change categorical borrowed
solvers, LSMR, automatic routing, convergence, DoF, weights or publication.

## Evidence and alternatives

The [local report](../../benchmarks/reports/development/owned-fe-projection-local-2026-09-22/README.md)
records exact source/artifact identities, alternating public-command timings,
independent science, boundary controls and retained losses. Profiling shows
RHS moments and fitted-value subtraction dominate small group solves. This
justifies the smaller arithmetic change before considering a geometry cache
with additional storage and lifetime obligations.

The original cases gain 7–9% by paired medians; fresh wide and three-partition
cases gain 12.6% and 18.4%. Weighted controls gain 6–7%. A positive-DoF weak
native control gains 8.7%; disconnected and low-memory cases gain little or
nothing. Categorical/PPML and cached one-partition controls are effectively
flat, with individual losses. No tile-size search or case-dependent guard is
adopted. Stock still wins all six directly comparable timing rows.

All 39 native pairs and 52 repeat checks pass; all 39 pairs have exact
scientific exports and identical tracked heap peaks. Tests cover weighted and
unweighted inputs, widths 0–129, extreme scales, empty levels, dependent/zero
channels, and repeated workspace use. Independent pilots retain both a stock
weak-design precision discrepancy and the distinction between exact FE rank
and the registered conservative slope DoF count. Neither is hidden by changing
production policy or oracle tolerances. This is development, not qualification.
