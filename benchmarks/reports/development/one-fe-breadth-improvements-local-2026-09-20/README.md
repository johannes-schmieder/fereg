# One-FE inference and output improvements, local Mac development

The owned-input pipeline now benefits robust, clustered and saved-FE OLS.
Further reuse of FE codes, singleton membership and strided score products
improves cases that already beat fixest. Plain numeric public predictions use
Stata matrix scoring on current data. The full-output request remains the main
gap, especially with ten regressors. The owner goal is **not achieved**.

These are source-bound development measurements on the same M2 Ultra, Stata
19 MP and pinned fixest 0.14.2 used by the
[preceding breadth comparison](../one-fe-inference-output-breadth-2026-09-19/README.md).
Each cell has 983,040 observations, one FE with 196,608 groups, unit weights,
and 16 native threads; Stata-side work uses its eight licensed processors.
Data import and audit exports are outside timing. Saved-output requests include
actual recovery and observation-level materialization in both applications.
The full-output request includes FE, residual, xb and combined prediction.

## Repeated measurements

The following three-pair blocks rotate baseline/candidate/fixest execution in
fresh processes after a small warm-up. Values are command medians in milliseconds;
paired ratios, dispersion and sampled RSS remain in [timings.json](timings.json).
No builds or tests ran concurrently with timing. Every measured call passes the
unchanged sample, coefficient, covariance, DoF, RSS and requested-output audits.

The `scorecache` artifact isolates selected-value reconstruction reuse from the
previous `clusterreuse2` artifact:

| Request | K | Previous candidate | scorecache | fixest |
|---|---:|---:|---:|---:|
| Robust | 10 | 167 | 164 | 171 |
| Cluster at FE | 10 | 167 | 164 | 180 |
| Two crossing clusters | 10 | 206 | 204 | 256 |
| Robust | 50 | 584 | 509 | 867 |
| Cluster at FE | 50 | 577 | 487 | 921 |
| Two crossing clusters | 50 | 654 | 580 | 1,308 |

The wide reduction against the preceding native candidate is 11–15% using
median paired ratios. Narrow changes are much smaller and overlap timing noise.

The `savedprep` artifact adds membership-based categorical singleton checks,
reuse with keepsingletons, and the existing ordered-code index constructor:

| Request | K | scorecache | savedprep | fixest |
|---|---:|---:|---:|---:|
| IID + saved FE | 10 | 202 | 190 | 211 |
| FE clustering + saved FE | 10 | 230 | 221 | 255 |
| Two crossing clusters + saved FE | 10 | 273 | 263 | 329 |
| IID + all outputs | 10 | 248 | 240 | 224 |
| IID + saved FE | 50 | 469 | 461 | 542 |
| FE clustering + saved FE | 50 | 566 | 564 | 1,015 |
| Two crossing clusters + saved FE | 50 | 657 | 649 | 1,396 |
| IID + all outputs | 50 | 546 | 524 | 532 |

The earlier twenty-cell `clusterreuse2` screen has one triplet per cell: seventeen
wins, one tie, and losses on both complete-output requests. It is preserved in
the receipts and is not presented as repeated acceptance evidence. The tables
above deliberately retain their measured artifact names rather than attributing
earlier measurements to later source.

## Mechanisms and scope

- Recognize categorical one-intercept partitions at the common fit boundary and
  clear their unused automatic fast-fit coefficient-retention request. Final
  output recovery and its independent reconstruction/projection certificates
  remain in place, including weighted one-FE requests.
- Carry the authoritative compiled RHS width, owned numeric rows and admitted
  raw moments through all OLS inference entry points. The overlapped SPI loader
  remains Mac-only and guarded by complete unit-weight numeric one-FE eligibility.
  Extra cluster-only sources do not become regressors. Every reused RHS value is
  checked against the final prepared sample; changed samples use reconstruction.
- Reuse FE encodings while adding final-sample cluster sources. This also applies
  to unchanged weighted and multi-FE preparation. Canonical source positions are
  remapped after adding a cluster source that precedes the FE source.
- Reuse validated one-partition membership to prove a singleton fixed point.
  Frequency-weighted and actual-deletion cases retain generic pruning. Ordered
  categorical codes use the existing index constructor; unsorted codes use the
  existing general constructor. Numeric topology and membership validation stay.
