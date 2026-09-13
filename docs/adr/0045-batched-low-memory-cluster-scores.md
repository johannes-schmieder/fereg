# ADR 0045: bounded shared low-memory cluster score passes

Status: accepted for local development, 2026-09-12.

## Decision

Scratch-backed OLS clustered inference may feed all nonempty subset accumulators
from one original-row score traversal when their entire compensated group bank
fits within one displaced `sample_rows * sizeof(usize)` ordering allocation.
The budget includes values, compensation, presence flags, range metadata and
mixed-radix prefix scratch. It uses physical retained rows, not effective N.

Planning completes before bank reservation and score reads. If the complete bank
cannot fit, no partial bank remains and ADR 0044's per-subset table/sort route
executes. Reservations are fallible and capacity checked. The action-local owner
is released before final covariance bread/publication work; it never coexists
with a fallback ordering array or retains an N-by-K design.

Each subset retains original within-group row order and lexicographic group
completion. The common driver retains cardinality/mask merge order, independent
per-group bread arithmetic, weight-kind score semantics, corrections, DoF and
PSD policy. Dense/strided and PPML consumers do not build this bank. One-way
scratch tables share the extracted finishing helper without new eligibility.
Solver/rank/sample rules, SPI/thread boundaries and transactional publication
remain unchanged.

## Evidence and limits

The [report](../../benchmarks/reports/development/cluster-subset-batch-local-2026-09-12/README.md)
records 26 pairs against c37686b. The original two-/three-way targets missed their
10% command criterion (0.7%/5.9% paired gains). A separate prospective follow-up,
motivated by the five-way control, repeats gains of 20.4% and 17.4% on small-unit
and larger-AW five-way cases. Adoption rests on those repeated many-way results,
not a revised claim about the original targets. Primary native/scratch peaks
are unchanged; every scientific export and non-time solver decision is exact.

Thirteen focused Rust tests and both independent explicit-dummy/Mata covariance
oracles pass. The reviewed universal plugin is byte-identical to the measured
revised artifact and passes static/native arm64 ABI checks. Initial slower
prototype and invalid-oracle attempts remain recorded. Weak controls retain
pre-existing memory-model discrepancies and variable peaks; the bounded bank
does not resolve those P0 gaps. No release, SCC or cross-platform claim is made.
