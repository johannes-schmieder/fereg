# ADR 0013: certified complete-input OLS pipeline

**Status:** Accepted
**Date:** 2026-08-29
**Decision owner:** Johannes Schmieder

## Context

ADR 0012 removed the worst moderate-width finite-solve bottleneck, but the
one-million-row, one-FE, 30-control prototype still spent more time before the
finite solve than Greg spent on the complete command. A source-level audit of
the exact gtools 1.11.8 tree showed that Greg's useful advantage was not a
single superior FE algorithm. Its hot path minimizes boundary traffic and
walks observation-major data in cache order, while fereg still paid for
candidate-mask reads, one SPI traversal per numeric column, copies between
loaded and prepared storage, recipe resolution inside row loops, and repeated
independent column scans.

Those costs were especially damaging for low-dimensional continuous controls:
the FE problem itself was trivial, but the input and certification pipelines
scaled with `N * K`. Removing a certificate or weakening a tolerance would
make the comparison faster but violate ADR 0003. The optimization therefore
had to reduce data movement while retaining an independent publication check.

## Decision

1. The Stata control plane may certify that a full candidate sample has no
   missing value in any source used by the model. It does so with Stata's own
   `markout` semantics before the one substantive plugin call and transmits a
   versioned `input_complete` bit. The original candidate variable remains
   unchanged.
2. Treat the bit as a performance certificate, not as scientific authority.
   The native complete-reader fails closed if any SPI value disagrees with the
   certificate. Requests with `if`, `in`, missing data, weights, strings, or
   incomplete candidates retain the original general path.
3. For a certified numeric full sample, read all requested numeric sources in
   observation-major order into their final column buffers. This replaces one
   complete SPI traversal per column with one batched boundary traversal and
   keeps periodic interrupt checks.
4. Transfer ownership of complete unweighted outcome and control vectors into
   prepared OLS storage. The sample, singleton, topology, and role checks are
   unchanged; only the redundant full-column copy is removed.
5. Resolve each prepared RHS recipe once and fill the row-major solve matrix in
   source-major order at one native thread. Multi-thread execution keeps the
   existing deterministic parallel partition.
6. Fuse single-thread weighted column norms into one row-major pass. The
   multi-thread implementation and fixed reduction order remain unchanged.
7. Construct the independent raw-residual publication vector by resolving
   each fitted column once and subtracting it source-major. The existing
   independent intercept/score/reconstruction certificate consumes that
   vector at the same tolerances. Fast memory accounting includes the added
   `N * 8` bytes whenever a finite coefficient is fitted; rank-zero fits do not
   allocate it.
8. Keep the action protocol and release staging strict. The action protocol is
   bumped from 1.4 to 1.5; older ado/plugin combinations fail before reading
   observations.
9. Do not adopt a dynamically rescaled one-pass group-mean kernel. It passed
   correctness tests but slowed the representative K=30 native path by about
   12 percent, so it was reverted before qualification.
10. Pin Greg's exact open-source comparator tree in `references.lock.toml`.
    Greg remains a beta performance comparator, not a compatibility oracle or
    runtime dependency. Only frozen, self-consistent comparator bundles are
    valid evidence.
11. Accumulate the guarded Cholesky moment blocks with ordinary portable
    multiply/add operations, then merge blocks in the existing fixed order
    with compensated addition. Do not use `f64::mul_add` in this dominant
    portable x86-64 loop: without a compile-time FMA target feature it becomes
    one software `fma()` call per moment. The independent score and bread
    certificates remain the fail-closed authority, and any rejected Cholesky
    candidate still falls back to rank-revealing QR.

## Local evidence

The focused prototype used Stata/MP 19 and the established one-million-row
standard dataset with one worker FE, `tolerance(1e-8)`, `iterate(16000)`, and
one fereg thread. Seven warm repetitions of the final candidate with the local
eight-processor Stata control plane produced:

| Controls | Whole command | SPI input | Native work | Precommit total |
|---:|---:|---:|---:|---:|
| 0 | 0.306 s | 0.037 s | 0.122 s | 0.159 s |
| 30 | 1.306 s | 0.173 s | 0.967 s | 1.141 s |

The conservative ADR-0012 checkpoints were 0.399 seconds at K=0 and 2.809
seconds at K=30. The current local medians are therefore 23 percent and 54
percent lower end to end. Relative to the initial precommit K=30 baseline of
about 2.49 seconds, the new precommit pipeline is also about 54 percent faster.

A fresh one-processor Stata comparison, matching the established requested-one
thread benchmark convention, produced:

| Controls | fereg | reghdfe | fereg/reghdfe |
|---:|---:|---:|---:|
| 0 | 0.498 s | 0.374 s | 1.332 |
| 30 | 1.562 s | 4.410 s | 0.354 |

