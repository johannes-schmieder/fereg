# Veneto AKM comparison, October 24, 2026

All **30 fereg fits pass** the frozen scientific and saved-output gates on the
long and short Veneto panels used by fevc. Median command time falls from
**16.266 to 9.616 seconds** on the long panel and **3.443 to 2.408 seconds** on
the short panel when moving from one to sixteen physical cores: descriptive
scaling of **1.69x** and **1.43x**, respectively.

There is **no scientifically eligible cross-package speed ratio** in this
campaign. Default fixest and reghdfe fail the independent saved-predictor score
check. Strict fixest completes all repetitions but its centered saved worker
and firm effects disagree with fereg beyond the frozen tolerance; its short-panel
saved prediction also fails the cross-package fitted-value check. Their command
times remain visible below. These observations identify failed checks, without
diagnosing the underlying numerical cause or proving which full-panel FE
solution is correct by an independent dense oracle.

## Models and retained samples

The response is unadjusted log daily wages. Each model includes worker and firm
intercept effects, unit weights and regular IID OLS covariance. Both FE channels
are saved **inside command timing**, including fixest's lazy FE recovery and
mapping back to observations. Data import, disk export and validation are outside
the measured command. These are estimation/output timings, not complete job times.

| Panel | Years | Rows | Workers | Firms | Controls |
| --- | --- | ---: | ---: | ---: | --- |
| Long | 1996–2001 | 5,163,428 | 1,106,419 | 86,647 | `((age-40)/40)^2`, `((age-40)/40)^3`, 1996–2000 indicators; 2001 base |
| Short | 1999, 2001 | 1,319,994 | 659,997 | 42,489 | 2001 indicator |

The retained samples reproduce the existing connected, leave-worker-out fevc
inputs, with no singleton FE levels. Long-panel cleaning and graph preparation
reuse the original six-year fevc procedure. The short panel uses its own original
source-order handling of earnings ties; reusing the long-panel cleaning was
rejected before timing. An initial preparation launch also failed for a missing
input before estimation. Both preparation observations are preserved in the
private campaign. No timed fit was retried.

The SCC input audit verifies exact observation, worker and firm identities
against retained fevc files. Differences between unadjusted and previously
adjusted wages lie in the declared control span, with maximum errors
`1.54e-13` (long) and `1.01e-13` (short), below the `1e-12` gate. The reproduced
long sample is 18 rows below the historical published count; it matches the
retained fevc input exactly. No sample change was made to close that difference.

## Core scaling and observed reference times

All times are seconds. Fereg and strict fixest entries are three-command
medians. Every asterisk denotes a scientifically ineligible comparator. Default
fixest and reghdfe entries are **one completed command**, followed by two skipped
positions at that core count after the capture failure. Their values are not
three-repetition medians. Ranges and individual observations are retained in the
linked evidence. No eligible comparator speed ratio is reported.

| Long panel cores | fereg | fixest default* (n=1) | fixest strict* (n=3) | reghdfe* (n=1) |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 16.266 | 65.238 | 115.975 | 557.015 |
| 2 | 13.218 | 38.728 | 67.276 | 329.873 |
| 4 | 11.059 | 24.844 | 40.362 | 235.638 |
| 8 | 9.825 | 17.120 | 26.145 | — |
| 16 | 9.616 | 17.396 | 26.403 | — |

| Short panel cores | fereg | fixest default* (n=1) | fixest strict* (n=3) | reghdfe* (n=1) |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 3.443 | 7.629 | 12.767 | 63.331 |
| 2 | 3.098 | 5.632 | 8.522 | 47.433 |
| 4 | 2.711 | 5.714 | 8.752 | 38.292 |
| 8 | 2.493 | 5.695 | 8.654 | — |
| 16 | 2.408 | 5.498 | 8.549 | — |

Most native gains arrive by eight cores. Eight to sixteen reduces median time
by 2.1% on the long panel and 3.4% on the short panel. The sixteen-core native
ranges are 9.543–9.683 and 2.394–2.452 seconds. Small differences are descriptive;
three process repetitions do not establish between-host uncertainty.

Fixest is pinned at **0.14.2**. The default profile remains unchanged; strict
OLS uses `fixef.tol=1e-11` and `ssc(K.exact=TRUE)`. Reghdfe is **6.14.1**, commit
`29b2b203f534413369d675d0c0f674d61291f536`, with its pinned ftools/require
dependencies. Stata/MP 19 is licensed for four cores, so reghdfe is measured only
at 1/2/4. Fereg's native worker budget is explicitly verified at 1/2/4/8/16;
fixest uses the same requested physical-core prefixes. Reference sources and
all used runtime files are frozen and checked before and after execution.

