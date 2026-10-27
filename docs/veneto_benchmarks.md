# Veneto AKM comparisons

## October 27, 2026: two-FE Auto speed candidate

A development build of the two-FE Auto speed changes (source `18b47675`,
Linux plugin `3f888ffe...`) ran against the beta.7 plugin (`79d81fb1...`) and
PyFixest main `263cf2d3` with within-py 0.4.0 on the audited long and short
panels, reused byte for byte. Each cell had three repetitions with the arm
order rotated, iid VCE and the campaign command clock, on one whole 28-core
node per run (`scc-ud2` without saved FE, `scc-ug1` with saved FE). This is a
descriptive comparison, not a qualification; see the
[record](../bench/results/two-fe-auto-veneto-20261027.json).

Without saved FE (seconds, medians; PyFixest/fereg above 1 means fereg was faster):

| Panel, workers | fereg candidate | beta.7 | PyFixest `within` | PyFixest/fereg |
| --- | ---: | ---: | ---: | ---: |
| short, 1 | 2.01 | 2.49 | 2.08 | 1.04 |
| short, 2 | 1.39 | 2.21 | 1.35 | 0.97 |
| short, 4 | 1.03 | 1.85 | 1.08 | 1.04 |
| short, 8 | 0.82 | 1.66 | 0.94 | 1.14 |
| short, 16 | 0.78 | 1.61 | 0.96 | 1.23 |
| long, 1 | 10.53 | 13.32 | 26.94 | 2.56 |
| long, 2 | 7.45 | 10.59 | 15.40 | 2.07 |
| long, 4 | 5.28 | 8.54 | 9.84 | 1.87 |
| long, 8 | 4.00 | 7.37 | 7.02 | 1.76 |
| long, 16 | 3.62 | 7.08 | 6.23 | 1.72 |

With both FE channels saved the candidate took 3.24-1.10 seconds on the short
panel and 14.61-4.98 on the long one from one to 16 workers, 1.06-2.18 times
faster than beta.7; PyFixest's saved effects exceed the command limit and were
omitted. All fixed-worker repeats were exact, and every cross-worker, beta.7 and
PyFixest comparison (coefficients, covariance, objective and centered saved
effects) passed the campaign tolerances.

## October 27, 2026: long panel rerun on one node

The October 26 long-panel runs landed on different nodes, one of them
(`scc-uf4`) about twice as slow as the others, so the saved and unsaved long
results were not comparable with each other. Both long-panel variants were
therefore rerun back to back in one whole-node job on `scc-uc2` (28-core E5-2680
v4), using the same beta.7 runtime (plugin `79d81fb1...`, built at `277c5d11`),
the same arms, and the October 24 run's audited inputs, smoke witnesses and
policy reused byte for byte. Only the long panel was scheduled: 5,163,428 rows,
1,106,419 workers, 86,647 firms and seven controls. Each variant has 84 planned
positions. All 30 native fits pass the production gates, with exact
fixed-worker repeats and cross-core agreement.

**Without saved FE**, 22 of 23 comparator cells are eligible; the other is
PyFixest 0.60.0 at one core, which hit the 600-second command limit. Entries
are three-command medians in seconds, with the comparator/fereg ratio in
parentheses.

| Cores | fereg s | PyFixest within | fixest default | fixest strict | reghdfe | PyFixest 0.60 |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 13.32 | 26.89 (2.02) | 59.89 (4.49) | 111.49 (8.37) | 503.16 (37.76) | timeout |
| 2 | 10.54 | 15.30 (1.45) | 34.71 (3.29) | 63.99 (6.07) | 288.72 (27.41) | 345.22 (32.77) |
| 4 | 8.56 | 9.82 (1.15) | 20.82 (2.43) | 36.98 (4.32) | 203.77 (23.80) | 188.52 (22.02) |
| 8 | 7.41 | 7.03 (0.95) | 13.03 (1.76) | 22.68 (3.06) | — | 124.21 (16.76) |
| 16 | 7.03 | 6.30 (0.90) | 13.42 (1.91) | 22.73 (3.23) | — | 128.59 (18.30) |

