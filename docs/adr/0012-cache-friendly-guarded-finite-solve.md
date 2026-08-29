# ADR 0012: cache-friendly guarded finite solve

**Status:** Accepted
**Date:** 2026-08-28
**Decision owner:** Johannes Schmieder

## Context

The frozen 100-scope SCC subset of the V2 command matrix showed that saved-FE
recovery was no longer the primary bottleneck. On standard and low-mobility
multiway designs, fereg was usually substantially faster than reghdfe, and its
saved-FE increment was much smaller than either comparator on difficult
mobility graphs. Performance collapsed instead when the finite control width
reached 30, especially at one requested thread and with one absorbed effect.

Profiling isolated two causes in the finite regression checkpoint. First, the
old Gram builder scanned the complete row-major matrix once for every column
pair, producing poor cache locality. Second, the rank-revealing Householder QR
performed another sequence of full-height column operations even for clearly
well-conditioned, full-rank designs. In the worst completed SCC cells these
operations accounted for essentially all of the 200-plus-second native time.

Normal equations are faster but square the condition number. They therefore
cannot replace the existing rank-revealing route without an explicit guard and
the independent score and bread certificates required by ADR 0003.

## Decision

1. For at most 64 finite candidates, construct the standardized right-hand
   side and upper Gram triangle in cache-friendly row-major blocks. A row is
   loaded once per block and contributes to every requested moment. When the
   retained candidates are a contiguous generated-column range, use the
   corresponding row slice directly; noncontiguous candidates retain the
   generic indexed kernel.
2. Use at most 64 balanced blocks regardless of sample size. Blocks execute in
   the bounded native thread team; their fixed-order compensated reduction is
   deterministic and includes every row. This caps scratch at
   `64 * (K + K^2)` doubles.
3. Attempt a diagonal-pivoted Cholesky solve only when every Schur pivot is
   finite and exceeds the largest initial pivot times the square root of the
   existing relative QR threshold. The stronger square-root guard accounts for
   the condition-number squaring in normal equations.
4. The fast path is full-rank only. A weak, rank-deficient, nonfinite, or
   bread-certificate-failing design returns to the existing scaled
   column-pivoted QR, which continues to own omission and rank decisions.
5. Coefficients, the finite inverse, residuals, scores, and reported-coordinate
   bread retain their independent recomputation and unchanged tolerances. The
   optimization does not weaken FE, reconstruction, score, rank, or bread
   publication checks.
6. Designs wider than 64 candidates keep the prior bounded columnwise moment
   builder and QR path. This avoids an unbounded `blocks * K^2` allocation and
   makes the optimization reversible by construction.
7. Do not add a dynamic BLAS/LAPACK dependency. The release audit intentionally
   rejects it, and the targeted moderate-width kernel is small enough to keep
   in Rust while preserving deterministic reductions and the existing binary
   portability contract.
8. For raw matrices of at most 64 columns, compute the score/intercept cache in
   two deterministic row-major passes: one pass for unweighted maxima and
   weighted scaled sums of squares, and one for compensated weighted means.
   Fixed-order block merging retains stable norms while avoiding two strided
   full-height scans per column. Wider matrices keep the prior columnwise path.
   The added scratch is bounded by `64 * 3 * P` doubles, at most 96 KiB for
   `P=64` and 48 KiB for the measured 32-column raw matrix.
9. When no finite candidate survives, publish the same certified rank-zero
   result directly from the absorbed outcome instead of weighting and then
   unweighting it through an empty QR problem. Weighted fast/low parity remains
   a required test.

## Evidence

The decision was driven by the SCC source
`d452dbeab958b44c8d106d2147acf25613bceb48`, whose completed-scope report is
`benchmarks/reports/captured/linux-x86_64/stata19/command-comparison-v2-partial-100/report.md`.
That report is explicitly partial: 100 of 116 scopes passed every strict gate
before the remaining work was canceled.

Targeted macOS/Stata 19 prototypes used one million rows and the final
production algorithm. Warm paired medians were:

| Design | Controls | Mode | fereg | Greg | reghdfe |
|---|---:|---|---:|---:|---:|
| one FE | 0 | fit | 0.376 s | 0.184 s | 0.371 s |
| one FE | 0 | save FE | 0.472 s | 0.271 s | 0.737 s |
| one FE | 30 | fit | 3.154 s | 0.929 s | 4.380 s |
| one FE | 30 | save FE | 3.514 s | 1.038 s | 4.803 s |
| worker-firm | 30 | fit | 7.127 s | 6.421 s | 19.791 s |
| worker-firm | 30 | save FE | 7.439 s | 6.863 s | 20.853 s |

The one-FE/30-control fit used 1.632 seconds at 16 native threads. Scientific
preflights had zero failures; maximum coefficient discrepancies were below
`6.1e-15` and fereg reconstruction errors below `4.3e-15`. These local results
are targeted prototype evidence, not a substitute for an SCC exhaustive matrix
or an exact cross-machine before/after comparison.

