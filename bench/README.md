# Local benchmark

Four fixed-seed OLS/PPML models exercise one and two fixed effects. The `smoke`
tier uses 10,000 rows and one worker; `standard` uses 100,000 rows and 1/8 workers.
Each has a strict-tolerance reference, one warm-up and five fresh Stata processes.
Preparation is outside timing. This is a descriptive development benchmark,
not an acceptance campaign or an independent scientific oracle.

With Python 3.11+, NumPy and pandas installed, and the current plugin built:

```sh
python3 bench/run.py --tier smoke --stata "$STATA_BIN" --output target/bench-smoke
python3 bench/compare.py target/bench-smoke/run.json
python3 -m unittest discover -s bench/tests
```

The complete benchmark test suite additionally requires SciPy, psutil,
matplotlib and ReportLab for the independent references and campaign reporting tools.

The output directory must be new. Runtime/frontend hashes are checked throughout;
inputs are hash-bound and round-trip verified. Failed commands and partial runs
remain visible. Comparisons retain coefficient, covariance, scalar, exact sample,
and requested-output checks, with a bounded immutable-reference cache. Timings use
Stata's timer and report medians by workload; they cannot establish small changes
below its resolution. Dedicated performance decisions require a separately declared
bounded protocol under `docs/development_workflow.md`.

Generated captures belong under `target/tmp/` and are deleted after inspection.
Historical campaigns and the legacy comparator are available only in Git history.

## CMG setup diagnostic

`cmg_setup.py` freezes the three-arm historical/current/public comparison described
in the [October 21 evidence](results/cmg-setup-diagnostic-20261021.json). It reuses
the native qualification runner's science, resource, timer and host checks, but
never qualifies default activation. Run `prepare CONFIG --output PLAN`, then
`run PLAN --output RESULTS --replica N`; `report PLAN --output RESULTS` additionally
requires `--accounting DIRECTORY` on SCC. The four input families, original seed,
worker counts and per-platform budgets are fixed by this diagnostic version.

For a future arithmetic-preserving setup comparison, select
`"diagnostic": "cmg-setup-candidate-v1"` and order seed `2026102104` in the
config. Supply exactly `accepted_hidden` (full revision
`3f9d54eeacb2ced61bf8cb9a629daa0d1a6d23d9`) and `setup_hidden` (the explicit full
candidate revision), each with `path`, `revision` and `build`. Both use
`incremental-two`; the distinct actual plugins, harness, inputs and execution
sources are hash-bound at preparation. The same four seed-2026101821 Auto/Auto
inputs run at W1/W4/W8, with one warmup per arm, six isolated Linux hosts with
two mirrored blocks each (720 calls), or twelve Mac blocks (600 calls). This
mode requires exact paired scientific fingerprints, solver signatures and
present route/hierarchy records (both may be empty before iteration 16),
independent oracles, exact own repeats and existing cross-worker checks. It
scores complete-call `NATIVE_SECONDS` with unchanged 120-second call and
3600-second Linux/14400-second Mac budgets, zero recovery or resampling, and
retains failures. This protocol description is not timing evidence and this
diagnostic cannot qualify public activation. The historical three-arm mode
remains the default.

The fixed pivot-repair comparison uses `"diagnostic": "cmg-setup-pivot-v1"`
and the same order seed. Its three distinct artifacts are `accepted_3f9_hidden`
at `3f9d54eeacb2ced61bf8cb9a629daa0d1a6d23d9`, `pre_pivot_040_hidden` at
`0407060c2959ff5076619511fcc85d82ceac8de0`, and `pivot_1cdb_hidden` at
`1cdb225e2460e391ad38f856755ae3ea5efb2e5b`. Both reference arms must agree
exactly with the pivot arm. The unchanged input, worker, block, warmup and
timeout rules give 1,080 calls across six isolated Linux hosts and 900 on Mac.
Each comparison retains its own nonregression bound; no activation follows.
The two earlier modes and their recorded artifact meanings are unchanged.

For separate setup attribution, apply `cmg_setup_profile.patch` to the exact
accepted `3f9d54ee` source and build/audit a distinct plugin. The bounded helper
`cmg_setup_profile.py REPOSITORY TASK_ROOT OUTPUT` requires the completed ordinary
diagnostic, matching frozen inputs, and that instrumented artifact. It runs only
18 calls and checks their exact baseline science/signatures. Its six phase timers
are attribution evidence, not qualification timings. Arrange host isolation
externally and keep profile measurements separate from the complete-call panel.
The initial executed ad hoc helper predates the maintained helper's identity
guards; that limitation remains explicit in the evidence.

`cmg_setup_attribution.py prepare` supports the exact `accepted`, `candidate`
and `pivot` source pairs. For the new comparison, use `--hidden-arm
accepted_3f9_hidden` or `--hidden-arm pre_pivot_040_hidden` for the first two;
`pivot` defaults to `pivot_1cdb_hidden`. An alias cannot change either revision.
Prepare fresh `counts` and `timers` copies with the final helper, build and audit
them, then `bind` their source archive, actual plugin and build log. `run`
requires the completed corresponding ordinary panel and checks all 18 calls
against that selected arm. The pivot's generic comparator is counted once,
including sampling and fallback; production work charges and polls are unchanged.
Counter-build wall time is never performance evidence. Select and freeze the
profile arms and finite budget before collecting new observations.

## Two-FE remaining-cost calibration

The [development specification](../docs/two_fe_auto_policy.md) freezes the
policy, label definitions, seeds, support requirements and finite budgets.
`two_fe_policy_inputs.py` prepares its 16 full-size inputs and 32 Fast/Low
cases with independent references, using a separately built and identified
`replay_large` executable. Its output directory must be new; no seed or size
overrides, resume or replacement-input path is provided.