## Validation and limits

All **108 planned positions** from ten configurations are accounted for: 60
successful captures (30 native and 30 strict fixest), 16 capture errors (ten
default fixest, six reghdfe) and 32 skipped repetitions. There were no command
timeouts. All ten native arm cells are eligible; all 26 reference arm cells are
ineligible. Every adverse observation and every skipped position is retained.

Native validation includes full sample retention, independent connected-graph
rank, stable omission/rank decisions, regular covariance, objective replay and
independent residual model scores. Twenty fixed-worker repeat checks require
exact scientific CSV/raw FE channel hashes; reported RSS also repeats exactly.
Thirty consistency comparisons against each panel's first one-core fit pass,
including six one-core baseline comparisons and 24 between different core
counts. Additional coefficient/covariance/objective checks against the four-core
witness pass. Each panel's native dense-design smoke independently passes
coefficients, covariance and fitted values.

Default fixest and reghdfe fail the independent saved-predictor model-score
bound of `1e-7` in every production cell. Strict fixest passes its capture,
sample, slope, covariance and objective comparisons, but both centered FE
channels disagree in all ten cells. All five short-panel cells additionally
disagree on the saved predictor. Centering uses observation-weighted channel
means on the connected graph, removing the arbitrary additive FE normalization.
The frozen comparison tolerances (relative/absolute) are coefficients
`1e-7/1e-10`, covariance `1e-6/1e-10`, fitted values and centered FEs `1e-7/1e-9`,
and objective `1e-8/1e-10`. No profile or tolerance was changed after observing
these results. Strict fixest and reghdfe pass both dense smokes; default fixest
passes the short smoke and fails the long smoke's model-score check. Smoke
passes do not override production failures. No full-panel dense oracle was run.

Setup `8270890`, smoke `8270891` and production array `8270918` tasks 1–2 all
have structurally complete accounting with `failed=0`, `exit_status=0` and the
application sentinels. Setup passed 362 benchmark-tool tests on Linux. Production
used separate complete 28-core Broadwell E5-2680 v4 allocations on `scc-ug4`
(long, 4,145 seconds) and `scc-ui4` (short, 895 seconds), totaling 1.40 node-hours.
Scheduler-reported `maxvmem` was 4.619 GiB and 2.579 GiB; sampled estimator-tree
RSS in the summaries is a different memory measure.

Both host reports pass replay of the whole-host reservation and all-queue
census. The long task retains 822 process and 70 census observations; the short
task retains 171 and 16. Coverage checks pass with no known competing compute.
All arms share socket-ordered nested CPU prefixes and NUMA memory binding within
each panel's allocation. Sixteen cores crosses the two 14-core sockets. Ordinary
OS activity is retained, and passive observation covers known compute activity.
One allocation per panel provides no independent physical-host replication.
Different hosts, sample sizes and controls prevent interpreting the long/short
timing difference as an isolated panel-length effect. These real-data results
are distinct from the [synthetic beta.3 campaign](fixest_benchmarks.md).

## Magnitudes of the accuracy differences

A subsequent diagnostic reads the existing **four-core first repetition** of
each arm on each panel. It does not refit models, replace observations or change
the campaign gates. The following are differences from fereg's saved outputs,
after removing the observation-weighted additive normalization from each FE
channel; they are not errors against a full-panel exact oracle. Worker and firm
RMSE give each retained level equal weight. All entries are in log daily wage
units.

| Panel | Comparator | Fitted-value RMSE | Worker FE RMSE | Firm FE RMSE | Maximum FE difference, either channel |
| --- | --- | ---: | ---: | ---: | ---: |
| Long | fixest default | 2.59e-7 | 3.92e-6 | 1.11e-6 | 2.36e-4 |
| Long | fixest strict | 2.62e-10 | 3.63e-9 | 1.45e-9 | 4.03e-7 |
| Long | reghdfe | 6.78e-6 | 8.60e-5 | 3.27e-5 | 5.06e-3 |
| Short | fixest default | 1.04e-6 | 2.17e-5 | 4.29e-6 | 6.87e-4 |
| Short | fixest strict | 3.12e-9 | 6.53e-8 | 1.36e-8 | 2.02e-6 |
| Short | reghdfe | 5.24e-6 | 1.09e-4 | 2.21e-5 | 3.16e-3 |

Slope and standard-error differences are negligible at this diagnostic point:
the maximum absolute slope difference across references is `1.05e-9` on the
long panel and `6.02e-11` on the short panel; maximum relative standard-error
differences are below `3e-10`. Reported RSS differs by at most `4.80e-12`
(long) and `1.29e-10` (short), relative to fereg. Reconstructing RSS from
reghdfe's saved predictor instead gives relative differences of `3.29e-9` on
both panels. Good slopes or RSS do not ensure equally precise individual FEs.