Fereg is faster than both fixest profiles, reghdfe and PyFixest 0.60.0 in
every eligible cell. PyFixest's `within` default is faster at 8 and 16 cores
(0.95, 0.90), fereg at one to four cores (1.15-2.02), the same crossover as
on October 26's `scc-uf4` run. Fereg's absolute times are 2.2 to 2.3 times
shorter on `scc-uc2`. Peak estimator memory was 1.38-1.47 GiB for
fereg, 2.0-2.4 GiB for fixest default, 2.3-2.5 GiB strict, 2.1 GiB reghdfe,
2.2-2.8 GiB PyFixest 0.60.0 and 2.6-3.6 GiB PyFixest `within`.

**With saved FE**, fereg took 16.27 seconds at one core and 9.64 at sixteen,
22-37% longer than without saved FE on the same node, with peak memory of
1.68-1.77 GiB. No comparator cell is eligible, so comparator times are
descriptive (`*`; n=1 is a single completed command before later repetitions
were skipped):

| Cores | fereg s | fixest default | fixest strict | reghdfe | Both PyFixest arms |
| ---: | ---: | ---: | ---: | ---: | --- |
| 1 | 16.27 | 63.68* n=1 (3.91) | 115.00* (7.07) | 559.05* n=1 (34.35) | timeout |
| 2 | 13.14 | 38.99* n=1 (2.97) | 67.58* (5.14) | 326.78* n=1 (24.87) | timeout |
| 4 | 11.10 | 24.91* n=1 (2.24) | 40.42* (3.64) | 239.39* n=1 (21.58) | timeout |
| 8 | 9.79 | 17.20* n=1 (1.76) | 26.06* (2.66) | — | timeout |
| 16 | 9.64 | 17.45* n=1 (1.81) | 26.56* (2.76) | — | timeout |

As on October 24 and 26, default fixest and reghdfe fail the independent
saved-predictor score check; strict fixest completes, but its centered worker
and firm effects differ from fereg's beyond the frozen tolerance; and both
PyFixest arms reach the 600-second command limit at every core count, because
they recover saved effects with single-threaded LSQR (see the route
diagnostic below). Fereg's saved-FE times on `scc-uc2` match October 26's
`scc-ue1` run within 0.8 seconds.

**Identity.** Unsaved run `20261027T021257Z-beta7-veneto-long-nofe-9bbd2649`:
reuse `8294346`, setup `8294347`, smoke `8294348`. Saved run
`20261027T021257Z-beta7-veneto-long-saved-9bbd2649`: reuse `8294349`, setup
`8294350`, smoke `8294351`. Both production runs share pair job `8294352`
(`bench/scc/panel_pair.sh`), which ran the unsaved variant first. The
unsaved run's smoke passed for all six fits; in the saved run's smoke, default
fixest again failed the long-panel score check. All scheduler accounting,
sentinels, host reservations and censuses pass. A first submission from
`ee60ff55` failed its smokes before any timing because the new controls suffix
changed the panel dataset key; `9bbd2649` fixed that.

| Retained aggregate evidence | Contents |
| --- | --- |
| Without saved FE: [summary](../bench/results/scc-20261027-beta7-veneto-long-nofe-summary.csv), [observations](../bench/results/scc-20261027-beta7-veneto-long-nofe-observations.csv), [science](../bench/results/scc-20261027-beta7-veneto-long-nofe-science.json), [inputs](../bench/results/scc-20261027-beta7-veneto-long-nofe-inputs.json) | All 28 arm cells and 84 positions, native repeat/core checks, identities, accounting and host replay. |
| Saved FE: [summary](../bench/results/scc-20261027-beta7-veneto-long-saved-summary.csv), [observations](../bench/results/scc-20261027-beta7-veneto-long-saved-observations.csv), [science](../bench/results/scc-20261027-beta7-veneto-long-saved-science.json), [inputs](../bench/results/scc-20261027-beta7-veneto-long-saved-inputs.json) | The same for the saved run. |

Full captures and the checked three-page reports remain private under the two
run roots in `/projectnb/welfgr/fereg/runs/`.

## October 26, 2026: beta.7 with PyFixest, with and without saved FE

The same audited long and short fevc panels ran on the v0.5.0-beta.7 packaged
Linux runtime (plugin `79d81fb1...`, built at `277c5d11`), first with both FE
channels saved inside command timing, as on October 24, and then without saved
FE. Arms are fereg, fixest 0.14.2 default and strict, PyFixest 0.60.0 (default
Rust MAP), PyFixest main `263cf2d3` with within-py 0.4.0 (default `within`
LSMR), and reghdfe at 1/2/4 cores. Each run has 168 planned positions: three
repetitions at 1/2/4/8/16 cores per panel. Both runs reuse the October 24 run's
audited inputs, smoke witnesses and policy byte for byte
(`panel_campaign.py reuse`); only the arm lists and the saved-FE request
differ. All 60 native fits pass the production gates, with exact fixed-worker
repeats and cross-core agreement.