```sh
python3 bench/two_fe_policy_inputs.py --generator REPLAY_LARGE --output target/tmp/TASK/inputs
python3 bench/two_fe_policy.py prepare CONFIG.json --output target/tmp/TASK/plan.json
python3 bench/two_fe_policy.py run target/tmp/TASK/plan.json --output target/tmp/TASK/results
python3 bench/two_fe_policy.py report target/tmp/TASK/plan.json --output target/tmp/TASK/results
```

The configuration uses the ordinary qualification `platform`, `harness` and
`cases` fields. Its three artifact keys are `never`, `force16` and `force32`,
with requests `two-remaining-never`, `two-remaining-force16` and
`two-remaining-force32`, respectively; all declare `families: ["two"]` and
identify the same actual plugin, full source revision and build settings.
The helper fixes W1/4/8, one retained warmup, two mirrored recorded calls,
120 seconds per call and 14,400 seconds per platform: 864 calls per platform.
Linux execution uses the verified whole-node runner, and its report also
requires `--accounting DIRECTORY`. No continuation or retries are available.

After both complete reports, `two_fe_policy_calibrate.py --platform MAC_PLAN
MAC_RESULTS --platform LINUX_PLAN LINUX_RESULTS --output NEW_FIT.json` verifies
the exact reported results, common source/input bindings and deterministic
labels, then fits once with the specified rational/encoded arithmetic. It
retains unavailable pairs and unsupported keys. A failed fit supplies no
fallback coefficients. These tools do not edit runtime coefficients or activate
Auto; held-out development validation and the separate 42-cell qualification
remain required.

That calibration is complete. The [direct decision-loss revision](../docs/two_fe_auto_gates.md)
reuses its committed observations with `two_fe_policy_gates.py`; it does not
rerun this panel. Its exact joint programme loss, fixed cuts and tie rules are
frozen before a single fit. Generic evaluation and encoding remain offline;
the runtime implements only the selected dependencies. Full independent
proposal and cross-critique reports are retained in the
[review record](results/two-fe-policy-revision-review-20261022.json).

## Packed SCC comparison

The [retained October 10 results and October 12 optimized rerun](../docs/fixest_benchmarks.md)
include all configuration summaries, individual timing observations, frozen input
identities and subsequent diagnostics. The rerun reuses the same dataset bytes;
its added strict OLS arm remains separate. Scientific failures and unavailable
cells remain explicit.

`campaign.json` and `campaign.py` define the current bounded SCC comparison:
56 configurations and three repetitions, with a two-way worker/firm AKM baseline
at one million rows and 16 controls. Four workers are primary; 1/8/16-worker
variations and selected 16-worker clustered/saved-output cases remain on the
same node as their baseline. Stata comparators are included only at 1/4 workers.
The broader one-/two-/three-FE dense/sparse panel uses 100,000/1,000,000/4,000,000
rows. Clustering is by worker. No proprietary dataset is required.

`scc/setup.sh` builds one exact committed source with GCC 12.2 selected explicitly
for the C bridge and sanitizer checks, verifies the reused pinned Stata closure,
freezes executable/frontend/comparator/harness bytes and tests the tooling.
`scc/job.sh` is the compute entrypoint: 28 reserved physical cores
on exactly 28-core nodes, sequential fits, explicit affinity, three GB/core and
four-hour tasks. Run an independent small explicit-design smoke before the four
production bundles. Setup, smoke and production accounting, licensed sentinels,
manifest coverage and frozen identities must all be checked at collection.
SCC's `max_slots_per_host` rule caps all jobs together at the host core count;
the exact 28-core filter and 28-slot request provide full-node isolation. Do not
add the unconfigured `exclusive` consumable: an explicit request matches no
queue and prevents dispatch.

Allowed physical cores are grouped by socket, with the largest allowed socket
first, so a fit stays on one socket whenever its worker count fits. Both arms
use the same prefix and `numactl` binds memory to the selected CPUs' NUMA node
(or nodes when the fit spans nodes). Every observation records the actual mask,
socket/node IDs, `lscpu` and transparent-hugepage setting. Missing topology or
failed memory binding stops the invocation rather than silently changing placement.

The stopwatch covers estimation, inference and requested individual FE recovery.
For fixest this includes lazy extraction and observation-level materialization.
Numeric FE IDs are matched by value to fixest's named effects, avoiding R's
scientific-notation formatting of large IDs. The regression witness checks
reordered labels above the 32-bit integer limit against both built-in predictions
and explicit-dummy OLS/Poisson fits.
Data preparation/import, small initialization and disk export are excluded.
The PPML fixest profile disables its K correction, applies the declared robust
N/(N-1) correction inside timing, and preserves its original covariance alongside
the aligned covariance. Default and stricter convergence profiles remain separate.
Every new OLS cell also includes `fixest-strict`, using `fixef.tol=1e-11`
and `ssc(K.exact=TRUE)`. Its covariance DoF come from `df.K`; the approximate
`nparams` count is retained separately. This is an accuracy profile, not a
guarantee of matching the native nesting convention: strict clustered cells
still have to pass the independent comparison. Manifests freeze their arm lists,
so adding this profile does not reinterpret the retained October campaign.
Native campaign metadata also records OLS preflight/input/native/total seconds
from runtime rows 18–21, or PPML total seconds and all 13 named phase rows
28–40. Missing phase observations remain missing; nested phase clocks must not
be summed as a disjoint elapsed-time decomposition.
Stock Stata covariance differences remain explicit failed comparisons.
PPML absorbed counts are normalized to the full categorical FE-span rank:
fereg transfers one intercept direction to structural coordinates, clustered
Stata counts exclude nested workers, and fixest retains the full nuisance count.
Raw counts and nuisance-constant omission flags are retained; the small explicit
dummy-design oracle independently checks the normalized rank before production.
Samples, all finite coefficient/covariance coordinates, objectives and saved
predictions are checked at SPEC tolerances. Only three valid paired repetitions
enter timing ratios; failed, timed-out, skipped and unattempted cells remain visible.
Completed command times may also be displayed with an asterisk when result checks
fail; these are descriptive observations with unchanged scientific failure flags.
Three completed commands give a median, and a single completed command is labelled
`n=1`. Errors during requested FE recovery and timeouts supply no completed timing.
The small dummy-design oracle validates coefficients, full covariance and fitted
values independently; it does not adjudicate every large production discrepancy.