- Use the existing numeric row-tiled predictor for native outputs. Public plain
  numeric float/double xb predictions use `matrix score`; factor/omission recipes
  and integer storage retain `_predict`. Predictions still read current data and
  honor edits, sorting, missing values and if/in restrictions.
- Reconstruct each selected strided score value once per observation at ranks
  8–64. The fixed 64-double stack bank adds no heap owner. Every Gram entry keeps
  its prior multiplication, observation and reduction order. This applies to
  eligible fast robust/clustered inference across FE designs and weights;
  packed, scratch and other widths retain their existing computation.

The preceding report's explanation of saved-FE cost was incomplete: automatic
one-FE absorption already omitted initial coefficient products. The retention
request nevertheless blocked fresh-projection and borrowed-descriptor shortcuts.
The optimization changes those guards, not a previously retained coefficient bank.

## Profile evidence and rejected experiments

Twenty instrumented `breadthprofile` calls match `clusterreuse2` scientific
exports exactly; requested outputs also pass the existing audit. See
[profile checks](breadth-phases-science.json) and
[phase times](breadth-phases-summary.json). Before `savedprep`, narrow categorical
preparation takes about 36 ms, including 5 ms pruning, 12 ms topology construction
and 4 ms final preparation validation. Recovery takes about 16 ms and writing a
saved column takes 9 ms. Wide strided score certification takes about 97 ms before
`scorecache`; its full-command improvement is measured separately above.

A checked batch-output prototype passed its ABI and Stata checks but gave little
gain. A subsequent direct Mac store prototype passed the mock ABI test and failed
the real Stata output oracle. Both were removed. A combined prototype containing
the rejected store was never adopted or used as successful performance evidence.
The failed artifacts and raw diagnostic logs remain in the local work directory;
licensed logs are not committed.

## Validation and limits

The compact archive contains 206 successful calls: 60 screening, 54 score-product
comparisons, 72 saved-preparation comparisons and 20 phase probes. The collector
verifies every retained source-manifest entry and plugin hash. Identities, export
hashes, audited input hashes and full comparison summaries are retained alongside
the timings. Imported values are checked columnwise using IEEE-754 hashes.

Focused native checks cover unchanged row-product bits and nonfinite rejection,
weighted packed/strided inference, 72 topology tests, and 128 preparation cases
crossing FE count, cluster-source order, weight, sample change and keepsingletons.
The loaded-row comparison covers authoritative RHS alignment, missing-data
fallbacks and inference paths. Separate plugin tests check construction allocations,
exact parent budgets and fast/low equivalence. See [native tests](native-tests.json).

The Stata fixtures compare unit/AW/FW/PW fits with explicit dummy regressions,
including numeric/string/interaction FE and complete/missing samples. Prediction
checks cover data edits, sorting, qualifiers, storage types, omissions, factors
and an explicit zero coefficient. A shared OLS/PPML input oracle and transaction
fixtures also pass on `savedprep`; their source hashes and terminal sentinels are
recorded in the fixture JSON files.

The eleven three-pair size/thread/group-layout controls against `normonly` pass
with exact native scientific exports: geometric mean paired ratio **0.995912**,
maximum cell median ratio **1.022222**. No confirmation is required. See
[control receipts](regression-controls-savedprep.json).

The committed milestone is the separately built `breadthmilestone` artifact,
plugin `f413c53e847d3a3f09794ee247c635257f1728285a8595d2040347b34ee9c246`.
It restores the generic pruning mask's original early release point after review;
the measured `savedprep` source remains preserved. All 511 source files and the
working production overlays are verified in [milestone.json](milestone.json).
Its construction-allocation test and four Stata fixtures pass. Eight paired
checks against `savedprep` pass at both widths for IID, robust, full outputs and
two-cluster saved outputs; see [milestone checks](milestone-check/timings.json).
This adds sixteen calls to the sixty-six control calls and the original 206,
for 288 archived development calls. It does not turn the earlier blocks into
measurements of this later artifact.

The predeclared endpoint still requires two independent five-pair blocks with a
ratio at most 0.90 in every cell, plus the complete regression requirements on the
final frozen implementation. Those final blocks and the three-pair twenty-cell
breadth regression block have not run. No installed plugin was replaced, and this
is not release or cross-platform qualification.
