# Generated terms, multiple slope partitions and fresh raw moments

Local Mac development milestone, 2026-09-22. Baseline source `cd599498`;
accepted candidate `rawtile1` is that source plus the archived `certificate.rs`
overlay. All 529 captured build inputs match the resulting computational source.
Candidate plugin SHA-256:
`016f3fb22bbf2b50f81b5b900a4c90d0e82997507686aefb57e41382cb9ab343`.
Baseline `covrow1` plugin:
`6f4ad854b739f5fd694f51f98f507dcc0e865d96ec4486564e6314dea0b07162`.

## Opportunity inventory

The additive harness now measures public lag, lead and difference terms, and
multiple intercept-plus-slope partitions, against stock fixest 0.14.2. Panel
setup occurs before timing in both applications; term generation occurs
inside estimation. IID/robust inference and declared outputs are matched.
Input loading and hashing precede timing; fit and requested prediction costs
are separately recorded. Stock uses its ordinary memory strategy, so the
native low-memory row is not an equal-budget comparison.

The five-case initial current-source screen found:

| Classification | Evidence |
| --- | --- |
| Measured losses | K10 lag/full outputs: native 66 versus stock 44 ms; mixed lag/lead/difference with gaps: 56 versus 36 ms. Two-partition slopes: 421 versus 55 ms; unbalanced three-FE slopes: 521 versus 75 ms. Scientific comparisons pass. |
| Launcher failure, subsequently corrected | All three K40 native launches terminated before estimation because the generated metadata exceeded Stata's startup argument buffer. Per-run captured includes replace long arguments. Corrected uninstrumented comparisons pass; the old failures remain in `screen/`. |
| Covered and competitive on this artifact | Corrected K40 serial lags have candidate 347 versus stock 368 ms medians, with matching science. This single row does not establish broad TS superiority. |
| Scientifically unresolved | Fresh weak slopes: both native artifacts fail warmup with r(459); stock reports nonconvergence. Fresh disconnected three-FE slopes: matching sample but stock coefficients, V and RSS disagree. Both comparisons remain unranked. |
| Still untested or limited | Higher-order/compound TS operators, arbitrary missing-source patterns, weighted stock TS, broad slope-only and saved multi-partition timing, and a positive-DoF weak multi-slope pilot. PPML receives a native categorical control and a licensed saved-slope fixture here, not a new broad stock timing claim. |
| Historical evidence | Earlier categorical, PPML, memory and serial-slope reports retain their own artifact identities. Their win counts and losses are not transferred to this checkpoint. Closed serial-slope/window experiments remain closed. |

No frozen primary, holdout or native-control definitions were edited. The
new designs are separate development cells. Seeds 202609250–254 are used for
pilots, paired inputs, diagnostics, fresh cases and controls, respectively;
the reserved frozen-validation seeds are untouched.

## Dominant cost and implementation

The automatic slope path uses the generic owned-FE design and its MAP pilot/
CG or LSMR continuation. The accepted one-partition slope cache does not serve
these multi-partition models. Public-command profiling places 85–90% of the
two original slope commands in initial absorption. Within those solves,
generic certificates cost 223/300 ms and projections 97/147 ms.

Per-level clocks in separate diagnostic builds identify raw diagnostic moments
as the largest certificate component. Their initial-solve totals fall from
120→55 ms and 176→76 ms. Normalization stays about 21–22 ms, QR 25–26 ms and
span projection 39/60 ms. These clocked artifacts are excluded from headline
timings; the stock reference library is uninstrumented throughout.

[ADR 0113](../../../../docs/adr/0113-owned-fe-raw-moment-tiles.md) batches fresh raw
moments into 32-RHS row tiles. It preserves every coordinate's scaling,
compensated arithmetic, observation order, zero behavior and overflow
saturation. Weighted QR and relative span certificates remain separate and
unchanged. There is no geometry cache across calls or PPML weight frames,
new solver selection, or changed scientific policy.

## Alternating timings and losses

Three repetitions rotate baseline, candidate and stock order. All builds,
profiling, independent oracles and archiving finish outside headline blocks.
Times below are complete requested-command medians in milliseconds. Ratios
are candidate/baseline paired minimum, median and maximum; the paired median
can differ from the ratio of separate medians.

| Case (N, K, threads) | Baseline | Candidate | Stock | Paired ratio min / median / max |
| --- | ---: | ---: | ---: | --- |
| Lag + saved FE/residual/xb/xbd (61440,10,4) | 66 | 64 | 43 | .955 / .970 / 1.014 |
| Mixed TS with gaps (61440,10,4) | 55 | 57 | 34 | 1.000 / 1.036 / 1.036 |
| Wide serial lags with gaps (61440,40,1) | 350 | 347 | 368 | .977 / .989 / 1.038 |
| Two-partition slopes (61440,10,4) | 421 | 362 | 55 | .846 / .848 / .866 |
| Unbalanced three-FE, two slope partitions (61440,10,4) | 519 | 428 | 76 | .817 / .836 / .839 |
| Fresh unbalanced lag/gaps + outputs (76800,8,8) | 59 | 60 | 39 | 1.000 / 1.017 / 1.017 |
| Fresh low-memory mixed TS (30720,12,1) | 84 | 83 | 36 | .976 / .988 / 1.000 |
| Fresh disconnected three-FE slopes (76800,20,8) | 2020 | 1802 | unranked | .857 / .894 / .917 |
| Fresh wide two-partition slopes (76800,33,4) | 1360 | 1091 | 236 | .786 / .802 / .820 |
| Categorical one-FE control (61440,10,4) | 21 | 20 | — | .952 / .952 / 1.000 |
| Categorical two-FE control (61440,10,4) | 80 | 80 | — | 1.000 / 1.000 / 1.000 |
| Public categorical PPML control (61440,6,4) | 224 | 221 | — | .978 / .987 / 1.027 |
| AW robust multi-partition control (30720,10,4) | 417 | 356 | — | .844 / .854 / .854 |
| PW clustered multi-partition control (30720,10,4) | 412 | 355 | — | .854 / .859 / .862 |