**Without saved FE**, the comparison isolates estimation and inference. 42 of
46 comparator cells are eligible; the other four are reghdfe and PyFixest
0.60.0 hitting the 600-second command limit at one and two cores on the long
panel. Entries are three-command medians in seconds, with the paired
comparator/fereg ratio in parentheses (above 1 means fereg was faster).

| Long, 5,163,428 rows (`scc-uf4`) | fereg s | PyFixest within | fixest default | fixest strict | reghdfe | PyFixest 0.60 |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 30.99 | 58.43 (1.88) | 137.06 (4.42) | 256.61 (8.28) | timeout | timeout |
| 2 | 23.45 | 33.02 (1.41) | 75.86 (3.23) | 139.84 (5.97) | timeout | timeout |
| 4 | 18.88 | 20.52 (1.09) | 42.95 (2.28) | 76.75 (4.07) | 450.54 (23.89) | 364.71 (19.36) |
| 8 | 16.32 | 14.01 (0.86) | 25.50 (1.56) | 44.72 (2.74) | — | 221.01 (13.47) |
| 16 | 15.63 | 11.74 (0.76) | 25.93 (1.66) | 47.08 (3.04) | — | 227.42 (14.51) |

| Short, 1,319,994 rows (`scc-ui4`) | fereg s | PyFixest within | fixest default | fixest strict | reghdfe | PyFixest 0.60 |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 2.48 | 2.05 (0.83) | 5.48 (2.21) | 11.00 (4.44) | 42.97 (17.30) | 70.95 (28.56) |
| 2 | 2.18 | 1.34 (0.62) | 3.46 (1.59) | 6.70 (3.07) | 31.41 (14.39) | 49.02 (22.30) |
| 4 | 1.86 | 1.08 (0.58) | 3.59 (1.92) | 6.91 (3.70) | 26.12 (14.06) | 50.28 (26.98) |
| 8 | 1.65 | 0.94 (0.57) | 3.55 (2.16) | 6.83 (4.15) | — | 49.91 (30.32) |
| 16 | 1.61 | 0.94 (0.59) | 3.75 (2.33) | 7.02 (4.37) | — | 47.64 (29.65) |

Fereg is faster than both fixest profiles, reghdfe and PyFixest 0.60.0 in
every eligible cell. PyFixest's `within` default is faster than fereg on the
short panel at every core count (0.57-0.83) and on the long panel at 8 and 16
cores (0.86, 0.76); fereg is faster on the long panel at one to four cores
(1.09-1.88).

**With saved FE**, as on October 24, no comparator cell is eligible, so
comparator times are descriptive (`*`; n=1 is a single completed command
before later repetitions were skipped):

| Long (`scc-ue1`) | fereg s | fixest default | fixest strict | reghdfe | Both PyFixest arms |
| ---: | ---: | ---: | ---: | ---: | --- |
| 1 | 16.31 | 64.47* n=1 | 115.89* | 571.43* n=1 | timeout |
| 2 | 13.20 | 39.09* n=1 | 67.19* | 327.15* n=1 | timeout |
| 4 | 11.09 | 24.91* n=1 | 40.50* | 236.66* n=1 | timeout |
| 8 | 9.93 | 17.17* n=1 | 26.09* | — | timeout |
| 16 | 10.40 | 17.50* n=1 | 27.02* | — | timeout |

| Short (`scc-uf3`) | fereg s | fixest default | fixest strict | reghdfe | Both PyFixest arms |
| ---: | ---: | ---: | ---: | ---: | --- |
| 1 | 8.19 | 18.10* n=1 | 30.66* | 159.58* n=1 | timeout |
| 2 | 6.99 | 12.89* n=1 | 19.49* | 112.23* n=1 | timeout |
| 4 | 5.99 | 12.88* n=1 | 19.51* | 85.60* n=1 | timeout |
| 8 | 5.49 | 12.91* n=1 | 19.25* | — | timeout |
| 16 | 5.33 | 13.36* n=1 | 20.12* | — | timeout |

