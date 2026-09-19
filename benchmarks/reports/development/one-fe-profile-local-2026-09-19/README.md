# One-FE OLS: where the fixest gap comes from

**Fereg loses principally in input transport and the surrounding data passes,
not because fixest solves the one-FE least-squares problem faster.** On this Mac,
the one-million-row, 50-regressor, 16-thread command takes 0.593 s versus
fixest's 0.461 s. Fereg spends 0.307 s loading observations and 0.215 s in native
work through precommit; the remaining 0.070 s includes Stata preparation,
publication and native work after that measurement boundary.

This is a profiling result, not an adopted optimization or release claim.
No estimator policy, installed artifact, reference pin, or ongoing SCC campaign
was changed. No SCC job was submitted. All measurements were completed on the
Mac using its separately installed Command Line Tools.

## Experiment and scientific checks

Source: `55c19cb679d675194c6baed74b09279e1f026b16`. The native/Stata source is
unchanged from the light campaign's `e23f15ca` candidate. Stock and diagnostic
artifacts, exact fixest archive, controller snapshots and failed/discarded
attempts are recorded in [identities.json](identities.json). The fixest source
archive is 0.14.2, SHA-256 `17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
The diagnostic copy adds R phase laps and C++ demeaning timers; it does not
alter numerical statements. The [patches](patches/) and
[harness](../../../one_fe_profile/README.md) make these changes reviewable.

Apple M2 Ultra, 24 cores, 192 GiB RAM; Stata/MP 19 with an eight-processor
license, R 4.6.1, Rust 1.85.1, Apple clang 17.0.0. Rust/fixest receive the stated
1/4/16 native threads; Stata's own processor count is separately capped by its
license. BLAS uses one thread. macOS placement is uncontrolled. Full environment
details are in [environment.json](environment.json).

The model is unweighted IID OLS with one categorical intercept FE and ordinary
numeric regressors. The primary FE has five consecutive observations per group.
Data come from the existing frozen generator with seed 104729. Both languages
receive identical numerical bytes, verified columnwise before timing. The
command includes estimation, inference and normal result construction; loading
the dataset from disk, independent auditing and exports are outside timing.
Optional residual variables and saved individual effects are not requested.
Fixest's default retained results, including its score matrix, are retained.

There are 152 accepted calls: four explicit-dummy oracle checks, 90 primary
fits, 40 group-layout fits, six detailed native fits, two Stata profiler fits,
four stack-sampling calls and six publication checks. Each stack-sampling call
contains eight fits and is excluded from timing comparisons. All process,
application-sentinel and scientific checks pass. Samples, slopes, full slope
covariances and RSS agree across packages at the existing tolerances. Native
stock/diagnostic full b/V, rank/status, DoF, fit and certificate tables agree
exactly. [science.json](science.json) records the checks and output hashes.

## Complete-command timings

Seconds are medians of three fresh-process stock invocations, each after a
small untimed warm-up. Command order reverses on repetition two. Ratios are
medians of paired ratios, so they need not equal ratios of the displayed
medians. Raw timings and ranges are in [measurements.json](measurements.json)
and [summary.json](summary.json).

| Observations | Regressors | Threads | fereg | fixest | fereg / fixest |
|---:|---:|---:|---:|---:|---:|
| 245,760 | 10 | 1 | 0.118 | 0.085 | 1.29 |
| 245,760 | 10 | 4 | 0.069 | 0.057 | 1.21 |
| 245,760 | 10 | 16 | 0.068 | 0.060 | 1.15 |
| 983,040 | 10 | 1 | 0.428 | 0.284 | 1.50 |
| 983,040 | 10 | 4 | 0.252 | 0.169 | 1.50 |
| 983,040 | 10 | 16 | 0.223 | 0.142 | 1.58 |
| 983,040 | 50 | 1 | 1.712 | 2.325 | 0.74 |
| 983,040 | 50 | 4 | 0.791 | 0.820 | 0.95 |
| 983,040 | 50 | 16 | 0.593 | 0.461 | 1.28 |

The 50-column single-thread win is substantial. The four-thread result is
close and should be treated as near parity. More threads do not remove the
input bottleneck: loading remains about 0.30 s while native computation falls
from approximately 1.27 s at one thread to 0.215 s at sixteen.

Changing only the FE grouping or row ordering does not reverse the 16-thread
loss:

| 983,040 rows, 50 regressors | Threads | fereg | fixest | Paired ratio |
|---|---:|---:|---:|---:|
| 1,000 interleaved groups | 4 | 0.782 | 0.765 | 1.02 |
| 1,000 interleaved groups | 16 | 0.579 | 0.441 | 1.31 |
| Original groups, shuffled rows | 4 | 0.871 | 0.775 | 1.13 |
| Original groups, shuffled rows | 16 | 0.605 | 0.433 | 1.40 |

The outcome/regressors stay fixed in the fewer-groups variant; it is a
controlled grouping sensitivity, not a regenerated correctly specified model.
Shuffling preserves each observation's complete row.

## Cost accounting

The stock fereg boundary measurements are:

| 983,040 observations, 16 threads | 10 regressors | 50 regressors |
|---|---:|---:|
| Input loading | 85.8 ms | 307.1 ms |
| Native preparation/estimation through precommit | 117.8 ms | 214.9 ms |
| Command minus native precommit boundary | 21.2 ms | 70.1 ms |
| Complete command | 223 ms | 593 ms |

Preflight is below 0.5 ms. Individual medians need not add exactly. The last
component is **not all ado time**: the native runtime table is captured before
final memory/publication checks, SPI stores and cleanup.

The separate symbol-bearing diagnostic build splits input loading into:

| Input subphase | 10 regressors | 50 regressors |
|---|---:|---:|
| Allocate/initialize input buffers and metadata | 6.4 ms | 27.0 ms |
| SPI reads, chunk bookkeeping and missing masks | 79.8 ms | 269.8 ms |
| Finalize input owners | <0.01 ms | <0.01 ms |

The wide input has 52 numeric sources including outcome and FE: 51,118,080
scalar values, about 390 MiB of doubles. The bridge loops through observations
and variables and calls `SF_vdata` for each value. Rust then constructs missing
masks. This is one main-thread input stage; increasing the Rust team cannot
parallelize those SPI calls. The measured read stage is about 1.5 GB/s of
double payload, an effective transport rate including API overhead, not a
hardware memory-bandwidth measurement.

Native preparation adds about 20 ms in the wide case: 5 ms for sample
construction/availability checks and 14 ms for factorization, singleton
handling and finalization. The corresponding narrow sample step is about
10 ms; the wide preparation team threshold matters here.

### Native work versus fixest

These are nested diagnostic medians, not components to add to the stock
command a second time. Boundaries differ between implementations; the table
shows them explicitly rather than pretending their internal phases coincide.

| Wide, 16 threads | fereg | fixest |
|---|---:|---:|
| RHS materialization | 31.6 ms | included in R environment/model setup |
| Raw reference norms/means | 21.5 ms | different reference/check contract |
| Initial one-FE absorption | 55.1 ms | 156.0 ms demeaning, including buffers/export |
| Finite regression | 31.5 ms | 136.5 ms |
| R environment/model setup | — | 73.0 ms |
| Retained score-matrix construction | not required for IID result | 68.0 ms |

Even charging fereg's RHS and raw-reference passes to absorption gives about
108 ms versus fixest's 156 ms. Fixest's demeaning timer contains roughly 43 ms
of workspace setup, 18 ms computing/broadcasting group means and 95 ms
subtracting/exporting results. Its native code allocates an observation-by-RHS
mean buffer, then constructs the demeaned matrix in a separate pass.

The deeper fereg finite-regression trace is approximately 3 ms setup/scaling,
25 ms standardized moments, and 4 ms guarded Cholesky, residuals and associated
certificates. No QR fallback occurs in these detailed cases. Fixest spends
about 120 ms in cross-products. Fereg's block-based moment path is a strength
to preserve, especially given its single-thread result.

Fereg still makes useful but expensive additional passes: about 10 ms for
reported-intercept bread/raw residual construction, 7 ms for residual
absorption, 6 ms for score checks, and 17 ms for inference setup/fit statistics.
The IID covariance, Wald and final inference-certificate arithmetic themselves
take well below one millisecond. These observations do not authorize removing
any independent check.

### Stata preparation and publication

Three separately timed diagnostic fits give this 50-column transaction split:

- 29 ms before entering the plugin, including parsing, metadata and shadows;
- 503 ms inside the plugin call, including its final stores and cleanup;
- 2 ms validating/naming returned results;
- 15 ms in `ereturn post ... buildfvinfo`;
- 1 ms in the remaining publication; and
- approximately 21 ms outside those transaction timers, principally the
  public wrapper, consistent with the separate Stata program profiler.

The same costs are much smaller for ten columns: about 9, 208, 1, 2, 1 and
4 ms respectively. Stata-side preparation and metadata work merit attention,
but result posting alone cannot explain or close the wide gap.

[detailed-phases.json](detailed-phases.json) contains the exclusive subphases.
[stack-summaries.json](stack-summaries.json) corroborates the wide-case input
call chain and identifies fereg group-mean/scaled-norm work and fixest's
cross-product loops. Samples cover all threads, including waits: their counts
must not be interpreted as wall-time percentages. The narrow fereg sampler
captured no usable stacks; no narrow native-sampling conclusion is claimed.

## What to optimize next

1. **Treat main-thread ingestion as a first-class target.** The approximately
   270 ms read stage is the largest wide-case cost. Measure bridge-loop and
   buffer-layout alternatives against this exact stage, preserving SPI
   main-thread access, storage-type/missing semantics and original-row error
   precedence. Earlier work already rejected a simple smaller-chunk change;
   repeating that experiment alone is unlikely to deliver the missing gain.

2. **Prototype a one-FE numeric path that reduces full-matrix passes.** The
   RHS/reference/absorption stages together consume about 108 ms at sixteen
   threads, and substantially more at one thread. Test retaining the owned
   column representation longer and/or stable group-centered tiles feeding the
   existing efficient moments. Count materializations and passes explicitly.
   Retain original-scale norms, rank fallback, independent FE/score/bread checks
   and final-sample ordering. Do not replace stable centering with subtraction
   of large raw cross-products. This is a hypothesis, not a measured saving.

3. **Reduce duplicate Stata syntax/metadata work and combine compatible
   native reductions.** The wide pre-plugin and public-wrapper costs total
   about 50 ms. Native fit-statistic scans add about 17 ms in both widths.
   These are bounded opportunities with useful narrow-model impact; neither
   justifies changing the public transaction or statistical outputs.

4. **Preserve the current finite solver and single-thread advantage.** A
   generic solver replacement has the wrong measured target. Any new one-FE
   route needs the ordinary, shuffled and fewer-group controls, then focused
   weighted/missing/rank-deficient checks as its eligibility expands.

The target is concrete: the wide stock median needs approximately **132 ms**
removed, or 22% of command time, to reach the observed fixest median. With
unchanged input loading, everything else must fall from roughly 286 to 154 ms
(about 46%). Halving the current 215 ms native phase alone would save only
107 ms and still miss the target. Halving only the finite solve saves about
16 ms. This calls for coordinated transport/pass/preparation improvements,
with a new command-level win demonstrated after each adopted change.

## Limits and instrumentation effects

This is complete-data numeric IID OLS on one Apple Silicon machine, with one
seed and three timing repetitions. It does not establish weighted, robust,
clustered, factor-variable, saved-effect, low-memory, Linux or universal
performance. No hardware cache/bandwidth counters were collected. Process RSS
was sampled every 50 ms; short fits are under-resolved, so no comparative
peak-memory claim is made. Native allocation figures remain available in the
numeric receipts.

Stock runs supply every speed comparison. Wide 16-thread traced command medians
were 0.580 s for fereg and 0.4605 s for fixest, versus stock 0.593/0.461 s.
For ten columns, fixest tracing gave 0.1585 versus stock 0.142 s, so its narrow
phase totals are less stable. Trace/stock differences include run variability
and cannot identify pure instrumentation overhead. The symbol-bearing native
diagnostic also differs in code layout; its three wide commands were
0.576/0.589/0.571 s with exact scientific outputs. These diagnostics locate
costs; their faster times are not optimization gains. Nested times are never
blindly summed, and unaccounted command time remains visible.

The failed compiler setup, disabled-Rprof wrapper issue, wrong-working-directory
diagnostic build, and suppressed-display publication probe remain recorded in
the identity report and local evidence. None is included in accepted timing
or scientific counts. No production optimization has been applied yet.
