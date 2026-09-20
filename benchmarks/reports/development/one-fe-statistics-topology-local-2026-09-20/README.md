# One-FE outcome statistics and topology ownership

Three further changes improve narrow one-FE OLS after the `84905c67` breadth
milestone: overlap independent outcome statistics in saved IID fits, transfer
unchanged categorical codes and membership instead of rebuilding them, and
extend the statistics overlap to robust and clustered inference. Full outputs
are now close to fixest in this local block, but the predeclared comfortable-win
endpoint remains open.

These are focused Mac development measurements, not release qualification.
Each cell has three rotated triplets, 983,040 observations, 196,608 FE levels,
10 or 50 regressors and sixteen native threads. The comparator remains the
pinned stock fixest 0.14.2. The same audited values, sample, inference request,
small warm-up and requested output bundle are used on both sides. Builds and
tests did not overlap timing. Imports and audit exports are outside the timer.

## Measurements

Command medians in milliseconds; each row compares contemporaneous arms.
Ratios and individual observations are retained in [timings.json](timings.json).

| Experiment / request | K | Previous | Candidate | fixest |
|---|---:|---:|---:|---:|
| Outcome overlap / saved IID | 10 | 190 | 182 | 218 |
| Outcome overlap / full outputs | 10 | 238 | 230 | 221 |
| Outcome overlap / saved IID | 50 | 470 | 473 | 546 |
| Outcome overlap / full outputs | 50 | 527 | 529 | 552 |
| Topology transfer / saved IID | 10 | 187 | 175 | 216 |
| Topology transfer / full outputs | 10 | 229 | 221 | 220 |
| Topology transfer / two clusters + saved FE | 10 | 263 | 256 | 341 |
| Topology transfer / saved IID | 50 | 459 | 460 | 518 |
| Topology transfer / full outputs | 50 | 517 | 516 | 538 |
| Topology transfer / two clusters + saved FE | 50 | 644 | 641 | 1403 |
| Inference overlap / robust | 10 | 163 | 156 | 162 |
| Inference overlap / FE cluster | 10 | 164 | 159 | 179 |
| Inference overlap / two crossing clusters | 10 | 202 | 197 | 253 |
| Inference overlap / two clusters + saved FE | 10 | 257 | 250 | 331 |
| Inference overlap / robust | 50 | 514 | 514 | 879 |
| Inference overlap / FE cluster | 50 | 500 | 494 | 931 |
| Inference overlap / two crossing clusters | 50 | 572 | 567 | 1303 |
| Inference overlap / two clusters + saved FE | 50 | 639 | 639 | 1371 |

The outcome-overlap guard excludes K=50; those first wide comparisons are
controls, not evidence of a wide gain. The later wide changes are small relative
to observed variation. The first preceding-candidate observation in the narrow
topology-transfer saved-IID cell was 227 ms; it is preserved along with the other
two observations of 180 and 187 ms. No slow observation was discarded.

The three series contain 162 successful application calls and 54 native paired
comparisons. All native comparisons have exact sample, b/V, coefficient status,
DoF, certificate and fit-statistic exports. All 36 comparisons requesting saved
outputs have byte-identical output arrays. The fixest comparisons retain the
existing scientific tolerances and exact sample checks.

## Mechanisms and scope

The ordinary IID path already overlaps an independent outcome mean/TSS scan
with fitting. The same arithmetic now runs beside categorical IID, ordinary
and categorical robust, and ordinary and categorical clustered fitting when
there is one intercept-only FE, unit weights, at least 16,384 rows, at most
16 model columns, fast execution and at least eight workers. The bounded team
operates only on immutable Rust-owned data. The scan adds no data buffer, and
its error stays deferred until the original fit, DoF and weight checks pass.

Robust and clustered composition can also reuse the fit's same-call independent
unit-weight residual sum. It is still computed from final residuals with the
existing scaled recurrence, not derived from fitted moments. Weighted and other
ineligible calls retain their existing statistics scans.

Categorical preparation transfers the availability stage's code vector and
membership index when its existing unit-weight, large-sample, single-source
singleton fixed-point guard proves the row axis is unchanged. Candidate-level
identities, canonical source mappings and retained-row order are preserved.
The receiving topology validates exact membership and nonempty final levels;
final prepared-data validation still runs. No code vector or membership index
is cloned. Interactions, changed singleton samples and weighted requests keep
their existing general construction. One-source slope requests can share the
structural transfer; numerical slope solvers are unchanged and no slope timing
claim is made here.

## Validation and artifact identity

[native-tests.json](native-tests.json) records the focused executions: ordinary
and categorical overlap equality/error checks, exact topology-transfer equality
and pointer ownership, malformed membership and injected allocation failures,
the construction-memory boundary, 13 robust tests, 10 clustered tests, exact
parent-budget behavior and fast/low inference equivalence. The overlapping
tests explicitly include collinearity, threshold boundaries and weight fallback.

The final candidate passes the saved-FE explicit-dummy oracle, current-data
prediction checks, candidate-input oracle and e-class transaction fixture.
Fixture receipts include the process result and both required sentinels.
Earlier candidates have their own fixture receipts. A generic clustered closure
initially failed Rust's `Send` bound; capturing the immutable OLS view directly
resolved that compile error before any plugin build or timing of that candidate.

The final artifact is `inferencetss`, plugin SHA-256
`ded173ab46c8cdda8da33e02345f2498afa90143129d2f8d4f59e717d8cb29e1`.
[candidate.json](candidate.json) binds its base `84905c67` and eight production
overlays. [identities.json](identities.json) binds every measured artifact and
verified source manifest. Separate exact-science files bind each native pair.
Raw licensed logs and observation-scale exports remain local under
`target/one-fe-breadth-optimization-20260920/`.

The subsequent `parallelmembership` experiment is outside this report's
candidate and timings. No new final five-pair blocks, complete regression
matrix, cross-platform qualification, installed-package replacement or push is
claimed. Narrow robust and full outputs, ordinary IID's comfortable margin,
and further reductions in already-winning wide paths remain active work.
