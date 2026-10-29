# Comparing fereg with fixest

The current published beta.10 synthetic and [Veneto AKM comparisons](veneto_benchmarks.md)
use the fixed beta.8 schedule and references. Earlier campaigns are retained below.

## October 28, 2026: published beta.10 campaign

Run `20261028T200905Z-beta10-campaign-7c5d9f39` measured the published
v0.5.0-beta.10 Linux plugin `a7964116...` (runtime source `3348fe45`, harness
`7c5d9f39`) with the same inputs, comparator pins, tolerances and repetition
schedule as beta.8 below. It covers 50 substantive configurations plus six
per-host reference cells: 1,448 positions, with five repetitions through one
million rows and three above. The comparators are fixest 0.14.2, PyFixest main
`0f41fffd` with within-py 0.4.0, FixedEffectModels.jl 2.1.0 / Julia 1.11.7,
reghdfe, ppmlhdfe and Stata 19 areg; strict variants retain their frozen
settings.

All **256 fereg fits passed**, including all 56 fixed-worker cell checks and
14 cross-worker groups. The 11 native independent smoke witnesses passed.
All production accounting, application sentinels, frozen runtime inventories,
fit coverage and host-observation replay passed. Each of the three SCC setup
runs passed 421 harness tests. The campaign retains 1,333 successful captures,
15 capture errors, 8 command timeouts, 86 positions skipped after failure and
6 larger-size positions skipped after timeout. Comparator smoke discrepancies
are recorded in the inputs evidence; a smoke pass does not replace per-fit
scientific checks.

Six whole 28-core allocations ran on `scc-ul2`, `scc-xa1`, `scc-xc3`, `scc-xa2`,
`scc-xc1` and `scc-xc4`. Every observed CPU was an Intel Xeon E5-2680 v4,
but no CPU-model restriction was requested. The native reference median was
0.924–0.943 seconds on five hosts and 1.668 seconds on `scc-xc1` (the dense
three-FE bundle). That slower observation is retained. Compare packages within
the same configuration and host; absolute times across bundles, or against
beta.8 on different hosts, do not identify a software speedup.

**Agreeing package comparisons.** Ratios below divide each comparator's
median command time by fereg's median on the same host. Agreement includes
results differing only by the absorbed-degrees-of-freedom covariance scalar;
all other discrepancies remain descriptive.

| Comparator | Model | Agreeing cells | fereg faster | Ratio range | Median ratio |
| --- | --- | ---: | ---: | ---: | ---: |
| reghdfe | OLS | 19 | 19 | 4.54–30.84 | 15.53 |
| ppmlhdfe | PPML | 13 | 13 | 7.53–78.17 | 19.12 |
| areg | OLS | 15 | 15 | 1.83–96.39 | 8.85 |
| fixest | OLS | 23 | 22 | 0.97–6.44 | 2.23 |
| fixest | PPML | 19 | 19 | 1.01–8.69 | 1.88 |
| fixest strict | OLS | 24 | 24 | 1.19–23.81 | 3.14 |
| fixest strict | PPML | 6 | 6 | 2.04–3.66 | 2.92 |
| PyFixest within | OLS | 24 | 18 | 0.64–4.29 | 1.58 |
| PyFixest within | PPML | 19 | 13 | 0.25–2.74 | 2.12 |
| FEM.jl | OLS | 17 | 14 | 0.55–2.30 | 1.61 |
| FEM.jl strict | OLS | 24 | 21 | 0.57–12.81 | 2.79 |

At one million rows and four workers, AKM OLS takes 0.933 seconds in fereg,
2.028 in fixest, 2.229 in PyFixest, 1.633 in FEM.jl (agreement by convention),
14.711 in reghdfe and 8.965 in areg. AKM PPML takes 4.228 seconds in fereg,
7.930 in fixest, 9.741 in PyFixest and 83.701 in ppmlhdfe.
The Stata comparators are slower in every agreeing cell. Default fixest is
slightly faster in dense three-FE OLS (1.955 versus 2.017 seconds); PyFixest
and FEM.jl win several designs. FEM.jl is faster in one-FE and dense two-/three-FE
OLS, but its default tolerance misses the sparse-design accuracy checks.
Saved-output discrepancies and covariance conventions are marked per cell in
the retained summary.

**Ten million rows.** AKM OLS takes 9.25 seconds in fereg, and AKM PPML 59.74
seconds; the latter reaches the 600-second limit in ppmlhdfe. Sparse three-FE
OLS now completes all three native repetitions, at 52.94–54.82 seconds
(median **53.18**), with exact repeats and scientific agreement across workers.
PyFixest takes 55.01 seconds, fixest 336.70 and strict FEM.jl 341.77, all agreeing
up to degrees-of-freedom conventions. Default FEM.jl takes 103.27 seconds but
misses the science checks; reghdfe and strict fixest time out, and areg is
skipped after its smaller-size timeout. No failed timing was retried. The old
beta.8 timeout remains historical evidence below; this campaign establishes
completion for beta.10, not an absolute speedup across hosts.