The 2026-10-08 saved-output investigation requalified the numeric-ID repair in
all nine affected wrapper calls. Fixest's reconstructed and built-in predictions
agreed within 1.1e-13; default convergence still failed fitted-value comparisons,
while the separate strict profile passed both million-row PPML saved cases.
For million-row OLS, reghdfe's individual FE recovery had a score of 3.91e-6
versus 3.66e-8 for its residual-based built-in predictor; `tolerance(1e-12)`
reduced the saved score to 7.44e-10. ppmlhdfe's built-in predictor also failed
the score gate, including some tighter-tolerance diagnostics. These failures
are not explained by our row export or intercept assembly. The precise PPML
convergence/postestimation cause remains unresolved. Original captures are
preserved; single-fit diagnostics supply no replacement campaign timing ratios.

Generated inputs, row captures and logs remain in the remote run; collect compact
JSON/CSV summaries and the final PDF. This descriptive campaign establishes no
universal speed claim or platform qualification.

For comparisons between campaigns, verify the recorded numeric column hashes
before claiming exact input or result identity. A fixed seed does not guarantee
identical floating-point inputs across processor features: generation includes
NumPy transcendental functions. Keep differing inputs visible alongside host,
timing and scientific comparisons. An exact before/after binary comparison must
reuse the same generated inputs rather than regenerate them on another host.

`campaign_collect.py <context.json> --output <new-directory>` collects the
terminal scheduler records, application sentinels, frozen inventory and compact
task results. It leaves the datasets and row exports on SCC. With Matplotlib and
ReportLab available, `campaign_report.py <collected-directory> --output <directory>`
produces a PDF and two CSVs. The report rejects incomplete job evidence, requires
all three paired repetitions for ratios, checks exact native repeatability and
cross-worker scientific agreement, and marks descriptive timings with result
discrepancies `*`. Unavailable times and unvalidated ratios remain NR. Optional
`--saved-fe-retests <accepted-diagnostic-directory>` adds a separate single-fit
retest panel and CSV; it does not replace production medians. Render and inspect
every PDF page before delivery.

## Native nonregression timing

The frozen timing workloads are six standard designs at one and four workers,
the million-row K=20 two-FE OLS design at eight and sixteen workers, and wide
K=64/65/129 robust designs. The high-worker cases guard norm/certificate scaling
that the one/four-worker panel missed. Hashes are in `standard_cases.json`.
Regeneration rejects missing, modified and unlisted `.case` files, including
stale files left in the output directory.

```sh
python3 bench/make_standard_cases.py
bench/build_timing_harness.sh
python3 bench/time_exact.py --plugin base=BASELINE.plugin --plugin cand=dist/fereg_macos.plugin \
  --cases target/standard-cases/*.case --output target/tmp/timing/RUN
```