Thus the wide-control target is 64.6 percent faster than reghdfe on the local
lane, while the zero-control path still has about 33 percent boundary and
publication overhead to remove. The installed macOS Greg command currently
fails independently of fereg with its own temporary-matrix `r(601)`, including
when invoked first in a fresh Stata process. Local Greg timings are therefore
rejected; the frozen Linux/SCC comparator bundle is the authoritative Greg
gate.

## First SCC attempt and portable-x86 repair

The first exact-source SCC attempt at
`93292b94f84aee7fced05dcd6985037765dfeb3f` was scientifically clean: setup,
all twelve scopes, and collection had `failed=0` and `exit_status=0`; every
scope had 36 timing rows, one passing preflight, zero command failures, zero
science failures, and the requested native thread count. The merged output had
432 timing rows and 12 preflights, using only fereg, Greg, and reghdfe.

It nevertheless failed the performance gate on the portable Linux build. The
one-FE, 30-control, one-thread fit took 104.278 seconds, of which 102.942
seconds were native, versus 1.366 seconds for Greg and 13.813 seconds for
reghdfe in the same shard. The corresponding one-FE, 10-control time was
17.232 seconds. The near-quadratic width scaling identified the dominant Gram
accumulation, not FE recovery or SPI input, as the failure.

The cause was architectural: the distributed x86-64 binary intentionally does
not use `target-cpu=native`, so Rust's fused `f64::mul_add` lowers to a software
`fma()` call when FMA is unavailable at compile time. The K=30 Gram kernel made
hundreds of millions of those calls. The amended candidate uses ordinary
multiply/add inside fixed row blocks and keeps the compensated fixed-order
merge plus independent score and bread certificates. All 333 core tests, 54
plugin tests, integration/oracle tests, formatting, clippy with warnings
denied, and the repository checker pass after the repair.

The exact repaired source `947f79373306760a635494935cc84c0f5ebfcda8`
then passed the focused portable-x86 qualification. Setup `7362157`, smoke
`7362158.1`, and focused tasks `7362162.2-5` all have complete SGE accounting
with `failed=0` and `exit_status=0`. Each of the five shards has a strict
success receipt, 36 timing rows, one passing scientific preflight, zero
command/science failures, and the requested fereg thread count. The source
archive SHA-256 is
`11466f6fb15f173fc7b70dfbb8599fbf03f845444b56b9b312f18426dc8af914`;
the Linux plugin SHA-256 is
`94017d23d97c1a0890cd9a1a180dad4ce3482b5b29cbe6efdbece5016b3e7099`.

The repair reduced one-FE/K=30/one-thread fit from 104.278 to 3.210 seconds
and standard worker-firm/K=30/one-thread fit from 109.292 to 5.080 seconds.
Within the repaired shards, fereg beat reghdfe in all ten fit/save cells and
Greg in all four standard worker-firm cells. The repaired standard
worker-firm fit ratios were 0.901/0.368 against Greg and 0.200/0.178 against
reghdfe at one/sixteen threads. One-FE/K=30 fit remained 2.554 times Greg at
one thread but only 1.255 times Greg at sixteen threads. The full evidence is
captured under
`benchmarks/reports/captured/linux-x86_64/stata19/adr13-qualified/`.

All Rust workspace tests passed: 333 core unit tests, 54 plugin tests, and every
integration/oracle target. The compatible Python lane passed 291 tests and the
repository checker. The universal plugin passed binary audit and the actual
Stata-plugin ABI loader. Nine focused licensed fixtures passed for installation,
candidate samples, hidden OLS parity, CMG auto routing, memory modes, saved
outputs, missing FE interactions, clustered interactions, and time-series
materialization.

## Qualification gate

The gate passed in two exact-source stages. Source `93292b9` passed the full
twelve-scope scientific and operational matrix but exposed the portable-x86
software-FMA regression. Source `947f793` changed only that Gram-loop
arithmetic and passed five focused repair shards covering one/two FEs,
K=10/30, fit/save, and one/sixteen threads. Combined, the evidence retains the
required one/two/three-FE, 0/10/30-control, standard/low-mobility, fit/save,
and one/sixteen-thread scientific coverage while directly requalifying every
affected performance path. All comparator ratios are within-shard.

## Consequences

- Moderate-width controls no longer pay a separate SPI scan and prepared copy
  per source. Input is observation-major and prepared storage owns the loaded
  buffers.
- Independent scientific certification remains mandatory and uses unchanged
  tolerances. The new residual construction is faster but not self-certifying.
- Complete-data users receive the fast path automatically; missing, weighted,
  conditional, and string-source models retain the fully general semantics.
- The main remaining performance targets are the one-FE single-thread
  boundary/publication path and K=0 fixed overhead. Solver tuning is unlikely
  to close either gap: the repaired multiway K=30 path already beats Greg and
  reghdfe, while one-FE native work is dominated by finite-input and
  publication passes rather than iterative FE recovery.
- macOS remains the focused prototype and smoke-test lane. Exhaustive and
  acceptance performance evidence remains SCC-only.