A second focused production round isolated the remaining raw-moment cache. The
old routine walked the row-major raw matrix twice per column, so one-thread
moderate-width fits repeatedly fetched the same cache lines. The two-pass fused
implementation produced these same-host warm medians:

| Design | Controls | Threads | Before total | After total | Before native | After native |
|---|---:|---:|---:|---:|---:|---:|
| one FE | 30 | 1 | 3.154 s | 2.809 s | 1.903 s | 1.462 s |
| worker-firm | 30 | 1 | 7.127 s | 6.816 s | 5.825 s | 5.444 s |
| one FE | 0 | 1 | 0.376 s | 0.399 s | 0.121 s | 0.124 s |
| one FE | 30 | 16 | 1.632 s | 1.726 s | 0.442 s | 0.432 s |

Thus the main target improved by 10.9 percent end to end and 23.2 percent in
native work; the worker-firm case improved by 4.4 and 6.6 percent. At 16 native
threads the native change was a smaller 2.4 percent gain because the old
column-parallel cache was already effective, while total time moved within the
noisy local boundary-dominated band. Zero-control time likewise showed no
measurable gain. All four targeted preflights had `science_fail=0`; maximum
coefficient discrepancies were below `6.2e-15`, and saved-FE reconstruction
errors were below `4.5e-15`.

After the final lint-preserving iterator form and universal rebuild, the exact
audited plugin `638c6246b36b3b5754305ef754abecacb6ef39aa23bc95eb6101f7799038904a`
passed the same one-FE/K=30 preflight and a three-run warm confirmation at
2.608 seconds total and 1.338 seconds native. The more conservative seven-run
medians above remain the primary local timing evidence.

The broader licensed fixture pass also refreshed the deterministic candidate
receipt for the two- and three-way clustered hidden-adapter cases. The fused
moment reduction moved the covariance entries by only a few units in the last
binary place; repeated Stata 19 runs reproduced the new bits exactly, while the
independent licensed-reference tolerance checks continued to pass unchanged.
This is an expected summation-order consequence, not a relaxed scientific
tolerance or a changed reference output.

The exact candidate source
`6be8f067f5c54e46e71e470b55beda7eb30a0ea1` then passed a focused Linux/SCC
qualification spanning twelve one-million-row scopes. Every scope had a
strict success receipt, 36 timing rows, one passing scientific preflight, zero
command failures, and the requested native thread count; the merged evidence
contains 432 timing rows and 12 preflights. The matrix covers one, two, and
three fixed-effect dimensions; 0, 10, and 30 controls; standard and
low-mobility graphs; fit and saved-FE modes; and requested threads 1 and 16.
The exact Linux plugin SHA-256 is
`60d4b6525aedbb3bfbcfeb9f14d6b1b994545825f03f4fb2461cbfc352ee45a7`.

Within the K=30 target cells matched to the frozen pre-ADR SCC population, the
geometric mean of the fereg/comparator ratio changed by a factor of 0.672
against reghdfe and 0.777 against Greg. At sixteen requested threads the
focused candidate's overall geometric-mean ratios were 0.916 against reghdfe
for fit and 0.839 for saved output. The hard multiway sixteen-thread fit cells
ranged from 0.260 to 0.650 times reghdfe. Multiway saved-FE publication added
5.9--9.2 percent.

The scheduler spread shards across heterogeneous 16-, 20-, and 32-processor
nodes, so cross-run absolute times and apparent scaling are secondary evidence.
The collector preserves each original shard environment and computes command
ratios only within a shard. A conspicuous one-FE/K=10 one-thread SCC time of
17.378 seconds did not reproduce in a same-source macOS diagnostic, which
recorded 1.010 seconds total and 0.391 seconds native with `science_fail=0`.
No SCC cell was filtered. The full evidence and operational provenance are in
`benchmarks/reports/captured/linux-x86_64/stata19/adr12-focused-12/`.

## Consequences

- Wide, well-conditioned control sets no longer pay repeated full-row scans or
  Householder QR, and moderate-width raw score/intercept caches no longer pay
  strided per-column scans. Difficult or wider designs retain the previous
  scientific paths.
- Saved-FE recovery remains a small increment in the representative wide
  prototypes: 11.4 percent for one FE and 4.4 percent for worker-firm. Further
  recovery optimization is lower priority than finite-kernel and Stata/plugin
  boundary work.
- Greg remains the speed target, particularly for one-FE designs. At zero
  controls, fereg native work is already small enough that input and result
  publication dominate; future gains there require reducing boundary passes,
  not changing the FE solver.
- Exhaustive performance qualification remains SCC-only. macOS remains the
  fast local lane for focused profiling, anomaly diagnosis, and correctness
  checks. The focused SCC qualification establishes scientific and operational
  breadth for ADR 0012, but does not replace the exhaustive release matrix.