This historical runner reproduces exact science and solver signatures and uses
the paired-median bootstrap upper bound against max(5%, 1 ms) per workload.
New qualification follows the [controlled block protocol](../docs/development_workflow.md#controlled-timing-protocol-approved-2026-10-14): ordinary OS bursts are
recorded, while competing builds, agent computation and estimation jobs remain
excluded. Existing accepted observations are not rerun merely because the
protocol changed. The former three-FE LSMR
timing inputs are not reproducible from a tracked recipe and are not frozen.

The [C2 qualification follow-up](results/c2-20261012-qualification.json) retains
the fixed fifteen-pair Mac panel and separate SCC diagnostics, including all
failed/inconclusive bounds. Johannes accepted the frozen candidate with the
remaining timing uncertainty; see
[the adoption decision and limits](../docs/VALIDATION.md#c2-adoption-with-unresolved-timing-2026-10-12).

### Controlled qualification

Use the [fixed-candidate runner](#fixed-candidate-qualification) for new routing
qualification. The former `time_blocks` and routing A/B/C coordinators are retired.
Their committed results retain their original meaning; no historical campaign
must be replayed to admit a new candidate. Separate CMG-Schur and profiling tools
below remain useful for bounded diagnosis.

### CMG-Schur prototype comparison

[`cmgschur_cases.py`](cmgschur_cases.py) and [`cmgschur.py`](cmgschur.py) compare
OLS `auto`, `cmg` and `cmgschur` with degree cutoffs 4, 5 and 6, using exactly two
categorical intercept effects, unit weights and robust covariance. This is a
prototype experiment, not release qualification. NumPy, a built native plugin
and the `fereg-oracle` `replay_large` example are required; Stata is not required.

The fixed synthetic panel contains a 600-row dense-oracle fixture, a 100,000-row
two-period AKM panel, a 1,000,000-row long panel with degrees through ten, and a
100,000-row graph with weak links between ten communities. Repeated worker-firm
observations are retained. Reported degree counts describe the input graph
before estimator pruning. The two-period case cannot distinguish the cutoffs.

```sh
cargo build --locked -p fereg-oracle --profile ci --example replay_large
python3 bench/cmgschur_cases.py --output target/tmp/cmgschur/cases
python3 bench/cmgschur.py --plugin dist/fereg_macos.plugin \
  --oracle target/tmp/cmgschur/cases/oracle.case \
  --cases target/tmp/cmgschur/cases/short.case \
          target/tmp/cmgschur/cases/long.case \
          target/tmp/cmgschur/cases/weak.case \
  --threads 1 4 --timeout 90 --budget 1800 \
  --output target/tmp/cmgschur/local-panel
python3 -m unittest discover -s bench/tests -p 'test_cmgschur.py'
```

Freeze the plugin, inputs, source revision and any uncommitted patch before
starting; retain those identities with the aggregate findings. The runner binds
plugin, harness, case and benchmark-script hashes, plus Git HEAD and the dirty
flag when available. Archives may supply `.fereg-source.json`; its hash and JSON
declaration are retained without treating them as a verified clean checkout.
Generation defaults to tolerance `1e-8`; every timed arm preserves that same
input tolerance. A single-worker LSMR reference at `1e-12` precedes each input
for each supplied plugin; the references must agree scientifically.
The small prerequisite also checks that reference and every tested arm against
independent dense dummy OLS and HC1 covariance, including the stronger CMG
absolute residual/fitted-value bound. Larger cases compare with strict LSMR;
that comparison is a numerical witness, not an independent dense oracle.

The stopping rule is fixed before execution: one warmup and seven rotating,
paired repetitions per arm at one and four workers, 90 seconds per process,
and a total 1,800-second budget including references. There is no automatic
resampling or continuation. Science checks enforce SPEC numerical envelopes,
exact sample/status/DoF decisions and exact repeats at fixed worker count.
A failed reference blocks its dependent work. A failed `cmgschur` cell stops
dependent timing; a failed original `auto` or `cmg` cell stays explicitly failed
and is excluded from ratios while other arms and cells may continue. A failed,
timed-out or incomplete comparison cannot establish a speed gain.

Native timing surrounds one complete plugin call, excluding replay file I/O.
`summary.json` retains individual observations, solver setup/solve diagnostics,
peak process RSS, and paired summaries. Phase clocks may overlap; do not add
them as an elapsed-time decomposition. Run controlled measurements after builds, tests
and other agent workloads finish. SCC observations do not qualify Mac speed.

For an optimization against a frozen parent, `--baseline-plugin` adds
`baseline-cmgschur4`, `baseline-cmgschur5` and `baseline-cmgschur6` to the existing
candidate arms. Both artifacts are hash-bound at start/end and in each record.
`candidate_over_baseline` compares matching cutoffs; values below one favor the
candidate. Each artifact must repeat exactly at a fixed worker count, while
cross-artifact numerical differences use the existing scientific envelopes and
exact discrete decisions. Every selected arm uses the same frozen input bytes.
Orders rotate and reverse in pairs, including a two-arm-only comparison.

Optional `--extra-cases wide orthogonal` leaves the original four generated
case files unchanged and adds a 100,000-row case with 16 controls containing
different worker/firm FE components, plus a 600-row dense-oracle `nocons` case
whose outcome and four controls are already exactly FE-orthogonal. Omitting
the reported constant keeps its projection RHS batch entirely orthogonal;
the independent oracle retains the nuisance constant through full worker dummies.
Four wide-case
controls are also exactly orthogonal, so batch RHSs have different work needs.

```sh
python3 bench/cmgschur_cases.py --extra-cases wide orthogonal \
  --output target/tmp/cmgschur-opt/cases
python3 bench/cmgschur.py --plugin CANDIDATE.plugin --baseline-plugin PARENT.plugin \
  --oracle target/tmp/cmgschur-opt/cases/oracle.case \
  --cases target/tmp/cmgschur-opt/cases/short.case \
          target/tmp/cmgschur-opt/cases/long.case \
          target/tmp/cmgschur-opt/cases/weak.case \
          target/tmp/cmgschur-opt/cases/wide.case \
          target/tmp/cmgschur-opt/cases/orthogonal.case \
  --threads 1 4 --timeout 90 --budget 1800 \
  --output target/tmp/cmgschur-opt/local-panel
```

Declare any smaller arm set before running with `--arms`; for example,
`--arms baseline-cmgschur4 cmgschur4` retains seven paired repetitions for a
bounded Veneto parent comparison. Baseline labels require `--baseline-plugin`.
The default still includes candidate `auto`/`cmg` comparisons. Removing those
arms deliberately means the run makes no new comparison against them. Keep
all outcomes when a budget expires; do not choose another panel after seeing
its timings and combine it as though it were the originally declared panel.

For a change that must leave the exact hybrid untouched, declare
`--exact-baseline-arms cmgschur4` and select both `cmgschur4` and
`baseline-cmgschur4`. Their full scientific-output and solver-signature hashes
must match exactly at each case/worker count. The runner fails as soon as both
hashes are available, including during the dense prerequisite. This optional
guard also supports cutoffs 5 and 6; ordinary result-changing parent comparisons
retain the numerical envelopes unless the guard is explicitly requested.

Sampled preconditioners are explicit-only arms: `cmgschur4-sample1`,
`cmgschur4-sample2` and `cmgschur4-sample4`. They request `solver=cmgschur`,
`cmgdegree=4`, `cmgsamples=1/2/4` and `cmgseed=0`. The default arm list is
unchanged. Absent/zero samples retain the exact star hybrid; positive samples
average that many independently sampled weighted GKS trees for high-degree
workers. Preparation strips sampling options from every nonsampled arm and
strict reference, including parent binaries, which do not support sampled arms.
The sampling count and fixed seed are retained in `summary.json`.

Use the optional high-degree fixtures to expose this tradeoff:

| Extra case | Rows | Workers | Firms | Maximum worker degree | Graph |
|---|---:|---:|---:|---:|---|
| `sampled-oracle` | 3,000 | 60 | 40 | 30 | Dense witness; 20 workers have degree 30 |
| `d30-mixed` | 600,000 | 10,000 | 1,000 | 30 | Mixed degrees 1/2/4/6/12/30 |
| `d30-weak` | 600,000 | 10,000 | 1,000 | 30 | Ten communities joined by nine workers |
| `d60-stress` | 600,000 | 5,000 | 1,000 | 60 | 4,000 workers have degree 60 |

All include repeated worker-firm observations and a connected firm graph.
Degrees count distinct firms; generation records the actual histogram and fixed
graph seed. Original default fixtures are unchanged. The dense prerequisite
contains unequal repeated-link counts among degree-30 workers, checks independent
OLS/HC1 science, and runs before the larger panel. Every selected sampled arm
also passes strict-reference comparisons and exact fixed-worker repeats.

```sh
python3 bench/cmgschur_cases.py \
  --extra-cases sampled-oracle d30-mixed d30-weak d60-stress \
  --output target/tmp/cmgschur-sampled/cases
python3 bench/cmgschur.py --plugin CANDIDATE.plugin --baseline-plugin PARENT.plugin \
  --oracle target/tmp/cmgschur-sampled/cases/sampled-oracle.case \
  --cases target/tmp/cmgschur-sampled/cases/d30-mixed.case \
          target/tmp/cmgschur-sampled/cases/d30-weak.case \
          target/tmp/cmgschur-sampled/cases/d60-stress.case \
  --arms auto cmg cmgschur4 baseline-cmgschur4 cmgschur4-sample1 cmgschur4-sample2 cmgschur4-sample4 \
  --exact-baseline-arms cmgschur4 \
  --threads 1 4 --timeout 90 --budget 1800 \
  --output target/tmp/cmgschur-sampled/local-panel
```

`comparisons_to_cmgschur4` reports sampled-arm ratios against the candidate's
exact hybrid. If `baseline-cmgschur4` is explicitly included with a parent
plugin, `candidate_over_baseline` also compares each sampled arm with that
parent's exact degree-four hybrid. There are no sampled parent arms. The same
full native clock, one warmup, seven rotating paired repetitions, scientific
failure policy and frozen stopping budget apply. This panel measures seed zero;
it does not estimate variation across sampling seeds or search for a faster seed.

For a longer existing panel, run conversion and fitting on the host holding
the data. Supply explicit numeric CSV column roles; `--regressors` is optional:

```sh
python3 bench/cmgschur_cases.py --csv INPUT.csv \
  --outcome y_adjusted --worker worker_id --firm firm_id \
  --output target/tmp/cmgschur/veneto-cases
python3 bench/cmgschur.py --plugin dist/fereg_linux.plugin \
  --oracle target/tmp/cmgschur/cases/oracle.case \
  --cases target/tmp/cmgschur/veneto-cases/csv.case \
  --threads 1 4 --timeout 600 --budget 3600 \
  --output target/tmp/cmgschur/veneto-panel
```

The CSV example predeclares a separate one-hour panel with a ten-minute process
cap; keep the seven repetitions and all five arms. Conversion preserves supplied
rows and outcomes without filtering or adjustment, rejects missing/nonfinite
values, and records the source SHA256. Keep proprietary inputs, generated replay
files and row outputs on their original local/SCC host. Collect only compact
aggregate summaries and hashes. Outputs belong under `target/tmp/cmgschur/`,
`target/tmp/cmgschur-opt/` or `target/tmp/cmgschur-sampled/` for their respective workstreams;
delete them after retaining reviewed aggregates and identities in committed
evidence. Follow the cluster workflow when submitting SCC compute work.

### Categorical preparation probes

The oracle examples `preparation_probe` and `membership_probe` isolate costs
before a complete native timing panel. Build them with the `profiling` profile
and put the Cargo target directory under the task's `target/tmp/` directory.
`preparation_probe ROWS WORKERS REPEATS akm|dense unit|aw|missing [fast]`
times owned preparation, with generation, cloning and exact repeat checks
outside timing. Its optional `fast` argument admits the bounded membership
scratch; the default uses no optional scratch. Set `FEREG_PHASE_TIMING=1` for
the categorical stage clocks. Compare frozen binaries on the same host and
check complete-result fingerprints and retained bytes before comparing times.

`membership_probe ROWS WORKERS LEVELS uniform|skew` compares the production
serial and optional parallel constructors in alternating order, including one
warmup and four measured pairs. Every result must equal the serial reference,
including observation order. Both probes cap inputs at 16 million rows and
32 workers; preparation caps measured repetitions at seven. These diagnostic
observations do not replace the full scientific gate, native replay or controlled
qualification-host timing. The [C2 evidence](results/c2-20261011-performance.json)
retains the bounded exploration and the final candidate separately.

## Stata-free native A/B observations

`replay_large` creates numeric replay cases from the existing oracle designs;
`native.py` compiles the actual-plugin C harness with explicit raised bounds and
runs two supplied plugins in alternating A/B, B/A order. Inputs, plugins and the
harness are hash-bound. Every repetition must pass native return-code,
completion/certificate, exact replay metadata and streamed output-column checks.
A later repeat mismatch fails the run; timings never substitute for scientific
qualification. Output is a fresh directory under `target/tmp/`, including `run.json`,
compact result heads, logs and the harness. Raw result JSON is removed after
successful checks unless `--keep-outputs` is requested. Failed partial runs remain
visible. No licensed Stata installation is required.

```sh
mkdir -p target/native-cases
cargo run -p fereg-oracle --example replay_large --profile ci -- \
  --output target/native-cases/ols.case --rows 100000 --levels 10000,1000 \
  --regressors 5 --weights unit --vce robust --graph akm-strong \
  --solver auto --seed 20261008
python3 bench/native.py --plugin-a /path/to/baseline.plugin \
  --plugin-b /path/to/candidate.plugin --cases target/native-cases/ols.case \
  --threads 1 4 --repeats 3 --timeout 120 --budget 1800 \
  --output target/native-observation
python3 -m unittest discover -s bench/tests -p 'test_native*.py'
```

Declare cases, worker counts, repeats, per-process timeout and total stopping
budget before running. The generator also accepts `--ppml`, two or more comma
separated FE levels (including three-FE designs), weights `unit|aw|fw|pw`,
VCE `iid|robust|cluster`, `--tolerance`, `--iterations`, `--threads`, and explicit
row/cell/edge admission bounds. PPML rejects analytic weights and IID here;
probability weights require robust/cluster. Cluster VCE uses the oracle's cluster
partition. `akm-strong`, `akm-weak`, `chain`, and `barbell` preserve the oracle
algorithms; `akm-strong` means adjacent-firm mobility, not the cloud prototype's
random-mobility override. `random-mobility` draws each row's FE codes independently
before generating outcomes, preserving the
design's coefficient and fixed-effect truth. The other graph families require
at least one complete edge cycle.
The existing barbell generator enumerates dense halves, so large worker/firm
barbells may exceed the edge budget or omit the bridge at the requested row
count and are rejected. Scalable barbell generation remains separate work.

OLS reports RRUNTIME's load/native/total seconds; PPML reports its total native
transaction seconds and does not label that as OLS native-only timing. Full
RSOLVER rows and unrounded seconds are retained. Wall time includes harness I/O;
per-child RSS is bytes (macOS `ru_maxrss` already uses bytes, Linux uses KiB).
These are descriptive observations, not release performance claims. Quiet Mac
qualification and the declared scientific witnesses remain required.

On Linux, add `--cachegrind --threads 1` with explicit 50,000–200,000-row cases.
Valgrind must be installed; instrumentation stays inside the same bounded
interleaved protocol. Each run reports total and per-function self instruction
counts (`Ir`), including harness/I/O costs, in `run.json`; raw Cachegrind files
remain for inspection. Cachegrind on a Mac requires a Linux arm64 container or
VM and Linux plugins. Instrumented runtime measurements are not native wall-time
qualification, and instruction counts from such a guest are not Mac performance
claims. The CLI currently supports macOS/Linux harnesses, not Windows.

## Fixed-candidate qualification

[`qualify.py`](qualify.py) implements one bounded regression round for a candidate
chosen before measurement. Python 3.11+, NumPy and SciPy are required. Build and
check the candidate, protected references and timing harness before planning the
round. The runner never builds artifacts, changes defaults or chooses a successor.
The [development workflow](../docs/development_workflow.md#compact-candidate-qualification)
sets the scientific, platform and operational rules.

The default routing coverage is 21 families at one and four workers: no/one FE,
slopes, low-memory saved effects, dense and small-retained two-FE, three/four/eight/
sixteen-FE, weights, weak/disconnected connectivity, repeated cells, wide RHS,
near-confounded third FE, dynamic weights, inactive columns and three high-degree
families. This is a sparse 42-cell coverage set, not a Cartesian product. Explicit
solver and PPML feature checks remain in `scripts/run_tests.sh` and
`scripts/replay_solver_routing.py`; a full optional-solver timing frontier is no
longer mandatory for each candidate.

The configuration is a JSON object with these fields:

| Field | Meaning |
| --- | --- |
| `scope` | `exact`, `many` or `two`; declares which eligible Auto family may change numerically. |
| `platform` | `Darwin` or `Linux`; build and measure native artifacts for that host. |
| `artifacts` | `original`, `incumbent`, `candidate`, plus `hidden` for public activation. Each has `path`, `revision`, `build`, `request` and `families`. |
| `harness` | Built complete-native-call timing harness. |
| `cases` | One row per fixture: `id`, `family`, `input`, `oracle`, `memory_limit_bytes` (positive bytes or `null`). |
| `threads` | `[1, 4]` for routing coverage. |
| `blocks` | Fixed even number of mirrored blocks, at most 24. |
| `shards`, `replicas` | Linux case partitions and repeated allocations; both are 1 on Mac. |
| `min_distinct_hosts_per_cell` | Optional Linux-only minimum, an integer from 1 through `replicas` (at most 12). The new two-FE policy round sets 12; every cell/reference must have that many accounted physical hosts. |
| `seed` | Unsigned 64-bit ordering seed. |
| `timeout_s`, `budget_s` | Fixed per-call and per-job limits, at most 120 and 14,400 seconds. |
| `coverage` | `routing` requires all 42 cells; `smoke` only checks tooling and cannot qualify a candidate. |

Artifact `build` records compiler and build settings in a short string. `request`
is `auto`, `preserve`, `incremental-many` or `incremental-two`. `families` lists
accepted numerical changes relative to the original (`[]`, `["many"]`, etc.).
Original and incumbent use their actual uninjected public invocation. A public
Auto candidate requires the corresponding hidden artifact, with exact agreement
of scientific output and numerical solver signatures. Unchanged explicit OLS
requests use `scope: "exact"` and `request: "preserve"`. This routing runner is
OLS-only; PPML remains covered by the mapped feature/replay suite.

An oracle is `{"kind": "routing", "path": "reference.json"}` for a bounded
independent categorical reference, `{"kind": "replay", "path": "reference.json"}`
for a native replay reference, `{"kind": "dense"}` for the fixture's independent
dense sidecar, or `{"kind": "inactive"}` for the analytic inactive-column witness.
Missing truth fails preparation or execution; labels never substitute for an
independent check. Paths in the configuration are relative to its directory.
The plan records paths relative to its own directory, allowing a complete input
bundle to be copied to the execution host without retaining old build trees.

```sh
python3 bench/qualify.py plan CONFIG.json --output target/tmp/TASK/plan.json
python3 bench/qualify.py run target/tmp/TASK/plan.json --output target/tmp/TASK/run
python3 bench/qualify.py report target/tmp/TASK/plan.json --output target/tmp/TASK/run
```

`plan` hashes each actual binary, harness, fixture and numerical-reference file
once and performs no native calls. It also binds the maintained runner, observer
and scientific checking modules. Each `run` verifies those files once at startup
and uses lightweight drift checks during execution; `report` requires matching
source identities and verifies its checking modules. This does not recursively
check old selections, Python environments,
generator executables or receipt chains. A traced warmup checks independent
numerical truth, typed scope certificates and setup/iteration/memory contracts.
Every timed call must reproduce that artifact/cell's exact complete output
fingerprint. Cross-worker comparisons require scientific agreement and stable
discrete decisions. All checks run outside the complete native-call timer.

The fixed run budget includes input verification, oracle and cross-worker checks,
output writes and observer shutdown. A main-thread deadline interrupts overdue
work and reaps owned native children; bounded observer cleanup finishes safely.
Cleanup overhead is recorded separately on failure. Budget exhaustion is terminal
and cannot be resumed as an operational repair. A delayed interrupt can never
make an over-budget run complete. Ordinary non-owned process CPU/RSS/activity
summaries are retained as noise observations without adding exclusion rules.

Prepare a new full input set only when needed; unchanged current inputs and valid
science may be reused after an operational repair. [`routing_cases.py`](routing_cases.py)
requires an explicit seed and generates all routing families in one invocation:

```sh
python3 bench/routing_cases.py --seed 123456 --output target/tmp/TASK/inputs \
  --source no-fe=NO_FE.case --source one-fe=ONE_FE.case --source slopes=SLOPES.case \
  --low-saved LOW_SAVED.case
```

The imported cases and saved effects need independent references. The generator
is the built `fereg-oracle` `replay_large` example by default; `--generator` selects
another built copy. `--small` produces tooling-smoke fixtures only.
[`routing_oracle.py`](routing_oracle.py) retains the independent categorical
mathematics. Generated inputs alone establish no scientific or timing result.

### SCC execution and timing

Linux runs use [`qualify.sge`](qualify.sge). Load the required Python environment
before submission. Set `TASKS` to the plan's `shards * replicas`; one array task
runs one pair, with all worker counts for each assigned fixture:

```sh
qsub -t "1-$TASKS" bench/qualify.sge target/tmp/TASK/plan.json target/tmp/TASK/run
```

The wrapper reserves a whole 28-core node with `omp28` and `num_proc=28`.
[`qualify_scc.py`](qualify_scc.py) verifies scheduler reservation, process activity
and CPU affinity throughout collection. A slot request alone cannot prove
isolation. Each task compares all arms on its own node; tasks never split a
candidate/reference pair across hosts. The plan limits external numerical
libraries to one thread and sets native worker counts explicitly.

After completion, collect `qacct -j JOB_ID -t TASK_ID` output for every task into
`ACCOUNTING/JOB_ID.TASK_ID.txt`, then combine the run:

```sh
python3 bench/qualify.py report target/tmp/TASK/plan.json \
  --output target/tmp/TASK/run --accounting ACCOUNTING
```

The fixed block order is a randomized arm permutation followed by its reverse,
with balanced orientations. Identical binary/request combinations share calls,
while retaining their separate original/incumbent roles in the report. Every
block includes two complete solves per distinct execution arm. Warmups are fixed;
there is no automatic pilot, retuning or increased sample budget.

For each protected reference, let B and N be the geometric means of its and the
candidate's two calls. The tested score is `N - B - max(.05 * B, .001)`. Acceptance
requires its one-sided alpha 0.025 order-statistic upper bound to be nonpositive.
Linux scores are first aggregated within physical host; many blocks or allocations
on one host do not create independent hosts. An unattainable bound is inconclusive.
When `min_distinct_hosts_per_cell` is present, short and fully qualified hostnames
are normalized without regard to case. Reports retain every observation and the
host-grouped bounds, list the distinct hosts for each comparison, and fail closed
if any cell/reference misses the frozen minimum. Twelve hosts across the whole
panel do not satisfy a twelve-host requirement for each cell. This count does
not establish statistical independence; the isolation and observation evidence
remains required. Omitting the field preserves historical report semantics.
Mac uses serial mirrored blocks with recorded host activity. These bounds assume
independent unit scores with a common median and make no simultaneous-coverage
claim. Ratios describe effect size; minima cannot establish acceptance.

Mac qualification does not require a completely quiet desktop. The passive
observer records ordinary CPU/RSS activity, including a verified `codex app-server`
role, without rejecting observations because of that activity. The role requires
the OS executable identity and an unambiguous first subcommand; an argument that
merely mentions `app-server` does not qualify. The server is not an owned process:
its tool children, `codex exec`, builds, tests and estimators remain classified.
New Mac plans use the practical cell policy from their first run. The policy
flags substantial sampled activity: at least
one CPU-second in a valid interval no longer than 7.5 seconds, or a known process
runnable at 20% CPU or more. Lesser activity stays recorded noise, including the
earlier 70 ms tool-runtime burst. A flag affects only its overlapping cell attempt.
The older strict plans keep their original policy and failed outcomes.
Fresh plans permit at most one replacement per affected cell and four replaced
cells overall, after two clear sampled intervals within 120 seconds. Their call
cap includes all distinct execution arms, warmups and discarded attempts; the
whole-run budget remains at most four hours. These plans cannot reset their
budgets through resume, continuation or recovery entrypoints. Historical recovery
plans retain their separate prior-evidence bindings and accounting.
Unknown desktop activity remains recorded noise. Missing observations,
artifact drift, native failures and scientific failures retain their failure
semantics. Python executable-name matching handles macOS's capitalized name,
but still requires a known test module or repository script. The SCC reservation
and competing-compute thresholds are unchanged.

Every attempt keeps all its timings, including slow observations. Balanced
randomized order reduces order effects; the confidence bound, rather than a CPU
quietness threshold, determines whether noisy results support nonregression.
Mirroring does not prove independence or remove session-wide effects. Report
serial dependence and the conditional coverage assumption honestly; inconclusive
evidence cannot trigger more samples. Process evidence retains PID/birth,
executable, relevant parsed desktop role, activity and an argv hash, not arbitrary
argument values. This supports the recorded role decision without claiming a
complete audit of unsampled processes.

### Results, repair and cleanup

`summary.json` combines coverage, protected comparisons, uncertainty and failures.
`smoke_complete` is a tooling result, never qualification. A native/scientific
failure, missing evidence or inconclusive required comparison keeps the incumbent
and requires discussion with Johannes. The tools never abandon the candidate,
advance to two-FE, activate a default or launch another round automatically.

Diagnose operational failures and attempt bounded repairs. `run --resume` is
limited to unchanged plans on the same host. Completed Mac cells can be reused
only with valid original host observations; SCC continuation after completed
timing cells requires discussion and reviewed accounting.
Ordinary resume does not replace a partially timed failure. The later user-approved
Mac recovery below permits a bounded retry of a specifically interfered cell.
Preserve actual failure details, and discuss an unresolved repair before abandoning
the candidate or changing the next step. Already valid science and unchanged
current inputs are reusable; a timing-setup failure does not require replaying
old A/B stages or generating new inputs.

After a reviewed operational repair and direction to continue, `continue-plan`
can freeze one Mac continuation using only untouched block slots. It binds the
immediate failed plan/result, consumes every started slot (including a partial
block), preserves original orders and all artifact/input bindings, and refreshes
execution-source identities. It never admits old failed observations into the
new acceptance calculation. Fresh session warmups count against the original
total call cap; elapsed execution is deducted from the original wall budget.
It rejects recursive continuations, native/scientific failures, insufficient
untouched blocks and any increased total call budget. This is a prospective
amendment, not ordinary `run --resume` or a new full panel. Review the exact
remaining schedule before launching it; evaluate it once after completion.

```sh
python3 bench/qualify.py continue-plan PLAN.json RUN/run-0-0/results.json \
  --output REMAINDER_PLAN.json
python3 bench/qualify.py run REMAINDER_PLAN.json --output target/tmp/TASK/mac-remainder
```

The new plan stays beside the original plan so relative input bindings do not
change. The output directory is new; the original failed output is immutable.

Johannes subsequently authorized practical Mac recovery with local retries and
Linux as the primary performance test. This supersedes the earlier restriction
to one untouched-slot continuation. Preserve completed cells under their original
source, science and per-cell observation evidence; the historical run remains
failed. A flat recovery manifest directly binds the immediate plan, result, raw
host observations and retained scientific snapshots. It does not follow older
continuation references. An affected partial cell is discarded as a whole and
rerun with the same candidate, inputs, warmups, balanced orders and sample count.
Unstarted cells run normally; the entire battery is never restarted.

```sh
python3 bench/qualify.py recovery-plan REMAINDER_PLAN.json REMAINDER_RUN/run-0-0/results.json \
  --host REMAINDER_RUN/run-0-0/host-0/host-observations.jsonl \
  --review INDEPENDENT_FAILURE_REVIEW.json --output RECOVERY_PLAN.json
python3 bench/qualify.py run RECOVERY_PLAN.json --output target/tmp/TASK/mac-recovery
python3 bench/qualify.py report RECOVERY_PLAN.json --output target/tmp/TASK/mac-recovery
```

Create the recovery plan beside the immediate prior plan, review it before any
native call, and run the report once after terminal validation. The independent
failure review must bind the exact prior plan, result and observation file; its
scientific checks and limitations remain part of the new plan.

New Mac interference events are recorded without aborting the entire job.
Before accepting a cell, synchronously observe its tail and check its whole span
from first warmup through last timed call. Overlap with an external activity flag
invalidates that attempt regardless of its scores. It may run once more after two
clear sampled intervals, waiting at most 120 seconds. Freeze at most one retry
per cell and four retried cells total. Count every native call, including warmups
and discarded attempts, against the frozen cap; the whole-run deadline includes
waits, preparation, science, observer shutdown and final writes. Stop on persistent
interference or exhausted caps. Observation faults, drift, native timeouts and
scientific failures keep their original failure semantics and cannot be retried
as interference. Regression or inconclusive performance never triggers sampling.

The CPU screen is deliberately practical. Small activity can still influence
timings, a tail sample can conservatively attribute post-solve activity to the
cell, and sampling can miss brief work. Forced tail samples split CPU-delta
intervals; macOS `%CPU` can reflect earlier activity. Retain these limitations,
one session per eligible cell, and conditional independence/common-median bounds.
The first externally eligible attempt supplies the cell's observations; do not
choose whichever attempt is faster. Report retained cells, every discarded
attempt, retries and all actual call/time accounting. Reuse the completed Linux
report without rerunning it against subsequently changed checker sources.

All output belongs under one `target/tmp/TASK/` directory. The runner retains
compact results and first-call output snapshots, removing repeated successful
native dumps. Keep only the current workstream's artifacts, inputs and evidence
while it remains unfinished. After committing compact summaries, remove completed
captures, duplicate archives, superseded build trees and old harness copies.
Historical source and outcomes remain available in Git; ignored paths are not
retained evidence.

The [2026-10-14 routing observations](results/solver-routing-20261014.json) and
[validation history](../docs/VALIDATION.md#routing-timing-outcome-2026-10-14)
retain their original failed, incomplete or inconclusive status. The replacement
suite neither rescues those attempts nor requires rerunning their retired stages.
