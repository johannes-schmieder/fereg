# Native work in no-FE OLS, 1 November 2026

The main native bottleneck is the generic treatment of the intercept: repeated,
mostly serial centering and certification passes over the full design. The finite
regression already has a cross-product/guarded-Cholesky fast path. Replacing the
small matrix factorization alone would leave most of the observed cost intact.
Matching regress on native work appears technically plausible for these ordinary,
well-conditioned regressions, but no optimized candidate has been implemented or
measured in this investigation.

This follows the [full command benchmark](simple-regress-macos-20261101.md).
It profiles the same published plugin, same million-observation Gaussian dataset,
and same Mac. No runtime, arithmetic, storage or routing changes were made.

## Measured decomposition

All values below are milliseconds. fereg times exclude its separately measured
Stata-to-Rust input stage. The regress comparator is its entire command, reused
from the original uninstrumented benchmark; it is not a separately isolated
regress solver time. Phase traces are diagnostic observations, not a new paired
performance comparison or a qualification result.

| Eight workers | 1 RHS | 16 RHS | 64 RHS |
| --- | ---: | ---: | ---: |
| Raw preparation: packed RHS and raw moments | 5.4 | 23.1 | 146.0 |
| Absorbing the constant, including norms and FE certificates | 41.1 | 159.9 | 606.3 |
| Finite fit: cross-products, factorization, associated work | 7.4 | 45.2 | 165.3 |
| Remaining native work, by subtraction | 40.3 | 45.3 | 92.3 |
| **Total fereg native** | **94.2** | **273.4** | **1009.7** |
| **Entire regress command, original benchmark** | **52.6** | **86.1** | **297.1** |

Component medians need not sum to the median total. The remainder is a descriptive
difference of medians, not a separately timed phase. The raw and constant rows
are disjoint subphases of projection; initial certificate and norm timings must
not be added to them again.

At 64 RHS variables, constant absorption is about 60% of native time; raw
preparation plus absorption is about 74%. At 16 RHS these shares are about 58%
and 67%. The finite fit is only about 16–17% in both cases.

| 64 RHS variables | 1 worker | 8 workers | 16 workers |
| --- | ---: | ---: | ---: |
| Raw preparation | 410.5 | 146.0 | 252.3 |
| Constant absorption | 618.0 | 606.3 | 608.4 |
| Finite fit | 524.6 | 165.3 | 124.1 |
| Total native | 1709.8 | 1009.7 | 1087.5 |

The finite fit benefits from additional workers. Constant absorption does not.
Sixteen-worker raw preparation was slower in this fixed diagnostic sample;
the measurements do not establish whether cache traffic, scheduling or desktop
noise explains that difference. No unfavorable observations were excluded.

## What regress does, and what is known about its implementation