Default fixest and reghdfe fail the independent saved-predictor score check;
strict fixest completes but its centered worker and firm effects differ from
fereg's beyond the frozen tolerance on both panels (and its short-panel saved
prediction too). Both PyFixest arms hit the 600-second command limit at every
core count. A separate diagnostic on the short panel at four threads (below)
shows why: PyFixest recovers saved effects with scipy's single-threaded LSQR,
which took 224-329 seconds at PyFixest's default 1e-6 tolerance and 439-532
seconds at the campaign's 1e-11, against 4-71 seconds for estimation.

**Hosts.** All four production tasks used whole 28-core E5-2680 v4 nodes, yet
`scc-uf3` and `scc-uf4` ran every package about 2.1-2.4 times slower than
`scc-ui4`/`scc-ue1` and the October 24 nodes: on `scc-uf3` the short saved-FE
fereg command took 8.19 seconds at one core against 3.44 on October 24, and
reghdfe, default and strict fixest slowed by the same factor, as did fereg's
non-numerical input copy. Paired ratios within a node remain comparable; absolute
times across the saved and unsaved runs, or across dates, are not.

**Route diagnostic.** On the short panel without saved FE, one Xeon Gold 6242
node ran each fereg solver route and PyFixest `within` three times at four
threads in one session; all routes agree on the first coefficient to about
1e-13 relative and on RSS to about 1e-15.

| Route | Median seconds |
| --- | ---: |
| PyFixest `within` (main) | 1.00 |
| fereg `solver(cmgschur)` | 1.36 |
| fereg `solver(lsmr)` | 1.45 |
| fereg `solver(auto)`, resolving to `cmgschur` | 1.80 |
| fereg `solver(cmg)` | 67.7 |
| fereg `solver(map)` | 270.5 |

With its diagonal Schur-PCG pilot before the CMG upgrade, Auto took about 30%
longer than building CMG at once (`cmgschur`). Explicit `solver(cmg)` (fused-FWL CMG) was about
50 times slower than `cmgschur` here and on the long panel. A long-panel run on
an older E5-2650 v2 node with an 8-slot, 64 GB allocation was cancelled before
`map` and PyFixest; there, explicit `solver(lsmr)` returned `r(909)` (LSMR
workspace exceeds the joint execution plan) while Auto completed. These are
single-session descriptive timings.

**Smokes and identity.** The unsaved run's real-data smoke passed for all 12
fits against the dense oracle. In the saved run's smoke, default fixest failed
the long-panel score check, as on October 24, and PyFixest 0.60.0 failed in the
harness's FE mapping on the short smoke panel, which has a single firm; the fix
(`f2b8865f`) followed, and the production panels with 42,489 and 86,647 firms
were unaffected. Saved run `20261026T184833Z-beta7-veneto-a4bd2cce`: setup
`8289450`, smoke `8289451`, production `8289452`. Unsaved run
`20261026T203913Z-beta7-veneto-nofe-5c1957cc`: setup `8290777`, smoke
`8290778`, production `8290779`. All scheduler accounting, sentinels, host
reservations and censuses pass.

| Retained aggregate evidence | Contents |
| --- | --- |
| Saved FE: [summary](../bench/results/scc-20261026-beta7-veneto-summary.csv), [observations](../bench/results/scc-20261026-beta7-veneto-observations.csv), [science](../bench/results/scc-20261026-beta7-veneto-science.json), [inputs](../bench/results/scc-20261026-beta7-veneto-inputs.json) | All 56 arm cells and 168 positions, native repeat/core checks, identities, accounting and host replay. |
| Without saved FE: [summary](../bench/results/scc-20261026-beta7-veneto-nofe-summary.csv), [observations](../bench/results/scc-20261026-beta7-veneto-nofe-observations.csv), [science](../bench/results/scc-20261026-beta7-veneto-nofe-science.json), [inputs](../bench/results/scc-20261026-beta7-veneto-nofe-inputs.json) | The same for the unsaved run. |
| [Route and PyFixest phase diagnostics](../bench/results/veneto-routes-20261026.json) | Aggregate route timings, return codes, hosts and PyFixest fit/recovery phases. |

Full captures and the checked four-page reports remain private under the two
run roots in `/projectnb/welfgr/fereg/runs/`.

## October 24, 2026: first comparison

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

### Models and retained samples

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

### Core scaling and observed reference times

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

### Validation and limits

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

### Magnitudes of the accuracy differences

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

### Frozen identity and evidence

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
