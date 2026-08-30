# ADR 0014: certified VCE hot reductions

**Status:** Accepted
**Date:** 2026-08-29
**Decision owner:** Johannes Schmieder

## Context

After ADR 0013 removed the main complete-input and finite-solve bottlenecks,
robust and clustered inference became the dominant cost in wide, large-sample
fits.  The original prepared-inference path repeatedly transformed observation
scores through the bread matrix and reconstructed the same sandwich inputs in
separate production and certification traversals.  That preserved strong
fail-closed checking but multiplied the `N * K^2` work.

The first optimized implementation removed the redundant bread work and fused
fixed-block score-meat reductions.  Its broad SCC matrix was scientifically
clean, but two implementation rounds exposed distinct performance defects:

1. the first candidate still performed bread work inside the hot reduction;
2. the first repair wrote the independent certificate triangle in transposed
   order, turning each observation-level certificate update into a cache-hostile
   column stride.

Neither defect justified weakening the covariance certificate.  The required
repair was to preserve two independent reductions while keeping both hot write
streams contiguous.

## Decision

1. Dense robust and clustered VCE paths accumulate score meat directly in
   deterministic fixed row or group blocks.  Bread multiplication occurs after
   those hot reductions.
2. Production and certificate meats use separate reserved storage.  They must
   not alias, and allocation accounting includes both buffers.
3. Both observation-scale triangular writes use the contiguous row-major upper
   triangle.  Transposed certificate writes are forbidden in the hot loop.
4. Independence is preserved by distinct production/certificate scale
   construction, distinct storage, and a reverse fixed-block merge order for
   the certificate.  Production merges blocks forward.  Both merges retain
   compensated addition and deterministic partitioning.
5. Dense clustered aggregation applies the same rule to direct group slots and
   radix-ordered group subsets.  Multiway inclusion--exclusion, cluster counts,
   finite-sample corrections, and public DoF semantics do not change.
6. Scratch-backed and non-dense paths retain their existing independent
   reconstructions.  The optimization is not permission to reduce general-path
   coverage.
7. The independently reconstructed meat and compensated sandwich remain the
   publication authority.  Existing coefficient, residual, score, bread,
   covariance, symmetry, and standard-error tolerances are unchanged; any
   disagreement fails closed.

## Qualification

Qualification used three exact-source SCC stages whose compact evidence is
captured under
`benchmarks/reports/captured/linux-x86_64/stata19/adr14-vce-qualified/`.

### Broad VCE surface

Source `9789f8463b8d2b8f635d228a3db9ad8b265eb1f4` covered 34 scopes:
IID, robust, and one- through five-way clustering; analytic and probability
weights for robust and one-way clustering; one and sixteen requested native
threads; and one-million/K=10 plus ten-million/K=30 designs.  Every benchmark
published a strict success shard with one passing scientific preflight and zero
command or science failures.  The merged population has 264 timing rows and 34
preflights and contains only fereg, Greg, and reghdfe.  Greg is absent only
where its public command does not support the requested multiway VCE.

The original benchmark wrappers returned nonzero only after publishing their
atomic success shards because a cleanup step tried to remove a work directory
that still contained the Stata log.  That operational defect and the original
accounting are preserved.  Validation array `7366246.1-34` and collector
`7366247` independently revalidated the published shards and completed with
SGE `failed=0` and `exit_status=0`; the costly benchmarks were not rerun.

### Bread-hoisting repair

Source `a8fbad2d26e67bcfd575cf8d44534d0d0df0d72f` retested 13 focused
boundaries.  Setup `7366384`, array `7366385.1-13`, and collector `7366387`
all completed with SGE `failed=0` and `exit_status=0`.  All 13 shards were
scientifically clean, producing 108 timing rows and 13 preflights.  Probability
weighted robust and one-way cluster fits at one million rows and sixteen
threads took 2.150 and 1.346 seconds; four- and five-way clustering took 2.818
and 2.254 seconds versus reghdfe's 35.047 and 44.833 seconds.

This source was rejected for production performance because its transposed
certificate writes made the K=30 hot loop cache hostile.  Ten-million-row
robust VCE took 456.748/397.109 seconds at one/sixteen threads, and one-way
clustering took 453.018/77.437 seconds.

### Contiguous certificate repair

Source `490e03a3e2458d8bc463c3f60389062e2f8fae42`, archive SHA-256
`4f4e44cc539419ae67043cfca824c92aed7c6b1969c1a37825939e2397d02aab`,
and Linux plugin SHA-256
`9cbe00b6c5a0d5c47af3d5d1fd32166a200d27202d335a602d584a3d5b48a4a2`
passed the exact eight-scope repair qualification.  Setup `7367526`, array
`7367527.1-8`, and collector `7367528` all have complete SGE accounting with
`failed=0` and `exit_status=0`.  Every shard has a strict success receipt,
exactly nine timing rows, one passing preflight, zero command/science failures,
the requested fereg thread count, and a clean Stata success marker.  The merged
receipt has exactly 72 timing rows and eight preflights.

Within these final shards, fereg beat reghdfe in all eight scopes and Greg in
all four sixteen-thread scopes.  The geometric-mean fereg ratio is 0.987
against Greg and 0.258 against reghdfe overall; at sixteen threads the ratios
are 0.620 and 0.216.  Relative to the rejected `a8fbad2` source, the
geometric-mean fereg time is 0.303.  The decisive ten-million-row medians are:

| VCE | Threads | fereg | Greg | reghdfe | f/Greg | f/reghdfe |
|---|---:|---:|---:|---:|---:|---:|
| robust | 1 | 161.455 | 78.262 | 442.199 | 2.063 | 0.365 |
| robust | 16 | 87.216 | 104.651 | 369.233 | 0.833 | 0.236 |
| cluster(firm) | 1 | 131.854 | 65.545 | 350.998 | 2.012 | 0.376 |
| cluster(firm) | 16 | 60.488 | 65.959 | 205.534 | 0.917 | 0.294 |

Cross-run absolute times are diagnostic because nodes differed.  The final run
used 32-processor nodes throughout; the earlier runs used 16- and 32-processor
nodes.  Acceptance therefore rests primarily on same-shard comparator ratios,
scientific receipts, and the large within-source repair effect, not on small
cross-run differences.  In particular, the final robust one-thread time is
9.7 percent above the first broad run's 147.125 seconds, which is not treated
as a regression claim across unmatched nodes.

## Consequences

- Certified robust and clustered inference no longer pays cache-hostile
  transposed certificate writes or repeated observation-scale bread work.
- The sixteen-thread wide/large VCE boundary now beats both same-shard
  comparators in every focused scope.
- Serial VCE remains about 19--23 percent slower than Greg at one million rows
  and about two times Greg at ten million rows, while remaining 2.7--4.2 times
  faster than reghdfe.  This is a candid remaining optimization target, not a
  release blocker for the accepted VCE implementation.
- Multiway and weighted scientific coverage comes from the broad exact-source
  matrix; the two narrow repair stages requalify the changed hot reductions.
- macOS remains a prototype and focused test lane.  SCC receipts are the
  authoritative performance and qualification evidence.
