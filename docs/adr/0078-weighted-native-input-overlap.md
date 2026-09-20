# ADR 0078: weighted native input overlap

**Status:** Accepted for local development
**Date:** 2026-09-20
**Decision owner:** Johannes Schmieder

## Decision

Extend the existing ordinary-intercept OLS numeric input team to supplied
analytic, frequency and probability weights. Count the weight column in the
bounded numeric width, transport it in the same main-thread SPI tiles, and
permit the existing speculative raw RHS construction. Deferred source and
weight vectors acquire their initialized lengths through the worker's checked
append path; rejected optional admission restores all initialized columns
before the existing reader runs.

The existing candidate, row, thread, width, numeric-source and memory guards
remain. Worker threads receive only Rust-owned data. No weight is normalized
or interpreted during transport. Authoritative preparation still determines
weight domains, candidate exclusion, missingness, singleton pruning, normalized
weights and the final sample. Exact prepared row, recipe and value matching
is required before consuming the speculative RHS; changes release it.

Raw unit-weight moment overlap remains restricted to absent supplied weights.
Weighted moments continue to use the final prepared weight frame. Inference,
output recovery, PPML, slope-bearing and no-FE entry points retain their
existing contracts. No new owner, allocation allowance or scientific kernel
is introduced.

## Evidence and limits

The [local report](../../benchmarks/reports/development/two-fe-weighted-input-local-2026-09-20/README.md)
records the corrected public-path artifact, an inactive preliminary prototype,
all 110 application calls and a separate low-memory confirmation. Three-pair
AW K=50 IID medians fall from 937 to 851 ms; full outputs fall from 1112 to
1033 ms. The separate profile reduces input from 283 to 226 ms and RHS setup
from 30 to 16 ms. Narrow gains are smaller. The shuffled AW K=6 case and
strong-d3 AW K=6 case still lose to stock fixest.

All 34 native result pairs and 15 saved-output pairs are exact; none raises
the native allocation peak. Fifteen independent output audits, three Stata
fixtures, the actual-plugin ABI check, six pipeline tests and the 27-case
weighted transport/preparation/inference test pass. A single low-memory run
slowed by 6.6%; the separate three-pair confirmation improves by 3.2% in median
paired time. Both remain in the archive. This is a local development change,
not completion of the broad two-way milestone or platform qualification.