**Memory limits and tradeoffs.** Ten-million-row kernel peaks, including loaded
data, are 5.71 GiB for native AKM OLS and 12.40 GiB for native AKM PPML. Sparse
three-FE OLS uses 12.50 GiB, against 12.71 for PyFixest, 7.19 for fixest and 4.37
for strict FEM.jl. At three million rows fereg still uses more memory than
those completing comparators (4.54 GiB versus 4.27 for PyFixest). PyFixest is
faster through three million rows; at ten million fereg's median is 3.3% lower.
The 28-slot requests reserve 3 GiB per core; these results do not establish
behavior under a smaller memory budget. See the companion
[Veneto beta.10 results](veneto_benchmarks.md#october-28-2026-published-beta10-long-panel-campaign)
for the real-data comparison.

**Operational repair.** Original array 8320662 failed before any timed fit
because the retained beta.8 bundles contained several identical copies of the
reference input. Repair 8320984 verified all 28 frozen inputs, every duplicate
hash/metadata record and every decoded column, then supplied unique canonical
paths. Replacement array 8320989 ran once. Only the frozen-input path inventory
entry changed; original failed accounting, old inventories and the repair hashes
remain in the [completion record](../bench/results/scc-20261028-beta10-completion.json).

Retained evidence: [summary](../bench/results/scc-20261028-beta10-summary.csv),
[all positions](../bench/results/scc-20261028-beta10-observations.csv), and
[inputs, science, hosts and hashes](../bench/results/scc-20261028-beta10-inputs.json).
This is descriptive measurement of an already-qualified release, with no
scientific tolerance changes, timing exclusions or resampling.

## October 27, 2026: beta.8 campaign with FixedEffectModels.jl and size series

Run `20261027T165902Z-beta8-campaign-670632aa` (harness `670632aa`) measured
the published v0.5.0-beta.8 package, Linux plugin `3f888ffe...` (build
`18b47675`), against fixest 0.14.2 (default and strict), PyFixest main
`0f41fffd` with within-py 0.4.0, FixedEffectModels.jl 2.1.0 on Julia 1.11.7 at
its default tolerance (1e-6) and at 1e-10, reghdfe, ppmlhdfe and Stata 19
areg. At one million rows and four cores it covers the AKM design with every
covariance type and saved FE, 4 and 64 controls, and one-FE and dense/sparse
two-/three-FE designs; AKM core series run at 1/2/4/8/16 cores, and size series
from 100,000 to ten million rows cover AKM OLS, AKM PPML and sparse three-FE
OLS. Each model and design ran on one whole 28-core E5-2680 v4 node (`scc-xe4`,
`scc-wk4`, `scc-wk3`, `scc-xe1`, `scc-xa2`, `scc-xf3`), with five repetitions up
to one million rows and three above. The same native reference cell took
1.015-1.047 seconds on every node. Memory is the kernel peak resident set of
the timed command, including the loaded data.

Production task 1 first started on a Xeon Gold 6132 node. It was stopped after
25 positions, before its timings were inspected, and resubmitted as job
8302543 pinned to E5-2680 v4; the stopped attempt is retained on SCC. Of the
1,448 planned positions, 1,330 succeeded, 15 completed with a capture error
(saved predictions that fail the model-score check), 9 reached the 600-second
limit, 88 later repetitions were skipped after a failure and 6 larger sizes
after a timeout. Results that differ only in counting absorbed degrees of
freedom agree "by convention": their covariance matches after one scalar
within 1% of one.

**Speed against fereg** (comparator median over fereg median in configurations
whose results agree, exactly or by convention):

| Comparator | Model | Agreeing | fereg faster | Ratio range | Median |
| --- | --- | ---: | ---: | ---: | ---: |
| reghdfe | OLS | 19 | 19 | 4.13-48.1 | 13.6 |
| ppmlhdfe | PPML | 13 | 13 | 7.47-77.7 | 19.0 |
| areg | OLS | 15 | 15 | 1.68-102 | 7.78 |
| fixest | OLS | 22 | 22 | 1.19-5.68 | 2.05 |
| fixest | PPML | 19 | 19 | 1.12-8.65 | 1.87 |
| PyFixest within | OLS | 23 | 16 | 0.61-3.75 | 1.51 |
| PyFixest within | PPML | 19 | 13 | 0.46-2.72 | 2.09 |
| FixedEffectModels.jl | OLS | 17 | 13 | 0.52-2.20 | 1.52 |
| FixedEffectModels.jl strict | OLS | 23 | 20 | 0.52-11.5 | 2.67 |

In the one-million-row AKM design fereg took 1.01 seconds (OLS) and 4.24
(PPML). reghdfe took 14.6 times as long, areg 8.9, fixest 2.0, PyFixest within
2.2 and FixedEffectModels.jl 1.6 (2.5 strict); ppmlhdfe took 19.7 times as long,
fixest 1.9 and PyFixest within 2.3. PyFixest within was faster in the sparse
three-FE OLS and PPML designs (0.46-0.74), the dense three-FE designs (0.87-0.89),
dense two-FE PPML (0.90-0.97) and with one FE (0.96-0.99); FixedEffectModels.jl was faster with one FE (0.52), in
the dense three-FE design (0.74), with 64 controls (0.89) and about equal in
the dense two-FE design (0.98). At its default tolerance FixedEffectModels.jl
differs numerically in the sparse designs; with 1e-10 it agrees and took
6.0-11.5 times as long. With saved FE, every comparator's saved predictions
miss the 1e-7 accuracy check except strict fixest.

**Cores.** From one to sixteen cores fereg's AKM time fell from 2.09 to 0.72
seconds (OLS) and from 8.99 to 3.11 (PPML). FixedEffectModels.jl gains less
from cores: 1.13 times fereg's time with one core and 1.95 with sixteen. On one
core reghdfe took 13.6 and ppmlhdfe 22.7 times as long as fereg.

**Size.** In AKM OLS fereg took 0.17 seconds at 100,000 rows and 10.6 at ten
million; reghdfe's ratio rose from 7.6 to 22.0, areg's from 4.3 to 11.0 and
PyFixest within's from 1.2 to 3.8, while fixest (1.6-2.3) and
FixedEffectModels.jl (1.6-2.2) stayed about constant. At ten million rows the
peak memory was 5.64 GiB for fereg, 7.18 reghdfe, 10.32 areg, 6.68 fixest,
11.35 PyFixest within and 4.18 FixedEffectModels.jl. In AKM PPML fereg took
60.8 seconds at ten million rows, where ppmlhdfe did not finish within the
limit; fixest took 2.0 and PyFixest within 2.7 times as long, with peaks of
12.41 GiB (fereg), 12.62 and 16.32. In sparse three-FE OLS fereg took 0.53 to
15.5 seconds from 100,000 to three million rows, PyFixest within 0.61-0.74
times as long, and fereg needed more memory than every comparator that finished:
5.77 GiB at three million rows against 4.37 (PyFixest within), 2.61 (reghdfe),
2.31 (fixest) and 1.83 (FixedEffectModels.jl).

**Ten-million-row sparse three-FE limit.** At ten million rows fereg itself
reached the 600-second limit in sparse three-FE OLS, while PyFixest within
(55 seconds), FixedEffectModels.jl (105 seconds at its default tolerance) and
fixest (338 seconds) finished; those comparator results could not be checked
against fereg. A separate diagnostic outside the campaign refitted the same
input with `threads(4)` and `memory(fast)` on two other nodes (a whole E5-2680 v4
node and 16 slots of another node); neither fit finished within 70 minutes. The
processes stayed compute-bound at about 3.5 cores and 5.8 GB of virtual memory,
so neither the low-memory plan nor the node explains the limit; at three
million rows the same design took 15.5 seconds. The campaign timing stays as
recorded. The cause, diagnosed afterwards, was an overstated additive-Schwarz
memory bound that sent the fit to the MAP fallback; see
[VALIDATION](VALIDATION.md#many-fe-schwarz-admission-at-scale). The beta.10
candidate reran only this size series under a bounded protocol (one whole
28-core node, three repetitions, 30 minutes per fit): it finishes ten million
rows in 59 seconds with 12.6 GiB, 7M in 39 and 5M in 28, where beta.9 exhausts
the bound at 5M and 7M, and it is 7-13% faster than beta.9 from 100,000 to
three million rows ([round record](../bench/results/beta10-2026-10-28.json)).
Those numbers come from a different node and are not campaign timings.

**Controls.** fereg's OLS time rose from 0.54 seconds with 4 controls to 5.44
with 64, more steeply than the comparators': at 64 controls reghdfe took 11.2,
areg 5.0, fixest 1.19, PyFixest within 1.51 and FixedEffectModels.jl 0.89 times
as long. In PPML the gap to ppmlhdfe widened from 9.6 (4 controls) to 30.4 (64).

## October 27, 2026: beta.7 supplement with multi-FE areg and control counts

Stata 19's `areg` absorbs several effects; the beta.7 rerun below had restricted
it to one-FE cells. Supplement `20261027T013651Z-beta7-supplement-ee60ff55`
(source `ee60ff55`, the same v0.5.0-beta.7 runtime identity `9713d640...`,
SCC-built plugin `74812e13...`) pairs areg with fereg in all 18 multi-FE OLS
configurations with at most four workers and no saved FE, using `dfabsorb
noabsorbtest tolerance(1e-8) iterate(16000)` as in the August comparison. It
also fits the AKM design at one million rows and four workers with 4 and 64
controls instead of 16, for every arm. Each comparator is paired with fereg on
the same node; the three bundles ran on whole 28-core E5-2680 v4 nodes
(`scc-wk4`, `scc-tr4`, `scc-tj4`). The 54 fereg captures that repeat main-run
cells are bitwise identical to the main run's.

**areg.** Where its results agree with fereg's, areg took 2.69-12.89 times as
long as fereg (median 7.09; 15 configurations, including the one-core AKM
cell at 7.17). It took 9.19 times as long in the one-million-row AKM design,
12.89 at four million rows and 4.72-5.36 in dense three-FE designs. In the
sparse two- and three-FE designs at one and four million rows it did not finish
within the 600-second limit. With worker-clustered covariance it does not
discount the worker effects nested in the clusters, so that cell (8.73 times)
is descriptive. Its peak memory was 1.07 GiB at one million rows and 4.33 GiB at
four million, against fereg's 0.61 and 2.29 GiB in the main run.

**Controls.** Ratios of median times to fereg's within each cell:

| Model | Controls | fixest | PyFixest within | PyFixest 0.60 | reghdfe / ppmlhdfe | areg |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| OLS | 4 | 1.73 | 1.52 | 4.13 | 12.66 | 12.00 |
| OLS | 16 (main run) | 1.54 | 1.62 | 3.35 | 9.19 | 9.19 |
| OLS | 64 | 1.15 | 1.46 | 3.35 | 10.75 | 4.86 |
| PPML | 4 | 1.28 | 1.58 | 3.98 | 9.63 | — |
| PPML | 16 (main run) | 1.87 | 2.35 | 6.36 | 20.13 | — |
| PPML | 64 | 2.42 | 2.21 | 6.19 | 30.40 | — |

All 66 native fits pass with exact repeats and cross-worker agreement; 57 of 62
arm cells are eligible (the exceptions are areg's clustered cell and four
timeouts). The 16-control row comes from the main run on other nodes, and the
control-count cells ran on different nodes, so compare ratios rather than
absolute times. Compact evidence: [summary](../bench/results/scc-20261027-beta7-supplement-summary.csv),
[observations](../bench/results/scc-20261027-beta7-supplement-observations.csv) and
[inputs and checks](../bench/results/scc-20261027-beta7-supplement-inputs.json).

## October 26, 2026: beta.7 rerun with PyFixest and areg

Fereg is faster in **all 35 scientifically eligible default-fixest
comparisons**: **18/18 OLS and 17/17 PPML**, versus 27/35 in the beta.6 rerun.
The strongest comparator is now PyFixest. Against its pinned main revision,
which defaults to the `within` LSMR demeaner, fereg is faster in **21 of 35**
eligible comparisons (13/18 OLS, 8/17 PPML); against the PyFixest 0.60.0
release, in 22 of 35 (15/18, 7/17). Fereg is faster than `areg` in all three
one-FE OLS configurations (1.12-2.06), and in every eligible strict-fixest
(22/22 OLS, 4/4 PPML), reghdfe (20/20) and ppmlhdfe (17/17) comparison. These
counts describe this fixed panel;
they are not probabilities of winning on an arbitrary model.

Campaign `20261026T184833Z-beta7-fixest-a4bd2cce` measured frozen source
`a4bd2ccea1d09fdd44349fd591598cd3b5a9fc6a`. Its runtime identity is the
v0.5.0-beta.7 package's (`9713d640...`); the SCC-built Linux plugin SHA-256 is
`cc1920c2b4bd258a706bebd1d44f309e703eb0ea75258920ce935dcf30b0e92a`. Datasets,
the 56 configurations, fixest/reghdfe/ppmlhdfe profiles, repetitions,
scientific tolerances, per-fit limits and socket/NUMA placement are unchanged.
New arms:

- **PyFixest 0.60.0**, the current PyPI release, with its default Rust MAP
  demeaner, in every configuration.
- **PyFixest main** at `263cf2d3` with within-py 0.4.0, whose default is the
  `within` LSMR demeaner with adaptive preconditioning. This default is merged
  but not yet in a PyFixest release. It also runs in every configuration.
- **areg** in the three four-worker one-FE OLS configurations. Its absorbed
  count is compared after adding back the intercept it reports separately.
  Stata 19's `areg` also absorbs several effects; restricting it to one-FE cells
  was a harness error, and a supplement adds it to the multi-FE OLS cells.

Both PyFixest arms keep default estimation tolerances, recover saved FE at
1e-11 like the fixest arms, and use locked Linux environments built in the
setup job and hashed into the runtime inventory. The schedule grew from 579 to
924 planned fits in eight bundles with a six-hour walltime.

Selected results are three-command medians in seconds; ratios are the median
**paired comparator/fereg** time within the same node, so values above 1 mean
fereg was faster. Rows use four workers. `*` marks a completed but
scientifically ineligible time; NR means no completed time.

| Model / design | Rows | fereg s | fixest s | fixest / fereg | within s | within / fereg | PyFixest 0.60 s | 0.60 / fereg |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| OLS, AKM | 1,000,000 | 1.73 | 2.67 | 1.54 | 2.81 | 1.62 | 5.81 | 3.31 |
| OLS, AKM | 4,000,000 | 4.03 | 9.16 | 2.28 | 10.79 | 2.70 | 27.52 | 6.89 |
| OLS, one FE | 1,000,000 | 0.74 | 1.01 | 1.36 | 0.76 | 1.01 | 1.06 | 1.42 |
| OLS, two dense FEs | 4,000,000 | 2.67 | 3.71 | 1.39 | 3.37 | 1.27 | 4.79 | 1.80 |
| OLS, three dense FEs | 4,000,000 | 4.48 | 4.55 | 1.02 | 3.98 | 0.89 | 5.34 | 1.19 |
| OLS, three sparse FEs | 4,000,000 | 21.27 | 122.51* | – | 15.58* | – | NR | – |
| PPML, AKM | 1,000,000 | 4.24 | 7.95 | 1.89 | 9.96 | 2.31 | 27.00 | 6.29 |
| PPML, AKM | 4,000,000 | 19.05 | 43.53 | 2.28 | 49.21 | 2.59 | 134.56 | 7.11 |
| PPML, two dense FEs | 4,000,000 | 15.23 | 15.82 | 1.04 | 14.44 | 0.95 | 12.50 | 0.82 |
| PPML, two sparse FEs | 4,000,000 | 27.40 | 195.29* | – | 51.87* | – | NR | – |
| PPML, three dense FEs | 4,000,000 | 17.27 | 21.07 | 1.22 | 15.91 | 0.92 | 13.70 | 0.80 |
| PPML, three sparse FEs | 1,000,000 | 31.46 | 149.42* | – | 18.99* | – | NR | – |

**Against fixest.** All eight beta.6 default-fixest losses are now wins. The
five PPML flips reflect beta.7's PPML two-FE protocol; dense two-FE PPML at four
million rows moved from 0.761 to 1.039. The three OLS flips (one FE at one
million rows and three dense FEs at one and four million) are not runtime
effects: every OLS fit is bit-identical to beta.6, and those cells were within
3% of parity. Default fixest still loses most where graphs are sparse, but
those comparisons stay ineligible because default fixest does not count
disconnected components exactly.

**Against PyFixest.** The `within` default is faster than fereg on dense
three-FE designs (0.64-0.89 OLS, 0.80-0.92 PPML), dense two-FE PPML
(0.90-0.98), one-FE PPML up to one million rows (0.76-0.98) and the
100,000-row one-FE and dense two-FE OLS cells. On sparse three-FE designs its
medians are 0.44-0.87 of fereg's (descriptive). Fereg is faster on AKM designs
(1.28-2.70 OLS, 1.93-2.73 PPML) and large dense two-FE OLS (1.15-1.29); on
sparse two-FE designs the descriptive median ratios are 0.99-2.15. PyFixest
0.60's MAP demeaner is the fastest package on dense two- and three-FE PPML
(0.68-0.88) and one-FE PPML (0.71-0.93), but it failed to converge after
10,000 iterations in four sparse cells and hit the command limit in six more.
With saved FE, PyFixest recovers effects with scipy's LSQR: the within arm took
10.2-10.5 seconds in the four-worker saved-FE AKM OLS cells against fereg's
2.2-2.4, and those cells fail the saved-prediction gate. PyFixest's sparse
cells are ineligible for the same reason as default fixest's: it counts
absorbed degrees of freedom by levels, not exact components, so DoF and the
covariance scale differ.

**Science and coverage.** All 168 native fits pass, repeat exactly at fixed
worker count and agree across workers. Against beta.6, all 168 pass the
sample/discrete, coefficient, covariance and objective checks; all 87 OLS and 9
one-FE PPML fits are bitwise identical, and the 72 multi-FE PPML fits stay
within 1.9e-5 (coefficients) and 2.4e-9 (covariance) of the gate envelopes.
Independent smoke oracles pass for fereg 11/11, strict fixest 10/11, PyFixest
within 8/11, PyFixest 0.60 3/11, default fixest 2/11, reghdfe 1/6 and
ppmlhdfe 1/5. The 924 planned positions have 831 successful captures, 14
capture errors, 13 command timeouts (six PyFixest 0.60), four PyFixest 0.60
demeaning failures and 62
repetitions skipped after a prior failure. Of 308 arm cells, 227 are eligible.
All failed observations are retained; no timing was retried.

Setup `8289446`, smoke `8289447` and production `8289448` tasks 1-8 all have
successful scheduler accounting and application sentinels. Bundles used whole
28-core E5-2680 v4 nodes (`scc-wo4`, `scc-wm1`, `scc-wn2`, `scc-wg3`, `scc-va1`,
`scc-wh4`, `scc-wh1`, `scc-wm2`), took 0.82-2.32 hours and 10.28 node-hours in
total; all nine host reports pass. Same-model nodes still differ by up to about
2.4x (see the [Veneto runs](veneto_benchmarks.md)), so only paired ratios within
a node are compared. This is descriptive Linux evidence from one seed and three
repetitions, with no confidence-qualified or universal speed claim.

| Retained evidence | Contents |
| --- | --- |
| [Configuration summary](../bench/results/scc-20261026-beta7-summary.csv) | All 308 arm cells, medians/ranges, eligibility, paired ratios, hosts and sampled memory. |
| [Observations](../bench/results/scc-20261026-beta7-observations.csv) | All 924 planned positions, failures/skips, process/command times, PyFixest identities and native phase clocks. |
| [Beta.6 comparison](../bench/results/scc-20261026-beta7-comparison.csv) | All 56 default-fixest configurations with both campaigns' hosts, times and ratios, plus the PyFixest and areg cells. |
| [Inputs, identity and checks](../bench/results/scc-20261026-beta7-inputs.json) | Source/plugin/input identities, PyFixest pins and freezes, accounting, host checks, oracles, repeat/worker checks, 168 historical science checks and artifact hashes. |

Raw captures, the compact collection and the checked 29-page PDF remain on SCC
under `/projectnb/welfgr/fereg/runs/20261026T184833Z-beta7-fixest-a4bd2cce/`.
The PDF's provenance table lists the old 3 h 50 min / 4 h stopping limits; the
campaign used 5 h 50 min and 6 h, as `campaign.json` records, and the renderer
now prints the policy's values.

## October 26, 2026: beta.6 rerun

Fereg is faster in **27 of 35 scientifically eligible default-fixest
comparisons**: **15/18 OLS and 12/17 PPML**, versus 29/35 in the beta.3
rerun. The other 21 default comparisons remain ineligible or unavailable.
These counts describe this fixed panel; they are not probabilities of winning
on an arbitrary model. Two of the four bundles ran on markedly slower nodes
than beta.3 (below), so count changes do not isolate runtime effects.

Campaign `20261026T012706Z-beta6-fixest-fe67c26c` measured frozen source
`fe67c26c664cdb6969f28d0fd220395c25293da0`, the v0.5.0-beta.6 runtime with the
many-FE OLS geometry policy active. Its runtime identity matches the package;
the SCC-built Linux plugin SHA-256 is
`acb42877d6b2b7c38fc6642bfaddd2d95979715c95368f61a976a43b3edc49e9`. Datasets,
the 56 configurations, comparators, repetitions, scientific tolerances,
per-fit limits, launchers and socket/NUMA placement are unchanged from the
beta.3 rerun.

Selected results are three-command medians in seconds; the ratio is the
median **paired fixest/fereg** time within the same node. Rows use four
workers unless noted.

| Model / design | Rows | Node | fereg s | fixest s | Paired fixest/fereg |
| --- | ---: | --- | ---: | ---: | ---: |
| OLS, AKM | 1,000,000 | wo4 | 1.761 | 2.675 | 1.533 |
| PPML, AKM | 1,000,000 | wd1 | 12.121 | 18.487 | 1.533 |
| OLS, one FE | 1,000,000 | wo4 | 1.413 | 1.387 | 0.981 |
| OLS, two dense FEs | 4,000,000 | yk1 | 2.471 | 3.404 | 1.378 |
| OLS, three dense FEs | 100,000 | wd1 | 0.352 | 0.431 | 1.224 |
| OLS, three dense FEs | 1,000,000 | wo4 | 2.073 | 2.028 | 0.978 |
| OLS, three dense FEs | 4,000,000 | wd1 | 9.731 | 9.455 | 0.972 |
| PPML, two dense FEs | 1,000,000 | yk1 | 4.299 | 4.071 | 0.944 |
| PPML, two dense FEs, 16 workers | 1,000,000 | yk1 | 3.314 | 3.270 | 0.987 |
| PPML, two dense FEs | 4,000,000 | wd1 | 47.500 | 36.151 | 0.761 |
| PPML, three dense FEs | 1,000,000 | yk4 | 5.268 | 5.200 | 0.990 |
| PPML, three dense FEs | 4,000,000 | yk1 | 20.259 | 18.954 | 0.936 |

The table includes all eight eligible default-fixest losses. Five are within
3%, which three repetitions do not establish reliably. The clear gaps remain
dense two-FE PPML (0.761 at four million rows, 0.944 at one million) and dense
three-FE PPML at four million rows (0.936). Dense three-FE OLS, the geometry
policy's target, moved from 0.730/0.939/0.559 to 1.224/0.978/0.972 at
100,000/1,000,000/4,000,000 rows. At four million rows fixest took 2.07 times
its beta.3 time on the slower node while fereg took 1.19 times. Strict fixest
comparisons give 21/22 OLS and 4/4 PPML native wins. Eligible reghdfe and
ppmlhdfe comparisons give 20/20 and 17/17 native wins.

All **168 fereg fits pass**. Captured coefficients, covariance, objectives and
discrete fields repeat exactly at fixed worker count; cross-worker scientific
and discrete checks pass. All 168 also pass the sample/discrete, coefficient,
covariance and objective checks against their beta.3 counterparts. The 147 fits
outside the seven three-FE OLS configurations are bitwise identical to beta.3.
All 11 independent native smoke oracles pass; reference smoke counts are
unchanged (default fixest 2/11, strict fixest 10/11, reghdfe 1/6, ppmlhdfe 1/5).

The 579 planned positions have the same outcome counts as beta.3: 534
successful captures, eight capture errors, seven command timeouts and 30
repetitions skipped after a prior failure. All failed observations are
retained; no timing was retried. Of 193 arm cells, 154 are eligible, 32 have
descriptive but ineligible timings and seven have no completed timing.
Four-million-row sparse three-FE PPML completes all three native fits (median
162.240 seconds); fixest and ppmlhdfe time out.

Setup `8280462`, smoke `8280463` and production `8280464` tasks 1-4 all have
complete successful scheduler accounting and application sentinels, with no
failed setup attempt. Bundles used whole 28-core nodes `scc-wo4` and `scc-wd1`
(Xeon E5-2680 v4, the scheduler's CPU type for all four beta.3 nodes) and
`scc-yk1` and `scc-yk4` (Xeon Gold 6132). They took 72-210 minutes and 8.11
node-hours. All five smoke/production host reports pass with no known
competing compute, and all 377 benchmark-tooling tests pass on SCC.

Absolute times on `scc-wd1` and `scc-wo4` are not comparable with beta.3.
On `scc-wd1`, configurations with bitwise-unchanged native arithmetic took a
median 2.3 times their beta.3 time, with every native phase slower, including
input handling; fixest took 2.2 times. On `scc-wo4` the factors were 1.8 and
1.4, which lowered fereg's AKM ratios (four-core OLS from 2.10 to 1.53). On the
Gold 6132 nodes both packages took 0.9-1.0 times their beta.3 times. The
campaign does not identify why same-model nodes differ. This is descriptive
Linux evidence from one seed and three repetitions, with no
confidence-qualified release or universal speed claim.

| Retained evidence | Contents |
| --- | --- |
| [Configuration summary](../bench/results/scc-20261026-summary.csv) | All 193 arm cells, medians/ranges, eligibility, paired ratios, hosts and sampled memory. |
| [Observations](../bench/results/scc-20261026-observations.csv) | All 579 planned positions, failures/skips, process/command times and available native phase clocks. |
| [Beta.3 comparison](../bench/results/scc-20261026-comparison.csv) | All 56 default-fixest configurations, both campaigns' hosts/times, eligibility and paired ratios. |
| [Inputs, identity and checks](../bench/results/scc-20261026-inputs.json) | Source/plugin/input identities, accounting, host checks and classes, oracles, repeat/worker checks, 168 historical science checks and artifact hashes. |

Raw captures, compact collection, the checked 21-page PDF and export script
remain on SCC under
`/projectnb/welfgr/fereg/runs/20261026T012706Z-beta6-fixest-fe67c26c/`.

## October 24, 2026: beta.3 rerun

Fereg is faster in **29 of 35 scientifically eligible default-fixest
comparisons**: **15/18 OLS and 14/17 PPML**, versus 26/35 in the October 12
campaign. The other 21 default comparisons remain ineligible or unavailable.
These counts describe this fixed panel, including covariance/output and worker
variants; they are not probabilities of winning on an arbitrary model.

Campaign `20261024T135209Z-beta3-fixest-7a9ac5c4` measured frozen source
`c5582eb089485e36defd38694b0e566bab85f75c`, with the beta.3 computational
runtime selected at submission. Subsequent repository and solver-reporting
edits are outside this measurement. The Linux plugin SHA-256 is
`e0aaf262631cabe49f739b5f0136a74e7af5a67c8b85c024a4d9f58a0cdc5110`.

The rerun uses the exact 36 retained production datasets and the same 56
configurations, 16 controls, three repetitions, comparator profiles, scientific
tolerances and per-fit limits as October 12. Fixest remains 0.14.2; default and
strict profiles stay separate. The primary panel uses four workers, with
selected 1/8/16-worker variants, 100,000/1,000,000/4,000,000 rows, AKM and
one-/two-/three-FE dense/sparse designs, and covariance/saved-FE variants.

Selected results below are separate three-command medians in seconds. The
ratio is the median **paired fixest/fereg** time; above one favors fereg and
need not equal the ratio of separate medians. All rows use four workers.

| Model / design | Rows | fereg s | fixest s | Paired fixest/fereg |
| --- | ---: | ---: | ---: | ---: |
| OLS, AKM | 1,000,000 | 0.988 | 2.079 | 2.095 |
| PPML, AKM | 1,000,000 | 5.157 | 7.883 | 1.529 |
| OLS, two dense FEs | 4,000,000 | 2.654 | 3.713 | 1.401 |
| OLS, three dense FEs | 100,000 | 0.293 | 0.214 | 0.730 |
| OLS, three dense FEs | 1,000,000 | 1.561 | 1.467 | 0.939 |
| OLS, three dense FEs | 4,000,000 | 8.169 | 4.563 | 0.559 |
| PPML, two dense FEs | 1,000,000 | 4.749 | 4.407 | 0.928 |
| PPML, two dense FEs | 4,000,000 | 20.666 | 15.873 | 0.769 |
| PPML, three dense FEs | 4,000,000 | 21.320 | 21.026 | 0.987 |

The table includes all six eligible default-fixest losses. The two largest
remaining gaps are dense three-FE OLS and dense two-FE PPML at four million
rows. Their native ranges are 8.165-8.187 and 20.643-20.761 seconds, versus
fixest's 4.560-4.572 and 15.831-15.904. The three-FE PPML difference is about
1.3%; three repetitions do not establish a reliable small gap. Strict fixest
comparisons give 20/22 OLS and 4/4 PPML native wins.
Eligible reghdfe and ppmlhdfe comparisons give 20/20 and 17/17 native wins.

All **168 fereg fits pass**. Captured coefficients, covariance, objectives and
discrete fields repeat exactly at fixed worker count; cross-worker scientific
and discrete checks pass. All 168 historical native counterparts also agree on
sample/discrete fields and pass the coefficient, covariance and objective
tolerances. That historical check does not compare row-level saved fitted
arrays; each campaign's own requested-output checks remain in force. All 11
independent native smoke oracles pass. Reference smoke passes remain explicit:
default fixest 2/11, strict fixest 10/11, reghdfe 1/6 and ppmlhdfe 1/5.
Production eligibility does not imply every comparator passes every oracle.

There are **579 planned positions**: 534 successful captures, eight capture
errors, seven command timeouts and 30 repetitions skipped after a prior failure.
All failed observations are retained; no timing was retried. Of 193 arm cells,
154 are eligible, 32 have descriptive but ineligible timings and seven have no
completed timing. Four-million-row sparse three-FE PPML completes all three
native fits (median 141.187 seconds); fixest and ppmlhdfe time out, so there is
no eligible speed ratio for that case.

Setup `8269312`, smoke `8269313` and production `8269314` tasks 1-4 all have
complete successful scheduler accounting and application sentinels. The four
bundles used whole 28-core nodes `scc-uh4`, `scc-ue2`, `scc-ui1` and `scc-ui3`,
took 62-114 minutes and consumed 6.43 node-hours in total. Both packages used
the same socket-ordered CPU prefixes and NUMA memory binding. Scheduler
reservation/all-queue checks were replayed from the retained observations;
all five smoke/production host reports pass, with no known competing compute.
Build/ABI/numeric guards and all 355 benchmark-tooling tests pass on SCC.

Two setup attempts failed before campaign estimation: an inherited comparator
metadata filename, then Linux timeout-cleanup and wrapper-test shell defects.
The fixes preserve the benchmark policy and computational runtime. The focused
three-case Linux retest `8269306` passed with successful accounting before the
accepted submission. Failure summaries, identities and logs are retained.

Every corresponding October 12 configuration ran on a different host, and
the full rerun now uses the socket/NUMA placement policy introduced in H5.
Historical timing differences therefore do not isolate runtime improvements
from hardware and placement. This is descriptive Linux evidence from one seed
and three repetitions, with no confidence-qualified release or universal speed
claim. Phase clocks may nest; sampled process-tree RSS is not an allocation peak.

| Retained evidence | Contents |
| --- | --- |
| [Configuration summary](../bench/results/scc-20261024-summary.csv) | All 193 arm cells, medians/ranges, eligibility, paired ratios, hosts and sampled memory. |
| [Observations](../bench/results/scc-20261024-observations.csv) | All 579 planned positions, failures/skips, process/command times and available native phase clocks. |
| [October 12 comparison](../bench/results/scc-20261024-comparison.csv) | All 56 default-fixest configurations, both campaigns' hosts/times, eligibility and paired ratios. |
| [Inputs, identity and checks](../bench/results/scc-20261024-inputs.json) | Source/plugin/input identities, six successful accounting records, host checks, oracles, repeat/worker checks, 168 historical science checks and artifact hashes. |

Raw captures, compact collection, the checked 21-page PDF and export script
remain on SCC under
`/projectnb/welfgr/fereg/runs/20261024T135209Z-beta3-fixest-7a9ac5c4/`.

## Comparison protocol

Use the maintained [SCC package comparison](../bench/README.md#scc-package-comparison)
for OLS/PPML comparisons with fixest, reghdfe and ppmlhdfe. The campaign freezes
package versions, datasets, model options, worker counts and a fixed repetition
budget. Compare packages within the same isolated allocation on a general
eligible node.

Timing includes estimation, inference and requested fixed-effect recovery;
input generation, import and export are outside the measured command. Report
paired summaries by workload and worker count, including failed, timed-out and
scientifically ineligible comparisons. A fast result is eligible only if sample,
coefficient, covariance, objective and requested-output checks pass. Default
and strict comparator tolerances are separate profiles.

Benchmark results apply to the source revision and binaries measured. They do
not establish a speed ranking for a newer release or for another platform.
The accepted changes to the current OLS Auto policy and their qualification
limits are summarized in [VALIDATION](VALIDATION.md).

For a quick local development check, use [the small benchmark](../bench/README.md#local-benchmark).
For a change to fereg itself, use the [fixed-candidate qualification](../bench/README.md#fixed-candidate-qualification).