The current [regress manual, Methods and formulas](https://www.stata.com/manuals/rregress.pdf)
states that it accumulates cross-products in deviation form and computes the
constant separately. For an ordinary model with an intercept, the underlying
mathematics is

\[
G=\sum_i(x_i-\bar x)(x_i-\bar x)',\qquad
h=\sum_i(x_i-\bar x)(y_i-\bar y),\qquad
G\hat\beta=h,\qquad
\hat\alpha=\bar y-\bar x'\hat\beta.
\]

After obtaining residual variance, conventional covariance uses the inverse of
the same small moment matrix with the appropriate intercept reconstruction.
Thus the large work is accumulating moments over observations, not factoring a
million-row matrix. The dominant cross-product cost is O(NK²); the small dense
factorization is O(K³). A 64-by-64 double matrix is only 32 KiB.

[Ken Higbee's 2004 StataCorp explanation](https://www.stata.com/statalist/archive/2004-05/msg00057.html)
describes a sweep operator with reordering for numerical stability and collinearity
decisions. [William Gould's 2013 explanation](https://www.stata.com/statalist/archive/2013-04/msg00211.html)
describes multiple inversion/solution methods, deviation-form accumulation and
some higher-precision arithmetic. These are historical implementation accounts,
not a complete specification of the current Stata 19 binary. They do establish
that “regress is fast because it blindly computes an inaccurate inverse” is an
inadequate explanation.

StataCorp's [Mata OLS example](https://blog.stata.com/2016/01/05/programming-an-estimation-command-in-stata-computing-ols-objects-in-mata/)
also describes compiled, parallel, accurate cross-product primitives. It is a
teaching implementation, not regress source code. The
[Stata/MP performance report](https://www.stata.com/statamp/performance-report/),
revision 3.4.0 of 25 September 2023, reports near-linear scaling for regress on
its own workloads. Those timings are not directly comparable to our Mac medians.

The algorithm explains why such scaling is feasible. Observation blocks can
accumulate private centered moments and combine only O(K²) state per block.
Workers have abundant independent work even when K=1. Stable centered summaries
can be merged with the between-block mean correction, so this approach need not
use cancellation-prone uncentered sums. Once moments are ready, expensive
numerical checks on the small matrix remain cheap relative to traversing N rows.
This is an explanation of the available parallelism, not a claim that Stata's
current private implementation uses exactly this block layout or reduction tree.

Our Stata stack capture contains proprietary routines without useful function
names. It cannot identify its exact SIMD kernels, memory layout, number of data
passes or present-day fallback policy. The documented moment algorithm and
observed command scaling are stronger evidence than guesses from those offsets.

## Why fereg currently scales poorly

The relevant source is the unchanged runtime revision
`864960f1a6c2a2d1d58747d63a4475843c9c4e28`.

1. **No explicit FE still goes through the absorbed-constant machinery.**
   The structural intercept enters the borrowed projector in
   [prepared_fit_view.rs](../../crates/fereg-core/src/ols/prepared_fit_view.rs).
   [borrowed_map.rs](../../crates/fereg-core/src/fe/borrowed_map.rs), in
   `absorb_borrowed_intercept_map`, performs setup and norms, an initial
   certificate, a constant projection, and a final certificate. This is a direct
   one-step centering operation, not many MAP iterations or an LSMR/CMG solve.

2. **Centering repeats expensive serial work.**
   `structural_constant_means` first scans every value to find scales, then scans
   again with scaled compensated accumulation. It is invoked by the initial
   certificate, by the projection itself, and by the final certificate. That
   gives six serial full-design scans for means/scales, plus serial mean
   subtraction and additional norm/finite-value scans. The packed matrix has y,
   K regressors and the reported constant. The explicit constant makes the
   initial certificate fail, so the initial pass does not avoid projection in
   these fits. The scaling and compensation serve numerical purposes, but their
   repeated serial organization is expensive. Function samples confirm all three
   mean call paths execute. The 606 ms absorption cost at eight workers is
   therefore consistent with source and sampled stacks, not merely a timing
   label.

3. **Cross-products parallelize across Gram rows with unequal work.**
   [finite_wls_kernel.rs](../../crates/fereg-core/src/ols/finite_wls_kernel.rs)
   divides the K Gram rows into roughly equal-size column partitions. Each
   partition scans all observations and standardizes its required suffix of
   regressors. Early rows have more upper-triangle entries than late rows, so
   equal row counts are not equal work. With one retained regressor there is only
   one Gram partition. With K=64 and eight partitions, the first contains
   64+63+...+57=484 upper-triangle entries out of 2080, limiting ideal scaling of
   that arithmetic alone to about 4.30 times; RHS, scaling, memory access and
   other phases alter the actual result. This layout preserves the observation
   accumulation order within each entry, an intentional numerical tradeoff.
   `accumulate_standardized_rows` is a prominent sampled hotspot.

4. **Preparation and post-fit work make more passes.**
   [one_shot.rs](../../crates/fereg-core/src/ols/prepared_fit/one_shot.rs)
   prepares raw moments and a packed RHS before projection. After the centered
   fit it also constructs a raw residual for intercept recovery/checking; it then
   projects the residual and prepares inference. At K=64 and eight workers,
   raw preparation alone takes 146 ms, raw-residual/bread work about 50 ms,
   residual absorption about 19 ms, and IID inference about 8 ms. These are
   secondary targets after constant absorption.

5. **The final score certificate is not another full K-column bottleneck.**
   [prepared_fit.rs](../../crates/fereg-core/src/ols/prepared_fit.rs),
   `score_denominator`, skips the raw score scan for kept finite columns; their
   score information comes from the stored standardized moments. The absorbed
   intercept retains a raw residual score check. Finalization is about 4 ms at
   eight workers across all three widths. This corrects the tempting but wrong
   inference from seeing an outer column loop in `certify_scores`.

The finite factorization in
[finite_operations.rs](../../crates/fereg-core/src/ols/finite_operations.rs)
already attempts guarded Cholesky, with rank-revealing QR when required. The
64-RHS capture directly sampled `cholesky_solution` and
`invert_guarded_cholesky`. This proves that path executed; sparse sampling does
not prove that every fit avoided all fallback work. There is no evidence here
that changing Cholesky to Stata's historical sweep operator would address the
dominant cost.

## Prospects for matching native performance

For well-conditioned no-FE OLS, matching regress is a credible engineering target.
There is no fundamental Rust or OLS complexity disadvantage. The existing finite
phase already takes about 45 ms at K=16 and 165 ms at K=64, compared with 86 ms
and 297 ms for the entire regress command. These comparisons show room for a
competitive pipeline; they do not prove a complete faster implementation exists.
The actual regress native-only target would be somewhat lower than its command
time and has not been isolated here.

The highest-value design change would be a general intercept-only moment path:
compute stable means and scales in balanced observation blocks, reuse them,
accumulate centered moments without repeated full-design centering/copying, and
combine blocks deterministically. It should retain necessary independent FE,
score and residual witnesses, current sample/rank/omission semantics and guarded
factorization fallbacks. Required residual construction and statistics could
share suitable passes. Work on Gram accumulation should favor balanced blocks
or balanced triangular tiles; changing reduction order requires an explicit
numerical scope and independent scientific validation. A no-FE mathematical
case is an appropriate general route; a special fence for these benchmark
dimensions would not be.

Two illustrative bounds show the priorities. Removing the entire finite-fit
phase from the K=64 eight-worker result would improve native time by only about
1.20 times. Speeding constant absorption up by an ideal factor of eight, with
everything else fixed, gives roughly 479 ms, still above regress's 297 ms command.
Therefore centering, redundant preparation and Gram scheduling all deserve work;
small-matrix factorization alone does not. These are arithmetic illustrations
using measured medians, not forecasts or measured candidate results.

A competitive path must use stable centered accumulation rather than naively
subtracting large uncentered cross-products. Changes to sample, rank or omission
semantics require a separate decision; performance does not justify relaxing
them. Native parity also would not remove the separate Stata-to-Rust input and
frontend costs documented in the original benchmark.

## Evidence and limits

- Host: Apple M2 Ultra, 24 cores, 192 GiB, macOS 26.6.2; Stata/MP 19 Apple Silicon,
  revision 12 August 2026, licensed for eight Stata processors. fereg workers 16
  are usable; there is no measured 16-core regress comparison.
- Published fereg 0.5.0-beta.12 plugin SHA-256:
  `28aed23eb1c8b8575fb1e1d3d49e7a2513066507353cd694f8c5269afc94f75c`.
- Frozen phase schedule: K=1,16,64 and workers=1,8,16, one warm-up and three
  shuffled measured repetitions per cell; 36 calls total. Existing trace flags
  were enabled on the unchanged binary. No candidate, declared numerical change
  or route change was introduced, and no external campaign environment was
  reused. This descriptive audit is not qualification through `bench/qualify.py`.
- Frozen sampling schedule: one 10-second capture each of fereg K=16/8 workers,
  fereg K=64/8 workers and regress K=64/8 processors. Loops contain respectively
  40, 20 and 80 fits after warm-up, with tracing disabled. All completed. The
  process elapsed times include sampling coordination and are not performance
  scores. All-thread stack counts identify executing routines, not fractions of
  elapsed time.
- No concurrent builds, tests, estimation or agent source reads during captures.
  Ordinary desktop noise retained; no adverse-result retries. Sandboxed launcher
  stalls were stopped before the scheduled measurements; the actual phase
  schedule and each fixed sampling capture ran once.
- All 36 traced coefficient vectors, full covariance matrices and RSS values
  passed the original independent Householder-QR references and campaign
  tolerances. Maximum absolute coefficient error: 1.63e-14; covariance error:
  1.33e-19; relative RSS error: 4.97e-14. N and residual degrees of freedom are
  exact, scientific vectors repeat exactly at fixed workers, and all worker
  counts agree with the independent reference. Each sampling-loop fit checks N
  and residual DoF; the final fit of each loop also passes the independent b/V/RSS
  checks. No claim is made that every sampling-loop vector was exported.
- This is a focused Mac diagnosis of unweighted IID, well-conditioned designs.
  Linux performance, ill conditioning and other inference modes need their own
  evidence before optimization or publication claims.

[Structured evidence](no-fe-native-profile-macos-20261101.json) retains every
phase record, the frozen schedule, summary statistics, numerical checks, compact
named-function sample summaries, hashes and reproduction sources. The
[original benchmark evidence](simple-regress-macos-20261101.json) retains the
data generator, independent references and uninstrumented command measurements.
Generated data, binaries and raw profiles are removed after these summaries are
recorded.
