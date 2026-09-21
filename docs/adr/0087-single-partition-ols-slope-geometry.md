# ADR 0087: reuse single-partition OLS slope geometry

**Status:** Accepted for local development under the approved optimization plan
**Date:** 2026-09-21
**Decision owner:** Johannes Schmieder

## Decision

Eligible fast `solver(auto)` OLS fits prepare each group's weighted slope
geometry once and reuse it for the initial response/regressor batch and final
residual. Eligibility requires one validated ordinary partition with slopes,
at least 16,384 retained observations, at most four absorbed channels and 64
actual RHS columns, and room for a certified fallback within the iteration
budget. Explicit solvers, low-memory fits, shared projection/PPML callers and
other topologies retain their existing absorption paths.

The projection keeps the existing raw Gram elimination and its rank cutoff.
The independent certificate keeps normalized, pivoted, twice-reorthogonalized
QR directions and the existing rank threshold. Only fixed-weight geometry is
cached: every returned residual gets a fresh norm and span check. Independent
groups run on the existing worker team. Contiguous RHS loops retain each
entry's row and FMA order; the diagnostic RMS merges group partials in order.
The unchanged infinity-norm certificate determines acceptance.

The cache holds square-root weights, group Grams/scales and a flat QR basis.
Construction admits its scratch and projection admits a separate output plus
group coefficients, norms and check reductions. Fallible reservations and
actual capacities remain authoritative. A rejected attempt releases the cache,
returns the untouched input to the established solver and charges discarded
time, peak and iterations. There is no cross-fit or changing-weight reuse.

Unit-weight fits may reuse same-input raw norms and return freshly certified
residual norms to OLS. Supplied-weight fits retain OLS's normalized-weight norm
scan: the generic slope projector uses raw weights, and those scales must not
be mixed. Saved-output fits may use the existing independently certified final
target recovery instead of requesting unused initial coefficient products.

Owned preparation classifies the original complete sample before removing
synthetic categorical encodings of continuous slope sources. Actual partition
sources, cluster roles, missingness and singleton decisions remain intact.
The existing bounded numeric loader may prepare RHS data for one ordinary
intercept-and-slope partition; its main-thread SPI boundary, recipe validation,
sample matching and memory admission are unchanged.

## Evidence boundary

The [local checkpoint](../../benchmarks/reports/development/slopes-cache-local-2026-09-21/README.md)
records exact source/artifact identities, candidate screens, independent slope
and frequency-expansion oracles, weighted saved-output audits, focused core/SPI
tests and four licensed fixtures, including protected PPML saved slopes.
It preserves a rejected experimental raw/normalized norm handoff and the test
that identified it. Narrow serial fits and saved-effect recovery remain open
performance targets. Broader graph, routing, constrained-memory and Linux work
remain part of the active milestone; this is not release qualification.