The normalized model scores explain the default capture failures:

| Panel | fereg | fixest default | fixest strict | reghdfe |
| --- | ---: | ---: | ---: | ---: |
| Long | 1.14e-13 | 1.22e-7 | 1.72e-10 | 2.68e-6 |
| Short | 9.79e-15 | 1.49e-7 | 4.37e-10 | 2.88e-6 |

The frozen score bound is `1e-7`: default fixest exceeds it by only 1.22x/1.49x;
reghdfe by 26.8x/28.8x. Strict fixest passes the score bound but still differs
in individual centered FE coordinates. Its long-panel FE tolerance failures
are sparse (580 of 1,106,419 workers and 15 of 86,647 firms); short-panel failures
are more widespread (66,056 of 659,997 workers and 5,642 of 42,489 firms), despite
the small absolute errors. Only 14 short-panel fitted values exceed the frozen
cross-package tolerance for strict fixest. No gates were relaxed to reflect
these magnitudes, and this diagnostic does not measure downstream fevc
variance-component differences.

The [aggregate diagnostic](../bench/results/scc-20261024-veneto-accuracy.json)
retains RMS, 99th-percentile and maximum differences, affected coordinate counts,
scores, source/input hashes and accounting. Its maintained
[reader](../bench/panel_accuracy.py) leaves all row-level values on SCC.

The [complete results table](../bench/results/scc-20261024-veneto-all-results.csv)
extends these measurements to all 36 package/core configurations and all 108
planned positions. Timings retain their original repetition values, medians,
ranges, failures and skips. Numerical columns give the worst discrepancy over
every completed repetition, using the corresponding same-core native reference
and the same observation-weighted FE centering. All completed reference captures
are inspected separately. Native repetitions reuse the first native diagnostic
only after exact original capture and reported-RSS equality has been verified.
The [full aggregate accuracy evidence](../bench/results/scc-20261024-veneto-all-accuracy.json)
retains every planned position, including skipped ones; no models were refitted
and the original eligibility decisions remain unchanged.

## Frozen identity and evidence

Campaign `20261024T195706Z-veneto-5c0a9240` uses execution source
`5c0a92404f3b5da8b76cde27a857b2e29a29d6da` and the already qualified packaged
Linux runtime from `1ac7125bc637e99c4655c065828559dac593ab01`. The plugin SHA-256
is `477691209f25364ed64959f15036b41059a81479d12af75b6d24667712f90beb`.
Later collector/report changes do not replace or rebuild the measured runtime.
Preparation follows fevc source `1e5acf429a40a825d2ddf499d468bb2ba161fbb9` and
the recorded original KSS graph procedures.

| Retained aggregate evidence | Contents |
| --- | --- |
| [Configuration summary](../bench/results/scc-20261024-veneto-summary.csv) | All 36 arm cells, eligibility, medians/ranges, failed timings, hosts and sampled memory. |
| [Observations](../bench/results/scc-20261024-veneto-observations.csv) | All 108 planned positions, failures, skips and available command times. |
| [Scientific comparisons](../bench/results/scc-20261024-veneto-science.json) | All native repeat/core checks and observed cross-package comparison decisions. |
| [Accuracy magnitudes](../bench/results/scc-20261024-veneto-accuracy.json) | Post-campaign four-core first-repetition diagnostics: fitted values, centered FE channels, slopes, covariance, RSS and independent scores; no refitting. |
| [Complete table with discrepancies](../bench/results/scc-20261024-veneto-all-results.csv) | Every package/core cell, all original repetition times/statuses, and worst numerical discrepancies across completed captures. |
| [All-core accuracy evidence](../bench/results/scc-20261024-veneto-all-accuracy.json) | Every planned position and every completed reference diagnostic at 1/2/4/8/16 cores, with input/source hashes and scheduler/application checks. |
| [Inputs, identity and checks](../bench/results/scc-20261024-veneto-inputs.json) | Input/preparation hashes and sample audits, frozen runtime inventory hash, scheduler records, host replay, native hashes, oracle outcomes and report/evidence hashes. |

Only aggregate evidence is committed. Original administrative records remained
on the Mac; derived model inputs were transferred to owner-only SCC storage with
Johannes's approval. Full row captures, saved FE vectors, observer captures,
compact collection and the visually checked four-page report remain private at
`/projectnb/welfgr/fereg/runs/20261024T195706Z-veneto-5c0a9240/` (report:
`report/veneto-akm.pdf`). The maintained
[collector](../bench/panel_collect.py) and [renderer](../bench/panel_report.py)
reconstruct the aggregate tables and report from this retained evidence.
