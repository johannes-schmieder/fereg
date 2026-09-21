# ADR 0091: bound OLS scratch-score read-ahead after row jumps

**Status:** Accepted for local development
**Date:** 2026-09-21
**Decision owner:** Johannes Schmieder

## Decision

In ordinary OLS scratch-score storage, a cache miss outside the continuation
of the prior block reads only the requested row. An empty cache or a miss at
the prior block's end uses the existing bounded read-ahead capacity. This
avoids repeatedly decoding large unused tiles during cluster-ordered access
while retaining sequential reads for score certification and robust inference.

Keep the same cache allocation, file format, row values, finite checks,
arithmetic/reduction order, weight semantics, rank and DoF, independent
certificates and transactional publication. Dense/strided score storage and
retained PPML adapters are outside this change. No solver, memory-admission,
convergence or tolerance policy changes.

## Evidence boundary

The [local source-bound report](../../benchmarks/reports/development/low-score-cache-local-2026-09-21/README.md)
records 27 completed calls in a three-cell, three-pair screen. Weighted cycle
K6/K50 clustered full-output medians improve from 23.121/37.150 seconds to
0.753/9.261; default fixest takes 3.268/12.009. A sequential dense robust control
has a 0.988 paired native ratio and retains its separate reference gap.

All nine native pairs have bit-identical numerical exports and matching
allocation peaks. Six independent native inference checks, six output audits
and eighteen exact repeat checks pass. The two default-reference cycle oracle
failures remain recorded. A rank-1/6/50 bit-preservation/read-volume fixture and
both low-memory clustered allocation-failure fixtures pass.

This accepts the scratch-read change for local development only. It does not
accept the report's other numerical drafts, complete the broader optimization
milestone or claim Linux/Windows qualification. The rejected low-final-product
retention direction remains preserved separately in the same report.
