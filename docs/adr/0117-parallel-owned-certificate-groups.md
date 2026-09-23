# ADR 0117: Parallel fresh owned-FE certificate groups

Status: accepted for local development, 2026-09-23.

## Decision

Use the existing worker team to evaluate independent effect-level groups in
automatic owned slope certificates when the current call has admitted QR
geometry, at least 16,384 observations and more than one installed worker.
No pool is created. Outside an installed team, serial calls, smaller inputs,
direct solvers and uncached contexts retain serial certification. These are
structural scope and scheduling conditions, not benchmark/seed guards.

Each group computes fresh weighted span coefficients and raw moments from the
current residual, using the same coordinate order and arithmetic. Workers
write disjoint result slots. Merge all group/RHS values in the original
ordered scalar loop, including the RMS FMA recurrence, finite checks and raw
maxima. Never replace that loop with a parallel reduction or sum each group
before merging: either would change floating-point grouping and could change
automatic contraction/routing. Residual norms, geometry rank, tolerances,
check frequency, solver state and publication remain unchanged.

Admit `8*max_effect_levels*(actual_RHS+1)` bytes beside the existing geometry
and solver envelope. One effect's result bank is reused within each fresh
certificate and released before return. Allocation/size failure uses the
already admitted serial check. If CG can retain geometry but cannot also
accommodate parallel scratch, disable parallelism without discarding that
geometry. Keep the maximum of earlier admitted attempts. LSMR still releases
the entire context before its existing admission. No optional optimization
can displace an affordable baseline route.

Workers read only immutable Rust-owned state. The current-weight context does
not cross absorption calls or PPML weight/sample generations. The separate
changing-weight reusable MAP implementation and existing specialized slope
cache keep their contracts. Existing runtime/stack headroom covers worker
execution; raw diagnostics use the existing bounded 768-byte tile per worker,
with no new team or observation-sized worker-private buffers.

## Evidence and follow-up

The [local report](../../benchmarks/reports/development/parallel-certificate-local-2026-09-23/README.md)
records 16.4–26.9% paired median gains on seven threaded slope cases, including
unbalanced three FE, low memory, weak native, analytic and probability weights.
Additional tracked heap is 0.064–1.065 MB. All 39 native pairs retain exact
scientific exports and non-clock solver fields. Independent weighted two-column
projection checks, cross-thread bit checks, exact-budget fallback tests and
licensed fixtures pass. Direct MAP has a 1.9% paired median loss, and smaller
control losses remain in the distributions. Stock still leads all five directly
comparable cases; weak stock covariance remains outside the unchanged envelope.

Close this bounded scheduling change without threshold or tile tuning. Fresh
certification becomes much cheaper; unchanged projection sweeps now dominate
these profiles. Investigate admitted group coefficient work and row-parallel
subtraction there as a separate milestone, retaining raw Gram/pivot semantics,
same-weight ownership and ordered per-entry arithmetic.
