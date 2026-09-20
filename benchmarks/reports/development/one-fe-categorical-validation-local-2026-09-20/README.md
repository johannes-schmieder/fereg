# Categorical validation placement and publication experiments

Moving categorical IID and robust DoF validation inside the already resolved
worker team reduces redundant serial scans of wide numeric input. The accepted
candidate is `teamonly`, based on `83c8751b` with only the two inference-file
overlays in [candidate.json](candidate.json). It retains the same checks, error
order, memory owners and numerical algorithms. Clustered and ordinary inference
are unchanged. The shared categorical entry points include saved-FE, weighted,
factor and absorbed-slope requests; this timing block covers unit one-FE OLS.

These are local Mac development measurements, not release qualification. The
endpoint in the frozen acceptance manifest remains unmet. Narrow full outputs
are approximately tied with fixest, and the wide full-output margin remains
below the required ten percent in this block.

## Measurements

Three rotated repetitions, 983,040 observations, 196,608 FE levels, sixteen
native workers; pinned stock fixest 0.14.2 and the same requested output bundle.
Builds and tests did not overlap timings. Command medians in milliseconds:

| Request | K | Prior `inferencetss` | `teamonly` | fixest |
|---|---:|---:|---:|---:|
| Saved FE | 10 | 175 | 174 | 218 |
| Full outputs | 10 | 224 | 216 | 218 |
| Saved FE | 50 | 449 | 437 | 528 |
| Full outputs | 50 | 528 | 507 | 536 |

An earlier experiment combined team placement with a complete-sample row-map
check. It measured wide saved FE at 439 versus 458 ms and full outputs at 503
versus 513 ms. The row-map change by itself produced no consistent improvement;
`teamonly` removes it. Individual observations and paired ratios are preserved in
[timings.json](timings.json), including all slow observations.

## Rejected experiments and diagnostics

- Parallel exact membership validation had mixed results in the one-repetition
  twenty-cell screen. It passed 180 malformed/valid cases and the topology and
  allocation tests, but the screen did not justify keeping the extra scheduling.
- The full-sample row-map shortcut passed 210 valid/malformed scenarios. Its
  three-repetition timing block showed no consistent benefit, so it was removed.
- Allocating private output variables together, while retaining missing-value
  initialization, did not consistently improve the narrow full-output case.
- Ordering the sample marker last made full outputs slower: 232/562 ms at
  K=10/50 versus `teamonly` at 216/507 ms. This diagnostic is not adopted.
- Temporarily omitting `buildfvinfo` produced only small, inconsistent differences
  (215/500 ms full outputs). It was a cost-isolation diagnostic; postestimation
  metadata equivalence was not established, and production retains the option.

The source manifests retain each separate artifact identity; no combined or
rejected artifact has been relabeled as the accepted candidate. Rejected patch
files and raw logs remain local under the campaign directory in `target/`.

The six `sample-ado-phases` calls isolate Stata phases using timers 81–88.
Timer 81 is pre-output metadata, 82 output setup, 83 plugin, and 84 all posting;
85–88 divide posting into validation, stripes/copies, ereturn post and remaining
publication. Each receipt retains both the small warm-up and main observation.
At K=10, the full-output diagnostic measured 19 ms output setup, 166 ms plugin,
and 15 ms posting (13 ms in ereturn post), followed by roughly 15 ms public
prediction cost. These single observations locate costs, not stable speedups.
The earlier native phase probe uses the rejected membership artifact and is
identified separately; it must not be treated as a `teamonly` profile.

## Validation and identity

[receipts.json](receipts.json) contains 187 successful application calls with
process status, explicit application return codes and terminal sentinels.
[identities.json](identities.json) verifies every native artifact's source
manifest and plugin hash. Exact-science files contain 99 comparisons: 20 from
the membership screen, one native phase diagnostic, six Stata phase diagnostics,
36 output-allocation comparisons and 36 final team/posting comparisons. All
native coefficient, covariance, status, DoF, certificate and fit-statistic
exports match exactly. All 85 applicable saved-output comparisons are byte
identical. Fixest comparisons retain the existing sample and numerical checks.

The accepted `teamonly` artifact passes four licensed Stata fixtures: saved-FE
explicit-dummy oracle, current-data prediction, candidate-input oracle and
transaction rollback. Earlier experiment fixtures and focused native test
receipts are retained separately. This is neither a full-suite nor an
exact-commit release claim. `teamonly` plugin SHA-256:
`fdc11d54843a7de44c9047c6ea9e7d7489d8281fd0eb2a4f482b5ed5304bf35c`.

The residual-overlap extension and no-fill temporary-output experiment are
subsequent work, outside this candidate and these measurements. No final
acceptance blocks, complete regression matrix, installed-package replacement,
remote push or cross-platform qualification is claimed.
