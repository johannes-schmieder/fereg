# Inference residual overlap and temporary output initialization

The retained change extends narrow residual/bread overlap to fits retaining a
sandwich score cache. A separate experiment avoids filling private output
variables before the plugin overwrites them; it remains experimental because
of the filtered-sample slowdown described below.
These local Mac development measurements do not yet meet the comfortable-win
endpoint, and they are not release qualification.

## Measurements

Three rotated repetitions per cell, 983,040 rows, 196,608 FE levels and sixteen
native workers; the same audited inputs, stock pinned fixest 0.14.2, inference
and requested numerical outputs. No builds or tests overlapped timing.
Command medians in milliseconds:

| Residual overlap request | K | Prior `teamonly` | `inferenceresidual` | fixest |
|---|---:|---:|---:|---:|
| Robust | 10 | 157 | 144 | 170 |
| FE cluster | 10 | 158 | 145 | 180 |
| Two crossing clusters | 10 | 197 | 186 | 248 |
| Two clusters, saved FE | 10 | 246 | 241 | 330 |
| Robust | 50 | 504 | 509 | 868 |
| FE cluster | 50 | 498 | 486 | 932 |
| Two crossing clusters | 50 | 575 | 575 | 1306 |
| Two clusters, saved FE | 50 | 639 | 653 | 1391 |

K=50 is outside the overlap guard. Its variation is retained as control
evidence, not attributed to the optimization.

| Output initialization request | K | Prior `teamonly` | `outputnofill` | fixest |
|---|---:|---:|---:|---:|
| Saved FE | 10 | 170 | 169 | 219 |
| Full outputs | 10 | 219 | 216 | 213 |
| Saved FE | 50 | 437 | 433 | 533 |
| Full outputs | 50 | 498 | 489 | 530 |

Full-output paired savings are 5/5/3 ms at K=10 and 14/15/2 ms at K=50.
The smaller saved-only differences are noisy. All observations and paired
ratios appear in [timings.json](timings.json); none were discarded.

The additional half-missing-sample control contains 12 calls and six exact
native/output comparisons, archived in [partial-control](partial-control/).
Narrow full-output medians are 206/206 ms for baseline/experiment; wide medians
are 453/469 ms, with a median paired ratio of 1.0353. Although below the declared
five-percent per-cell ceiling, this repeatable wide slowdown motivates moving
excluded-row initialization into the existing native publication pass before
adopting the output change. The output experiment is not part of the retained
production checkpoint.

## Mechanisms and invariants

The existing residual-overlap guard still requires unit weights, one ordinary
intercept FE, fast execution, at least 16,384 observations, 8–16 generated
columns, at least eight installed workers, auto/MAP and no retained FE
coefficient product. Allocation-phase probes keep the sequential route. Only
the exclusion of a retained inference score cache is removed. The immutable
packed or strided cache was already live across bread completion and residual
projection. The same admitted owners now execute on the same scoped team.
Projection/score errors are still consumed at their original boundaries after
bread and initial within-TSS validation. The optional RSS is independently
computed from final residuals with the unchanged scaled recurrence.

This extends to ordinary and categorical robust/clustered fits, including
eligible saved-FE requests. Existing IID, weighted, wide, low-memory and
non-one-FE routing is unchanged.

Output allocation uses `st_addvar()` with no-fill only for private double
tempvars. This is the temporary-variable case documented by the
[Stata manual](https://www.stata.com/manuals/m-5st_addvar.pdf). The sample marker
keeps its existing initialization. The plugin fills every retained output row.
After complete result validation, the existing receipt's physical active-row
count determines whether excluded rows need missing values. If so, ado fills
those rows before any variable is renamed into public state. The count is not
frequency-weighted N. Errors or Break discard the private tempvars through the
existing transaction. No raw output is read before it is filled, no persistent
fitted-value cache is added, and the single guarded estimation call is unchanged.

## Validation and identities

There are 108 accepted application calls and 36 exact native scientific
comparisons, with 18 byte-identical saved-output comparisons. All process,
application and sentinel checks pass. Source manifests and plugins are
verified by [identities.json](identities.json). Native tests include:

- Exact full-fit and error-order comparisons with retained packed/strided scores
  across the 16,384-row threshold and 1/8/16 workers, with injected intercept,
  bread and residual failures. Reused RSS is checked against a fresh scan.
- Independently observed allocation peaks for eligible robust and clustered
  fits with 16,385 and 524,301 rows, exercising packed and strided retention;
  the existing numerical-frame plus solver-workspace bound covers every case.
- All 13 robust and 10 clustered module tests, including their weighted,
  oracle and failure checks.

`inferenceresidual` passes four Stata scientific/prediction/transaction fixtures.
`outputnofill` passes those four plus a new full-output fixture covering missing
and restricted samples, singleton removal, all four weight modes, all five
native output channels and seven rollback stages with exact prior sample/b
and variable-list restoration. The first two attempts at the new fixture
incorrectly supplied zero frequency and probability weights; the estimator
correctly rejected them. The fixture was corrected to use admissible weights,
retaining zero analytic weights. Those failed logs remain local and are not
counted as accepted executions.

Artifacts are separate source snapshots, not an exact release SHA:

- `inferenceresidual`: `eb29e5702a19c9e855375f3d56620a896eb2263d0adc4fc8f814e25de6bc641c`.
- `outputnofill`: reuses `teamonly` native plugin
  `fdc11d54843a7de44c9047c6ea9e7d7489d8281fd0eb2a4f482b5ed5304bf35c`
  with the verified ado overlay.

The subsequent narrow certificate and recovery-validation experiments are
outside this report. Final
acceptance blocks and the complete regression matrix remain pending. No
installed package, remote state or cross-platform qualification changed.