Six slope rows improve 10.6–19.8% by paired medians. The weighted controls
deliberately absorb x1/x2 and preserve their omitted finite-column statuses;
they are native comparisons, not ordinary-fixest weight inference oracles.
Seven of eight scientifically comparable stock rows still favor fixest.
No further tile-width search or case-dependent guard followed these timings.

Losses are retained: mixed TS +2 ms, fresh saved-output TS +1 ms, a wide TS
pair +13 ms, and a PPML control pair +6 ms. Small mixed control changes do not
support an optimization claim for those paths.

## Scientific and failure evidence

All 42 successful native pairs and 56 native repeat checks pass the unchanged
coefficient, full covariance, sample and objective envelopes. Thirty-nine
pairs have identical scientific CSV exports; all six saved-output binary
pairs are identical. The remaining three pairs belong to the parallel
disconnected case: maximum coefficient difference is 1.01e−16 and covariance
difference 3.19e−17. Both artifacts also vary slightly across their own repeats;
status, DoF, FE and subset metadata remain identical.

Direct candidate/stock checks pass 24/28. The three disconnected comparisons
fail coefficients, covariance and RSS; the first coefficient discrepancy is
6.88e−7 and RSS discrepancy .01667. Native residual DoF is 39914, while stock
reports 36882 parameters at N=76748. Rescaling alone cannot adjudicate these
coefficient/objective differences. Its observed 7430 ms stock median is not
a comparable performance ranking.

The weak case has one complete three-arm failure round: stock nonconvergence
at 38.505 seconds, and r(459) native warmup failures on both artifacts. Stock
reports 15495 parameters at N=15360. This is a warning about the comparison,
not an independent rank calculation or an attribution of native r(459) to DoF.
The already-started second baseline attempt was interrupted and remaining
weak repetitions were not run. Exact original declarations, incomplete
request, process-stop receipt and all failures remain; the two other fresh
cases resumed unchanged in a separate root. A positive-DoF pilot and error
localization are required before further weak multi-slope timing.

The acceptance blocks contain 114 completed attempts: 111 successes, one
stock nonconvergence and two native warmup failures, plus the one interrupted
attempt. No completed attempt has a timeout, detected overlap or monitor
error. The initial screen's three launcher crashes are additional failures,
not removed or counted as successful estimates.

Validation also passes:

- Five independent bounded designs on baseline, candidate and stock: original-
  axis keyed TS lookup before availability/singleton pruning; explicit FE and
  indicator-by-slope projections; independent rank, b, V, sample, RSS and outputs.
- 32 distinct focused Rust tests, including eight FE-certificate oracles and
  eight absorbed-slope oracles; two optimized tile tests additionally pass.
  Entry-by-entry bit checks span widths 1–129, reordered observations, zero
  columns/channels, cancellation, extreme scales and raw overflow saturation.
- Actual-plugin ABI/panic containment and four harness boundary tests.
- Four licensed fixtures: `estimate_ols_slope_only_multiple_parity`,
  `estimate_ols_time_series_parity`, `feppml_saved_slopes`, and
  `estimate_ols_eclass_transaction`, with explicit fixture and terminal sentinels.

The setup's missing Stata executable environment, an undersized pilot graph
warning, the original output-row alignment failure, a diagnostic-instrumentation
anchor failure and a test-draft type error are preserved. Corrected pilots,
instrumentation and tests pass; none is silently relabeled as a passed attempt.

## Memory and next subsystem

Three fixed arrays add 768 bytes of stack payload, with no additional heap
buffer or new admission rule. Tracked native peaks are identical outside the
parallel disconnected row, whose pair differences span −19.16 to +22.28 MB.
There is no tighter whole-action or constrained-budget memory claim. The
static allocation audit still fails documented source drift and was not
regenerated. Weighted constrained-fit failures, nonretaining CG span instability,
factor-base provenance and SCC/Linux deferrals remain open.

Close this tile experiment. The next useful performance subsystem is the
generic owned-FE projection/certificate lifecycle: investigate reusable local
geometry and repeated projection work on independently valid multi-partition
designs. It still leaves substantial losses on strong/unbalanced slopes.
First establish a valid weak-design pilot and preserve the disconnected stock
discrepancy as a separate scientific question. Narrow TS overhead is a secondary
target; do not tune this tile size or reopen closed serial-slope windows.
This is a clean local implementation milestone, not completion of the broad
goal, qualification, push or release.

## Immutable receipts

[Archive](receipts.zip) and [checksum](receipts.sha256): 5,020 verified entries
plus the hash manifest, 40,199,498 compressed bytes. SHA-256:
`41582394510808a0c55e97fd0b2bec37d3e525cc3d6ce2254b77652bcf69587d`.
All archived hashes verify; machine paths are sanitized. Data, binaries and
licensed raw logs are excluded; original hashes, protocols, source identities,
plans, amendments, distributions and incomplete attempts remain.

The separate [oracle-boundary addendum](oracle-boundaries.zip) and
[checksum](oracle-boundaries.sha256) preserve final-review harness guards:
reject oversized slope designs before dense allocation and refuse nonpositive
residual DoF. Four harness tests and all five independent pilots pass again.
The production candidate and timed controllers are unchanged; the original
receipt archive remains immutable.
