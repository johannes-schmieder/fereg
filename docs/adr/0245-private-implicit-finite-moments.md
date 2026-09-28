# ADR 0245: Private implicit finite moments and narrow dense partitions

Date: 2026-09-28

## Decision

Extend ADR0243's private finite-moment ownership to retained implicit designs
with at most 64 active columns, and scaled retained dense layouts assigning
one left column per partition. Use one statically dispatched helper to own the
bounded row, RHS and Gram arrays, then copy completed rows once. The helper's
stack frame stays outside wide/scratch calls. Existing eligible dense calls use
the same owner; there is no second retained bank or alternative solver layer.

The implicit caller binds the existing original columns and current-weight
centers. Its private row scratch also avoids repeatedly writing shared cache
lines. The old admitted row bank remains available for wide/other workspace uses.
Wide, unscaled dense and scratch loops retain their previous paths. The earlier
multiple-left-column guard served reuse of standardization; private accumulation
also helps one-left-column partitions. No benchmark name or seed selects a route.

Each moment still visits observations in order, with the same normalization,
selected values, division and FMA expressions. Private storage has current-call
lifetime only. No geometry, weight generation, prior solution or cache acquires
new scientific authority. Projection, model-space, rank, finite solve, score,
objective, reconstruction and final-Hessian acceptance remain independent and
unchanged. Errors publish no partial result.

## Evidence and tradeoffs

The [source-bound report](../../benchmarks/reports/development/ppml-private-low-scc-2026-09-28/README.md)
binds c09eeab3 (259 inputs), Mac/Linux artifacts, independent scalar moments,
weighted Poisson/Hessian/sandwich and OLS oracles. Both platforms pass 104 focused
Rust tests and six licensed fixtures; Linux also passes PPML smoke. The discovered
macOS runtime-allocation failure is repaired separately under ADR0244, with a
cold-construction regression test and twenty unchanged public allocation audits.
No runtime event is exempted and no tolerance is loosened.

Initial, fresh and scaling blocks complete 249 applications. All 83 native and
83 stock scientific comparisons pass. All 83 native coefficient/covariance export
pairs, including differing thread counts, are byte-identical. All 65 same-thread
iteration and selected-memory comparisons agree.

Fresh low-mode whole-command medians improve 14–35%; narrow fast T8 improves 25%.
Finite solving improves from 63.3 to 29.3ms at low T4 and from 359.5 to 124.4ms
in the larger low T8 case. All 18 direct scaling pairs improve, with actual
1→2→4→8 worker transitions included. The old small-core slowdown is not universal
and the tested transitions now pass; no new ceiling is introduced.

Costs remain visible. Fresh fast T4/wide/two-FE controls are 3.9%/3.1%/4.8% slower;
the two-FE loss follows an initial 0.8% loss. Larger fast is 0.4% slower fresh.
Serial low has one 4.6% losing pair. Retain the coherent ownership change for its
substantial repeated gains, while keeping those control losses as remaining work.
This is not universal non-regression or satisfaction of the 1.20/.90 stock goals.
Do not fit guards to these validation inputs or begin another long micro-tuning
cycle for the residual control losses.

Private arrays total 33,792 bytes (33 KiB) per active partition, at most
2,162,688 bytes over 64 eligible simultaneous partitions. Existing eligible
dense partitions already carried that payload. The newly eligible implicit
and one-left-column paths add transient stack storage; no heap/N-sized bank or
thread is added. The shared `ThreadTeam` forwards a configured Rayon stack size
and otherwise uses Rust's runtime default. ADR0243's specific 8 MiB statement
must not be treated as a general shared-team guarantee; CMG has a separate
explicit stack allowance. Full stack/physical-cap accounting remains open.

PPML heap peaks agree in 55 of 60 same-thread pairs. Three-FE peaks vary in both
directions, up to +6.14MB/6.0% fresh; changed overlap of existing independent work
is plausible, not established. Fresh RSS median ratios span .977–1.028.
Stronger physical memory claims, source-allocation-inventory qualification and
other unresolved numerical/comparator issues are not repaired by this change.
