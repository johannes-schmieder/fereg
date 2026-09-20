# Wider independent-statistics and residual overlap

The retained change extends two existing one-FE overlap paths from sixteen to
sixty-four generated columns. It improves the already-winning wide IID,
robust and clustered cases, including saved FE. All thirty-one original-baseline
regression cells pass. The first final comparison block still fails the
comfortable-win target in two narrow cells; that failed block is preserved.
These are local Mac development measurements, not release qualification.

## Incremental evidence

Three rotated repetitions, 983,040 observations, 196,608 FE levels and sixteen
native workers on the M2 Ultra. Stata MP uses eight processors and external
BLAS one. Imports and audit exports are outside command timing. No builds or
tests overlapped timing. The stock comparator is pinned fixest 0.14.2, archive
SHA-256 `17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.

Command medians in milliseconds for fifty regressors:

| Request | Prior owner checkpoint | Outcome overlap only | Both overlaps | fixest |
|---|---:|---:|---:|---:|
| IID | 389 | 384 | 370 | 456 |
| FE, residuals, xb and xbd | 481 | 476 | 467 | 520 |
| Robust, saved FE | 545 | 542 | 521 | 927 |
| Two clusters, saved FE | 626 | 615 | 608 | 1,384 |

Median paired reductions for both overlaps versus the prior checkpoint are
4.88%, 2.89%, 4.04% and 2.88%. Every native allocation peak is identical across
the three native arms within each request: approximately 925.9, 932.9, 932.9
and 956.5 MB respectively. Exact peaks, sampled RSS and all repetitions remain
in [the memory record](wide-overlap-screen-memory.json) and
[the primary archive](primary/timings.json).

The independent outcome scan now overlaps eligible legacy IID and shared
categorical IID/robust/clustered fitting through width 64. It retains the
unit-weight, fast-mode, one-FE, at-least-16,384-row and at-least-eight-worker
guards. Residual projection, independent score certification and RSS may
overlap bread completion through width 64 under the same existing guards,
including auto/MAP, no retained FE coefficient product and no allocation-phase
probe. The fixed status array uses the same constant as its width guard.
Numerical operations, reductions, error consumption order and certificates are
unchanged. No new heap owner is introduced. Weighted, small-team, low-memory
and non-one-FE routes retain their previous scheduling.

An earlier broad outcome experiment regressed a four-worker wide control.
This change retains the eight-worker minimum; its four-worker controls do not
enter either widened path. It is not a revival of that rejected scope.

## First frozen five-pair block

The source and artifact were fixed before `widecore-final-a`. All 200
application calls pass and fereg wins all 100 pairs. Eighteen of twenty cells
have median paired fereg/fixest ratios at most 0.90. The limiting cells are:

| Request | K | fereg median, ms | fixest median, ms | Median paired ratio |
|---|---:|---:|---:|---:|
| IID | 10 | 126 | 140 | 0.907143 |
| Full outputs | 10 | 201 | 221 | 0.917051 |
| Full outputs | 50 | 471 | 524 | 0.898855 |

The complete per-cell results are in
[the failed-block assessment](widecore-final-a-assessment.json). Ratios use
within-repetition pairs, not ratios of displayed medians. The second block was
not run because the first already fails. Neither this block nor its original
[preselected plan](widecore-final-plan.json) is replaced by a favorable screen.
The active goal still requires a stronger candidate and both final blocks.

The current candidate's twenty breadth regression cells versus original
`normonly` have maximum median paired ratio 0.976744 and geometric mean
0.677801. The eleven size/thread/group controls have maximum 1.015692 and
geometric mean 0.992741. No cell needs a confirmation run. These cumulative
gains must not be attributed solely to the width extension.

## Validation and next bottlenecks

The archive contains 442 successful application calls: 48 incremental, 200
final-block, 120 breadth-regression, 66 size/thread/layout and eight diagnostic
phase calls. There are 221 nonself exact native comparisons and 84 byte-identical
saved-output comparisons. Samples, coefficients, covariance, statuses, DoF,
fit and certificate arrays match the preceding checkpoint exactly. Comparisons
against original `normonly` retain the registered checks; the inherited saved-FE
fit/certificate norm differences are adjudicated in the
[owner checkpoint](../one-fe-validated-owners-local-2026-09-20/README.md), and
all sixty current breadth results match that checkpoint exactly.

Eleven focused Rust tests pass on the experimental combined implementation;
the two new wide tests are also rerun individually on the final source.
They cover numeric and categorical inference, rank deficiency, packed/strided
score caches, exact residuals, independent RSS and simultaneous injected
failures. Three Stata fixtures pass on each of the three new native artifacts,
including the independent input oracle, final saved-FE recovery and transaction
rollback. All actual-plugin ABI checks pass. [Validation](validation.json)
binds tests and final source; separate fixture receipts preserve earlier arms.

The eight-call unmodified-artifact profile locates narrow IID's combined
descriptor/RHS validation at 5.3–5.6 ms, initial absorption at 11.7–12.2 ms,
finite solve at 7.8–7.9 ms and overlapped bread/residual completion at 8.3–8.4 ms.
The saved categorical RHS phase is only 1.3–1.7 ms. These diagnostic boundaries
motivate checking redundant numeric validation in the legacy RHS reuse path;
they do not establish a further gain. Full-output storage, Stata posting and
prediction remain additional costs. Previously rejected output and posting
experiments are not silently reinstated.

Final `widecore` plugin:
`ee6eb5df6ef656b6765dea5bfc5b016b4154ffb6ed9395833236649bc34e20e1`,
built from `6cb8c230` plus the two recorded core overlays. Source hashes match
the retained implementation. The build helper embeds September 19; these
executions and collection occurred September 20. No installed package,
remote state or cross-platform qualification changed.
