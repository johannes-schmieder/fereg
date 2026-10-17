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

The output directory must be new. Runtime/frontend hashes are checked throughout;
inputs are hash-bound and round-trip verified. Failed commands and partial runs
remain visible. Comparisons retain coefficient, covariance, scalar, exact sample,
and requested-output checks, with a bounded immutable-reference cache. Timings use
Stata's timer and report medians by workload; they cannot establish small changes
below its resolution. Dedicated performance decisions require a separately declared
bounded protocol under `docs/development_workflow.md`.

Generated captures belong under `target/tmp/` and are deleted after inspection.
Historical campaigns and the legacy comparator are available only in Git history.

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

### Controlled paired blocks

[`time_blocks.py`](time_blocks.py) implements the prospective controlled protocol
with Python 3.11 or later. It freezes complete-solve ABBA/BAAB blocks, balances
orientations, shuffles workloads each round, and checkpoints every complete or
failed attempt. Exact comparisons bind a prior complete scientific qualification;
routing comparisons bind independent witnesses and each arm's qualified
scientific fingerprint. A timeout, scientific failure or changed binding stops
collection and cannot be excused by a noisy host. Freeze a new output directory
and finite attempt budget; do not repeat a failed experiment into another path.

[`block_host.py`](block_host.py) wraps collection and records process identities,
CPU activity and load. It leaves ordinary desktop activity in the sample and
never pauses unrelated processes. A frozen classifier may exclude a whole block
for sampled known competing computation. Exclusions consume attempts. The
validator recomputes classification from raw observations and checks source,
policy, events, manifest and record hashes. Missing coverage is inconclusive.
This sampled observer cannot prove that no short-lived process ran, and does
not claim sleep or memory-pressure detection.

```sh
python3 bench/time_blocks.py seal --spec SPEC.json --output MANIFEST.json
python3 bench/block_host.py --manifest MANIFEST.json --policy POLICY.json \
  --events RUN/host-events.jsonl --records RUN/records.json --output HOST \
  -- python3 bench/time_blocks.py collect --manifest MANIFEST.json --output RUN
python3 bench/time_blocks.py analyze --manifest MANIFEST.json \
  --records RUN/records.json --monitor-receipt HOST/host-receipt.json \
  --output REPORT.json
```

All generated paths belong under one `target/tmp/<task>/` directory; `RUN` and
`HOST` must be separate fresh directories. Save the exact observer `POLICY`
object before sealing the specification. The specification binds the harness,
each comparison's two artifacts and prepared inputs, prior science, prospective
design declaration, monitor source and policy. Its design declares an even
attempt count, warmups, seed, per-call and aggregate budgets, session identity,
block analysis unit and upper-tail alpha 1/40.

Schema 2 groups execution arms and the complete comparison graph by cell. A
block executes one frozen permutation and its reverse, requiring only two calls
per distinct arm. Counterbalanced permutations give every pair balanced
ABBA/BAAB order across rounds. Shared calls are stored once; each pair receives
one score per complete block. Every comparison shares the block's scientific
success and host eligibility, including when an unrelated arm fails. Two-arm
cells provide ordinary paired confirmation. Longer blocks change cache and
thermal exposure; a two-arm A/A pilot does not establish their precision or
independence.

Protected acceptance tests median `N - B - max(.05 * B, .001)` using block geometric means
and the attainable order-statistic upper bound. Ratios, means, upper tails,
orientation contrasts and lag-one diagnostics are reported separately. Minima
are descriptive only. Statistical coverage assumes independent block scores
with a common median; a small A/A pilot cannot prove those assumptions. The
report therefore separates a conditional statistical pass from release
qualification, which also requires reviewed experimental assumptions, complete
workload coverage and the relevant scientific/deployment gates. Schema 2 also
supports the original internal-frontier allowance, `max(.20 * B, .001)`, and
reports normalized-regret bounds. A reference declared protected by its
scientific manifest cannot be relabeled internal to relax its allowance.

[`routing_discovery.py`](routing_discovery.py) is a separate Linux/SCC discovery
screen. It freezes the twelve-policy grid and baseline on the 42 A cells,
requires fresh Linux scientific prerequisites, then combines three fixed
three-repetition rounds from the maintained routing producer. It reports point
estimates and a shortlist only. Its frozen external isolation validator must
replay scheduler, placement, process and accounting evidence; a slot request or
asserted isolation flag is insufficient. No B/C input bodies are opened, no Mac
unavailability outcome is imported, and the result cannot satisfy the legacy
Mac B/C or public-default deployment gates.

[`scc_routing_host.py`](scc_routing_host.py) supplies the discovery collector and
raw-evidence validator. It checks an exact 28-core single-host reservation,
the global cross-queue resource quota, physical CPU placement, fixed library
limits and the scheduler wall limit before collection. Separate process and
scheduler samplers retain evidence throughout the run. After accounting becomes
available, `finalize` derives an isolation receipt from those observations;
`validate-stdin` independently replays its facts without live scheduler access.
Failed admission or missing evidence cannot yield a discovery shortlist.

[`routing_blocks.py`](routing_blocks.py) binds a reviewed SCC shortlist to the
original Mac B panel and a new twelve-round mirrored design. Fresh Mac scientific
prerequisites are generated through `routing_run.py --prerequisites-only
--qualification-context CONTEXT`, without inventing a legacy A decision. The
context derives complete candidate/reference coverage, preserves unavailable
optional comparisons, and refuses premature C access. Its `decide` function
recomputes the complete comparison graph from raw observations and returns one
conditional B finalist or no qualifying policy. Public activation and C
use the separate reviewed [`routing_block_deployment.py`](routing_block_deployment.py)
transition; the old deployment contract is not silently reinterpreted.

Freeze its declaration before B. After an actual nonbaseline B finalist, source
review and fresh correctness/build checks admit only original A/B equivalence
collection through the existing prerequisite runner. Each cell has two calls
to the selected hidden policy and two to uninjected public Auto. Exact science,
solver signatures and independent witnesses must agree; resource evidence
identifies the traced prerequisite rather than claiming every repeat was traced.
Only complete checked A/B records admit C. C keeps all original cells and
comparators, protects both the original baseline and selected hidden artifact,
and uses 24 separate two-arm blocks per required comparison. Missing, failed or
inconclusive evidence prevents confirmation; there is no fallback or continuation.
The final result remains conditional and requires operational review before
promotion. Evidence revalidation happens outside native-call timing.

The separate prospective `portfolio` experiment in `routing.py` admits exactly
baseline and `portfolio-v1`. The latter uses `routing_portfolio_v1`: exactly two
categorical effects first apply a charged dimension-only sufficient bound for
direct factorization, preserving the four-iteration structural cap, full setup
cap and joint memory admission. Direct and incumbent-CMG are exclusive; a
declined direct preflight retains its charge and permits one synchronous CMG
attempt at iteration 16, cutoff 5, one sampled tree and seed zero. Three or
more categorical effects try at most four MAP sweeps with strict private Auto
certification and an original-slot seed for incumbent LSMR, with no general-FE
CMG handoff. Earlier isolated policies and their evidence remain distinct.
Fresh A/B/C bodies, the full standard and high-degree/extended coverage, actual
sampling science, the existing scientific gates and fixed stopping rules remain
required. This finite hypothesis does not reopen a completed campaign or promote
the candidate into public Auto.

For this hypothesis, discovery uses the separate closed schema-2 path in
`routing_parallel.py`. Three distinct reserved nodes each run fresh two-call
science prerequisites and one complete 42-cell round. Each available arm has
two science calls, one traced call, one warmup and three measured solves:
at most 2,520 calls per allocation and 7,560 overall. Science, timing and cleanup
share a fixed 14,400-second allocation budget; terminal accounting must also
fit that bound. No global science proof, old window, retry or replacement enters
this campaign. Require all three complete terminal records, independent science,
strict isolation and full aggregate replay before reading a shortlist.

The frozen descriptive screen rejects the portfolio if any baseline comparison
violates its allowance, or its worst available internal normalized comparison
exceeds one; equality passes this screen. Missing valid internal comparisons
block screening. A rejection retains the complete ranking and stops before Mac
B. Passing the screen only admits fresh Mac B12 selection, reviewed public/hidden
equivalence and untouched C24 confirmation, with the existing tolerances,
allowance and stopping rules. It does not establish a performance claim.

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

## Finite routing policy qualification

The [2026-10-14 retained observations](results/solver-routing-20261014.json)
leave public adoption blocked: standard timing is scientifically exact with 18
passes and two inconclusive cells; ablation timings are exploratory because of
overlapping host activity; policy A was interrupted and cannot select a policy.
No B/C selection follows from those observations. Johannes subsequently
authorized bounded recovery and future retries; each attempt still needs frozen
bounds, retained observations and appropriate repeated-look accounting. See the
[validation explanation](../docs/VALIDATION.md#routing-timing-outcome-2026-10-14)
before interpreting these measurements or proposing a new bounded qualification.

[`routing.py`](routing.py) supplies the policy manifest, sanctioned developer
injection mapping and A/B/C selector. It reuses `time_exact.compare`: paired
median ratios, 10,000 fixed-seed bootstrap draws and a nominal 95% interval **per
comparison**, with no claim of simultaneous coverage. It does not launch timing
jobs or replace independent scientific checks. No routing policy is qualified by
adding the tooling, preparing inputs or passing its unit tests.

The initial candidates are `baseline` and `h4-k0`, `h8-k0`, `h16-k0`, `h32-k0`.
Candidate labels map to `inject=routing_h4_k0`, etc.; all request `solver=auto`.
The baseline uses its immutable historical artifact without injection. The
candidate artifact must be identical across policies. Degree cutoff 5 and seed 0
are fixed. Add all four checkpoint variants of `k1` or `k2` only after binding the
corresponding independent scientific qualification hash in the manifest. Sampled
candidates additionally require high-degree mixed, weak and stress coverage in
every panel; `routing_cases.py --sampled` appends these three existing fixtures.

Prepare framed arm inputs without changing the numeric/model body:

```sh
python3 bench/routing.py prepare --source INPUT.case --arm h16-k0 --threads 4 \
  --output target/tmp/solver-routing/prepared-h16-k0.case
```

Existing developer-injected inputs are rejected. Forced policy support belongs
to the runtime's sanctioned injection surface; unknown injections must fail,
never silently act as auto. This preparation command is not evidence that a
runtime implements or exercised an arm.

[`routing_cases.py`](routing_cases.py) prepares twelve sparse families: no FE,
one FE, slopes, dense two-FE, small retained dimension, a 20-level third FE, four
and eight FEs, probability weights, weak connectivity, repeated cells and 16
regressors. The latter three reuse the CMG-Schur generators; the categorical
cases reuse `replay_large`. The generator's inability to create zero/one FE and
slopes is explicit: supply separate inputs, and their canonical absorbed-term
tables are checked. There is no Cartesian product of every feature.

```sh
python3 bench/routing_cases.py --panel A \
  --source no-fe=NO_FE_A.case --source one-fe=ONE_FE_A.case --source slopes=SLOPES_A.case \
  --output target/tmp/solver-routing/cases-a
```

Repeat preparation for B/C with their own imported cases. Each panel has a
separate fixed seed. `panel.json` gives cells and eligible explicit comparators;
those declarations still require scientific verification. The selector rejects
case or numeric/model-body hashes shared across A/B/C, including imported inputs.
Changing only a worker count or solver option cannot create a new holdout.
Later experiments may predeclare distinct unsigned 64-bit `--seed` values;
the original panel defaults remain unchanged. They also need distinct imported
inputs. A different seed is only a construction choice: the full manifest's body
hash checks still establish disjointness.
`--extended --low-saved SAVED_EFFECTS.case` adds sixteen FEs, disconnected
components, a nearly confounded third FE, probability weights spanning
`exp(-8)` to `exp(8)`, exactly inactive columns and an imported saved-effect case
with `memory=low`. Imported replay oracle JSON sidecars are copied and hash-bound
for later verification; preparation does not run them. PPML, real graphs and
large slope-heavy problems still require separately declared inputs and
scientific qualification before those scopes are promoted.

The unsealed tournament specification is a JSON object with:

- `schema: 2`, `experiment_kind: "routing"` (the default), `warmups: 1`, `repetitions: {"A": 3, "B": 7, "C": 7}`.
- `failure_policy` exactly equals `routing.FAILURE_POLICY`: availability protocol version 1,
  120 seconds per call, four hours per panel, no retries and an all-success
  reliability gate. These bounds are frozen before the panel.
- `harness_sha256`, and `arms` mapping names to `{sha256, policy, role}`.
  Roles are `required_reference` for protections, `candidate` for other policy
  arms and `optional_comparator` for every explicit comparator. For candidate
  route arms, `policy` is exactly `routing.policy(name)`; baseline's policy is
  `null`. Explicit comparator arms have `sha256` and may name `solver` when the
  arm name differs from the solver. CMG-Schur comparators may declare
  `cmgsamples: 0`, `1` or `2`.
- `candidates`, containing the complete permitted grid and `baseline`;
  `protections: ["baseline"]`, or `["baseline", "incumbent"]` to also protect a
  frozen stage incumbent. An incumbent may declare an exact `policy` object to
  retain its forced preset; null/absent means its artifact's uninjected auto.
  `sampled_qualification` maps qualified counts `"1"`
  and/or `"2"` to evidence hashes; omit it for retained stars only.
- `panels`, mapping A/B/C to cell arrays. Each cell has `id`, `case_sha256`, `body_sha256`,
  `family`, `coverage` tags, `threads` and `comparators`. The initial required
  coverage tags are the twelve families above; every panel must include them.
  A case may recur at different worker counts within its panel. Optional positive
  `memory_limit_bytes` declares the native memory cap; inherited memory-cap
  environment settings are cleared.

Freeze before measuring A. The output seal is a canonical-content SHA-256, not a
signature or a substitute for access control. Every output is created
exclusively; the CLI refuses overwrites.

```sh
python3 bench/routing.py freeze --spec SPEC.json \
  --output target/tmp/solver-routing/frozen.json
python3 bench/routing.py select --manifest target/tmp/solver-routing/frozen.json \
  --stage A --records RECORDS_A.json --output target/tmp/solver-routing/selected-a.json
python3 bench/routing.py select --manifest target/tmp/solver-routing/frozen.json \
  --stage B --records RECORDS_B.json --previous target/tmp/solver-routing/selected-a.json \
  --output target/tmp/solver-routing/selected-b.json
python3 bench/routing.py select --manifest target/tmp/solver-routing/frozen.json \
  --stage C --records RECORDS_C.json --previous target/tmp/solver-routing/selected-b.json \
  --output target/tmp/solver-routing/qualified-c.json
```

Measurements must bind the frozen `manifest_sha256`, `stage`,
`selection_sha256` (null for A; the preceding decision's seal hash for B/C),
`harness_sha256`, `artifacts` mapping every required arm to its hash, and `cases`
mapping every cell ID to its input hash. The `records` array contains one row
per required cell/arm/repetition, including warmup `-1`, then `0..2` for A or
`0..6` for B/C. Each passing row includes positive finite `seconds`, exact-repeat
`science_sha256`, independent-check `witness_sha256`, `certificate_pass: true`,
`science_pass: true`, and `resource_checks` with passed `iteration_caps`,
`memory_caps` and `setup_caps`. Legacy/explicit arms may mark setup caps
`not_applicable`. Optional `action_resources` retain per-action used/limit integer
pairs for `iterations`, `setup_work` and `memory_bytes`; any overrun invalidates
the row. Scientific hashes may differ between artifacts but must repeat exactly
within every cell/arm, including warmup. Witness hashes must remain fixed.

Do not synthesize passing flags from convergence alone. Native diagnostics,
independent witnesses, scoped replay and fixed-artifact repeats must substantiate
the record. This legacy tournament uses its declared rotating paired order;
new discovery/confirmation campaigns must freeze the controlled protocol and
their analysis separately, without relabeling historical outcomes. Every planned
call remains recorded. A terminal call can have
`status: "unavailable"` only when actual native return code 459 and a predeclared
solver-specific exhaustion diagnostic establish iteration exhaustion, or when a
dispatched call reaches the full prespecified timeout. Availability policy v2 also
recognizes only the explicit CMG-Schur tuple `430`, label `nonconvergence`, and
message `cmgschur did not certify within its iteration budget`. Its distinct
`certification_nonconvergence` reason preserves the source semantics: this bounded
solver can return it after exhausting its budget, PCG recurrence breakdown
(including nonfinite recurrence values), or final
certificate rejection after its bounded correction. It is no successful scientific
result and does not assert that every native iteration was consumed. The exact
tuple is scoped to the optional `cmgschur` family; other 430 errors still block.
Its log/result hashes,
observed resource fields and any available resource trace remain attached.
Subsequent calls for that arm are `not_run` and bind the original failure row by
hash; the producer never retries a failed arm. Infrastructure or identity errors,
short aggregate-budget timeouts, unknown native errors, wrong answers, missing
truth, failed resources and nonexact repeats are `blocked`, not availability.
Missing rows or unbound skipped rows also block; no case is dropped.

Required baseline/incumbent failures block the panel's nonregression claim.
Candidate failures exclude that candidate. An optional comparator's established
unavailability does not invalidate a successful candidate's independently checked
science, but leaves their runtime comparison undefined. All prerequisite,
warmup and measured calls must succeed to identify a cell/arm runtime. A failure
after some successful repetitions therefore invalidates that arm's entire runtime
sample; its successful repetitions are retained, never filtered into an estimate.
An optional comparator's blocked or unresolved evidence blocks the whole panel.

Numeric frontier scores use only complete joint-success comparisons. Every
omitted runtime comparison remains visible as availability dominance with
`runtime_identified: false`, no ratio, confidence interval or normalized regret,
and mandatory per-policy frontier coverage. If no internal runtime comparison is
identified, the internal score is null, not zero; its ranking key ties with other
undefined scores and the existing conservative tie rules apply. All performance
claims are conditional on the explicitly listed finite comparison set. Schema-1
manifests or observations cannot be reinterpreted under this prospective policy.
Unknown arms, duplicated observations and changed identities reject the input.

Normalization uses excess regret `(ratio - 1) / max(relative allowance,
1 ms / reference median)`, with negative values retained for speedups. A
shortlists up to four complete policies by baseline violations, worst normalized internal median
regret, geometric-mean baseline ratio, then conservative ties. B requires every
baseline comparison's bootstrap upper bound to fit max(5%, 1 ms), and every
joint-success explicit comparator to fit max(20%, 1 ms). Among passing candidates it
minimizes the worst normalized internal upper bound; ties prefer baseline,
retained stars, larger checkpoint, then fewer trees. C measures only the policy
frozen by B, protection arms and explicit comparators. It accepts or rejects
that policy, never picks a runner-up. No command automatically continues or
resamples an inconclusive holdout. A failure is not qualification of the retained
baseline against the internal frontier.

[`routing_run.py`](routing_run.py) is the maintained producer for these records.
It reuses the CMG-Schur native process, result reader, dense-oracle check,
full-output scientific comparison and rotating arm order. Provide an already
built timing harness whose hash matches the frozen manifest; it never builds
while measuring. Arm-to-artifact and cell-to-case bindings must exactly cover the
selected stage. `--previous` is required for B/C and binds the preceding selection
before any native calls.

```sh
python3 bench/routing_run.py --manifest target/tmp/solver-routing/frozen.json \
  --stage A --harness TIMING_HARNESS \
  --plugin baseline=BASELINE_PLUGIN --plugin h4-k0=CANDIDATE_PLUGIN \
  --plugin h8-k0=CANDIDATE_PLUGIN --plugin h16-k0=CANDIDATE_PLUGIN \
  --plugin h32-k0=CANDIDATE_PLUGIN --plugin lsmr=CANDIDATE_PLUGIN \
  --case CELL=INPUT.case --witness CELL=EVIDENCE.json \
  --output target/tmp/solver-routing/measured-a
```

Repeat `--case`/`--witness` for every declared cell and add each explicit comparator
needed by any cell. The distinct `it-coefficients` arm uses `solver: "it"`
and exactly `injection: "it_coefficients"`; `it` continues to request uninjected
public IT. No other comparator injection is accepted. Compact IT requires
declared `categorical_intercepts: true` and positive `absorbed_terms`, which the
producer verifies against the canonical model table. `routing_cases.py --compact-it` prospectively adds this comparator on eligible cells, including
one FE, and retains public IT wherever it was already declared. It never
rewrites a frozen panel or reinterprets the historical IT experiments.

Supported comparator solver values are `map`, `it`, `lsmr`,
`cmg`, `cmgschur`, `diagonal`, `cells` and `direct`; aliases declare an arm's
`solver` field. CMG-Schur comparators use cutoff 5, the declared sample count
(default 0) and seed 0 when sampling. Their eligibility and scientific evidence
must be established for each cell. Candidate arms use their injected auto policy;
the baseline uses uninjected auto, while an incumbent preserves its declared policy. Stage A allows three
measured repetitions without changing `time_exact.py`'s existing timing CLI.

An evidence JSON file must have `schema: 2`, `status: "checked"`, the frozen
`stage` and `selection_sha256`, per-arm `arm_outcomes`, the cell's
`case_sha256` and `body_sha256`, `manifest_sha256`, `cell_id`, `threads`,
and `arms` mapping every required arm to exactly its manifest configuration
(including its artifact hash and policy, where present). Each of `independent_witness`, `scoped_replay` and
`resource_contract` is `{ "path": "relative-or-absolute-proof-file",
"sha256": "..." }`. These must be actual passing evidence from the preceding
scientific and resource checks, not placeholders; all files and hashes are
verified and frozen for the panel. The sealed prerequisite proof is revalidated arm by arm against the same
availability rules. Every native success also requires an executable independent
case target; a generic external proof cannot substitute for it. Small case sidecars containing the existing dense
oracle additionally receive that oracle check on every native result. Copied
independent replay references receive the existing `scientific_errors` check on
every result, including saved-effect channels when requested. These imported
checks are bounded to one million input cells. Every result also receives the
existing full scientific comparison with the baseline prerequisite. That
cross-artifact comparison is not presented as an independent mathematical oracle.

Before warmup, the producer makes one **untimed traced prerequisite** per
cell/arm. Candidate `routing_action` traces must report certified actions and
actual used/limit counts for native iterations, optional setup work and admitted
memory. The trace's `preset` must equal the requested injection, and any Schur
handoff's checkpoint/sample values must agree with it. Any missing, malformed or over-limit candidate action blocks dependent
timings. A zero-FE case may bypass projection actions entirely; accepting that
absence requires its canonical absorbed-term count to be zero **and** another
bound `no_fe_exact_path` proof in the evidence JSON. Legacy comparators lacking
new counters use the declared resource-contract evidence; they never fabricate
candidate setup counters.

Both prerequisites and timings clear inherited diagnostic/route toggles
(`FEREG_PHASE_TIMING`, `FEREG_CMG_ORDINARY`, memory-audit/tracing controls and
`FEREG_MEMORY_LIMIT_BYTES`); only a manifest-declared memory cap is re-enabled.
Measured native calls explicitly disable tracing, avoiding additional candidate
logging in the timed region. Every warmup and measured run must exactly match
its traced prerequisite's science and solver signature and pass the full
scientific comparison. Records retain the actual prerequisite counters,
diagnostic digest, input variant hash and paired-order position. A failed
prerequisite blocks only dependent calls and leaves explicit failed/skipped rows;
a failed immutable identity check invalidates the whole run. The selector rejects
`running`/`failed` producer files even if some timing rows are present. A total
budget expiration retains unrun cells; it never reduces the denominator or
starts another panel. The frozen bounds are 120 seconds per call and four hours for
the declared panel; CLI values must match the manifest.

For independent kernel experiments, add explicit comparator arms with solver
values `diagonal`, `cells`, `cells_v2` or `direct` and use `routing_run.py --ablation-only` on
panel A. This prepares `routing_diagonal`, `routing_cells` and `routing_direct`
injections (plus `routing_cells_v2` for the compact parallel prototype), collecting only baseline/protection/comparator arms. All science,
resource and repeat guards still apply. The output retains every arm, including
failed and unresolved calls, and reports runtime comparisons only for
complete joint-success cells against the baseline.
Its purpose is `ablation_only`; the selector refuses it for policy promotion.
The finite h/k candidate grid is unchanged. This is the pre-tournament route for
independently qualifying one kernel change at a time.

A smaller manifest may set `purpose: "ablation_only"`, contain only panel A,
and use `candidates: ["baseline"]` with the needed protection/comparator arms.
It retains the same fixed repetition declaration and all identity/science/resource
requirements, but does not require the full tournament's family coverage or h/k
grid. Collect it with `--ablation-only`; neither its manifest nor its observations
can be passed to the tournament selector.

Separate prefix and overlap experiments use the same producer, availability
rules, paired statistic and fixed A/B/C repetition counts. They do not expand
the h/k grid. Each later manifest includes `prior_manifests`, an array of sealed
preceding manifests, including any reserved earlier panels. All current body
hashes must be disjoint from that complete preceding chain and from one another
across A/B/C. This validates metadata only; it does not open earlier datasets.
Fresh scientific witnesses are required when each stage becomes eligible.
The protocol owner must include the complete preceding manifest history; a seal
checks integrity, not authorship or whether a prior experiment was omitted.
Every experiment keeps the baseline artifact uninjected.

For `experiment_kind: "prefix"`, candidates are exactly `baseline`,
`prefix-none`, `prefix-map4` and `prefix-it4`; protections are `baseline` and
`incumbent`, both uninjected. The three challengers use one artifact and their exact
`routing.policy(name)` descriptors. They replace the old three-FE shortcut:
none runs no pilot, MAP4 runs four sweeps, and IT4 runs four accelerated steps
priced as eight sweeps. No prefix is stacked. The optional CMG handoff is disabled
for this isolated experiment; the remaining general router is identical across
the three arms. Eligible cells have at least three categorical-intercept terms,
verified from the model; all five declared many-FE coverage families must appear.
Conservative ties prefer baseline, none, MAP4, then IT4. A result qualifies this
fixed prefix experiment only; a later composition with h/k needs its own frozen
artifact, manifest and complete qualification.

For `experiment_kind: "overlap"`, selectable arms are exactly `serial` and
`overlap`; protections are `baseline` and `serial`. The serial arm's policy is
one already qualified finite h/k policy; overlap uses
`routing.policy("overlap-hH-kK")` for the same H and K. Both must use exactly the
qualified serial artifact, while baseline retains its immutable artifact.
H=4 is ineligible. `serial_qualification` contains the preceding sealed routing
`manifest` and complete sealed C `decision`, with its record and B-selection
hashes. A B winner or an incomplete/failed C result is insufficient. The serial
manifest must also be in the freshness chain, and its sampling evidence remains
bound. Every panel includes a multiple-worker cell; one-worker fallback cells
can remain in the declared global coverage. Serial wins ties. C tests only the
frozen B choice; failure retains the previously qualified serial policy without
running a runner-up or claiming that the failed overlap experiment qualified.
No kernel, prefix or overlap Cartesian product is accepted. PPML performance
qualification remains separate and is not supported by this OLS producer.

Before timing, the same producer collects scientific prerequisites for the frozen stage:

```sh
python3 bench/routing_run.py --manifest FROZEN.json --stage A --prerequisites-only \
  --harness HARNESS --plugin ARM=PLUGIN --case CELL=INPUT.case \
  --witness CELL=PREREQUISITE_INPUT.json --output target/tmp/solver-routing/science-a
```

Repeat the bindings for every required arm/cell. This mode makes one traced
native call and one untraced exact repeat per eligible cell/arm. It produces
scientific proofs and evidence wrappers, with no timing summaries, policy
selection. B prerequisites require the frozen A shortlist, and C prerequisites
require the passing frozen B selection; both use `--previous`. Missing selection
is rejected before reading those inputs. This never grants early holdout access.
Missing independent truth or resource evidence leaves explicit blocked cells.
A wrapper records every declared arm's outcome, including excluded candidates
and established optional unavailability. It is consumable only when required
references pass and no optional comparator has blocking evidence. Successful
arms retain their full science/resource checks. Any frozen identity failure
invalidates the run and removes all generated wrappers.

The prerequisite input has `schema: 2`, `status: "ready"` and the same
case/manifest/worker/arm identity fields as an ordinary evidence wrapper. It requires a hash-bound `resource_contract`
for legacy arms. It also requires a case-specific `independent_witness`, unless
the case carries a checked dense/imported replay oracle. `scoped_replay` need
not be supplied: this mode computes it from the actual native outputs. Passing
the repository test suite or agreeing with the historical binary does not
establish independent truth for a new large case. The maintained
`inactive-columns` construction has an additional witness: actual input/model
bytes must establish exact pairwise cancellation within each worker/firm cell;
independent small-column OLS, HC1 and bipartite graph rank then give its targets.

Ready wrappers may additionally freeze `frozen_dependencies`, a list of exact
`path`/`sha256` bindings for checker sources, nested evidence, case sidecars and
local oracle files. The producer checks these before collection and includes
them in its existing identity guard throughout the run. Generated checked proofs
and wrappers retain the same list, which is revalidated before timing. This
prevents preparation from silently accepting changed local truth or tooling when
native collection later starts. Relative dependency paths resolve against the
ready wrapper and become absolute in the generated proof and checked wrapper.

[`routing_oracle.py`](routing_oracle.py) builds separate bounded categorical
references for frozen A inputs using an independently formed Schur complement
and direct SciPy factorization. It validates canonical model bindings and uses
graph gauges with a separated positive grounded spectrum for sufficient rank
admission under an explicit floating error model, including Schur formation
and projection perturbation guards. This is not an exact or interval proof;
extra aliases or ambiguous conditioning are refused. Its JSON/NPZ
reference binds the original case/body and producer hashes. When an evidence
wrapper's `independent_witness` points to this reference, the producer loads and
hash-binds both files and checks coefficients, covariance, sample/drop
decisions, fitted/residual outputs and supported rank/DoF targets on every native
call. This module supports a bounded subset of categorical models; unsupported
inputs still require their own independent witness. It never rewrites inputs or
opens the holdout.

Scoped compatibility reuses `replay_plugins.numerical_scope_indices` and
`work_scope_indices` for individual numerical/work leaves, plus each RB/RV
scalar and numerical observation output. Default-route engine IDs, requested
tolerances, certified/presence flags, DoF and all other discrete fields remain
exact. An explicit comparator additionally permits its independently
asserted requested/used solver IDs and preconditioner mapping in each solver
row. No runtime-signature field is relaxed for OLS. Science-checker source files
and their direct dependencies are hash-bound along with native artifacts.

For explicit CMG only, a successful graph projection independently requires
requested/used `7/7`, certified stop `2`, fallback `0`, and zero preconditioner
and pilot fields. Its warning count must be a finite nonnegative integer;
positive warnings require positive iterations. Fast primary absorption permits
at most one polishing warning; Low primary absorption permits at most
`1 + ceil(C / P)`, where `P` is the effective pool size and `C` is the full
`RSTATUS` row capacity. Model construction creates at most one generated column
per coefficient, and publication bounds coefficient count by that capacity.
Thus this is a conservative bound on Low's response-plus-regressor-block
projection calls, including omitted and aliased slots. It does not use retained
rank. Low final absorption permits at most one polishing warning; Fast final
`7/7` permits none. Only these independently asserted counts may differ from
baseline Auto at timing-stripped `RSOLVER` indices 9 and 23. Effective mode and
pool size remain protected `RRUNTIME` fields.

The Fast final-residual CG correction is a separate contract: requested/used
`7/4`, certified stop `2`, fallback `1`, warning count `1`, and zero
preconditioner and pilot fields. Only that tuple permits final fallback/warning
leaves 22 and 23. Its verification can complete with zero CG updates; the
positive-iteration requirement belongs only to graph-polishing warnings.
Primary CG and Low final CG remain outside this comparator contract.

Generic explicit IT on parsed slope/mixed models has a separate informational
warning contract. Both solver rows must report requested/used `5/5`, certified
stop `2`, fallback `0`, and zero preconditioner/pilot fields. Generic IT emits
one notice per projection, including initial certification at zero iterations.
Fast therefore requires primary/final counts `1/1`; Low permits primary counts
from `1` through `1 + ceil(C / P)` and requires final count `1`, using the same
validated full coefficient capacity and effective pool-size bound. Only warning
leaves 9 and 23 may differ from Auto. Categorical IT, other solver tuples and all
scientific tolerances and model decisions retain their existing checks. Raw
warning counts remain exact across repeats and comparable worker requests.

Exact repetitions retain warning counts and all solver diagnostics. Before
publishing any checked wrappers, the producer also compares asserted CMG
diagnostic tuples across complete successful worker-count pairs. Requests are
grouped by effective prepared input with only `threads` removed, arm/artifact,
and the explicit memory-limit environment. Different modes, pools, tolerances
and injections cannot become accidental peers. A mismatch blocks both cells,
retains their observations and prevents either wrapper from qualifying;
incomplete or single-worker groups explicitly make no cross-worker claim.
Failed calls retain their existing binary row columns along with metadata and
logs, with all retained artifacts hash-bound.


### Public-default deployment after serial B selection

[`routing_deployment.py`](routing_deployment.py) supplies a separate, prospective
schema for one narrow transition from a hidden serial `(h,k)` arm to public OLS
Auto. Declare and retain the amendment before any timing:

```sh
python3 bench/routing_deployment.py declare --before-any-timing \
  --manifest FROZEN.json --output target/tmp/solver-routing/deployment.json
```

The declaration binds the original selector, statistic, candidate source and
complete A/B input identities. After B, it recomputes A and B from their records
and accepts only one passing nonbaseline serial winner. Only OLS invocation
wiring in `crates/fereg-plugin/src/estimate.rs` may change; build inputs and the
selected algorithm remain fixed. Reviewers must verify the source diff and
fresh non-C qualification, including two calls per artifact on every A/B cell
with exact science and solver signatures. Explicit requests and PPML remain
unchanged. Evidence packets are hash-bound reviewed attestations, not automatic
proof that their prose claims are true.

The derived C schema preserves all original C input identities, repetitions,
failure rules, budgets, baseline and explicit comparators. It protects the
selected hidden artifact as incumbent, replacing an original incumbent only
when that incumbent is byte-for-byte the baseline configuration. The public
candidate uses uninjected `solver(auto)`. Freeze it before C access, run C once,
and retain the old default on failure or inconclusive evidence. No runner-up,
retuning, continuation or post-C production change is allowed. Chronology and
one-attempt enforcement require operational coordination; an exclusive output
path alone does not establish them.

This schema is deliberately rejected by the existing producer. Public C
execution hooks require separate implementation and verification after an
eligible B winner exists; declaring the schema does not activate the solver.

### Compact-cell native discovery

[`compact_cells.py`](compact_cells.py) is a separate bounded experiment for the
private `routing_cells_v2` backend. It compares one identical plugin artifact
under uninjected Auto, the original cells arm, diagonal-only, and cells-v2.
There is no policy tournament or public promotion. Twelve fixed synthetic
families cover weak and high-degree weak graphs, dense links, repeated links,
non-movers, hubs, disconnected components, dynamic weights, wide RHSs, exact
initial orthogonality, Low mode, and easy solves; every input has at most
100,000 rows. Worker counts are one and four.

Prepare on a compute node using already-built `replay_large`, the native timing
harness, and plugin. The native harness must admit 100,000 rows and at least
3,000,000 cells. Preparation computes the existing independent SciPy Schur
witness for every family, then freezes all inputs, prepared arm requests,
witness arrays, tools, generator, harness, and plugin hashes. The science stage
uses two complete native calls for every arm/worker/case, unchanged scientific
tolerances, exact repeats, cross-arm and cross-worker numerical/discrete checks,
and traced resource admission. A two-slot smoke runs only the first one-worker
case through all four arms and cannot authorize measurement.

Example commands, with absolute deployment paths substituted for uppercase
placeholders (create scheduler log directories before submission):

```sh
qsub -P welfgr -pe omp 4 -l h_rt=01:00:00,mem_per_core=3G \
  bench/compact_cells_scc.sh prepare --root CAMPAIGN/input \
  --generator REPLAY_LARGE --harness NATIVE_HARNESS --plugin PLUGIN
qsub -P welfgr -pe omp 2 -l h_rt=00:15:00,mem_per_core=3G \
  bench/compact_cells_scc.sh smoke --plan CAMPAIGN/input/plan.json --output CAMPAIGN/smoke
qsub -P welfgr -pe omp 4 -l h_rt=01:00:00,mem_per_core=3G \
  bench/compact_cells_scc.sh science --plan CAMPAIGN/input/plan.json --output CAMPAIGN/science
qsub -P welfgr -pe omp28 28 -l num_proc=28,h_rt=01:15:00,mem_per_core=3G \
  bench/compact_cells_scc.sh measure --plan CAMPAIGN/input/plan.json \
  --science CAMPAIGN/science/science.json --output CAMPAIGN/measurement
```

Submit each dependent stage only after its prerequisite has succeeded; SGE
`hold_jid` alone is not a success check. The wrapper loads `miniconda/25.3.1`,
uses `SCC_MINICONDA_BIN` by default, and limits numerical libraries to one thread.
Set `FEREG_BENCH_PYTHON` to an existing absolute NumPy/SciPy interpreter when the
module base lacks those packages; record its package versions in the preparation log. Its
toolchain environment uses the established shared fereg Cargo/rustup roots.
It performs no build or installation. Python needs the existing NumPy/SciPy
environment. For local tests use
`python3 -m unittest discover -s bench/tests -p test_compact_cells.py`.
Successful stage logs end with `COMPACT_CELLS_SUCCESS stage=prepare`, `smoke`,
`science`, or `measure`; require this marker together with successful scheduler
accounting and the corresponding frozen receipt.

Measurement validates the complete 28-physical-core reservation using the
existing SCC configuration, PE, quota, affinity and scheduler validators; slots
alone do not establish isolation. Both worker settings use `taskset` on the
same first four physical cores. One fixed warmup per request runs in that
allocation, followed by six complete randomized rotating blocks, seed
2026101603. The first four blocks balance all arm positions exactly; over six,
position counts differ by at most one. Each call is bounded at 120 seconds;
warmups and measurement each have separate 1,800-second budgets. Exact measured
repeats compare against the same-allocation warmup; cross-host prerequisites
use the existing numerical/discrete checks. No oracle solves occur in this job.

The first error, scientific failure, timeout, identity change or budget
exhaustion stops the run without replacement or resampling. Raw outcomes and
outputs remain available, including partial blocks. Complete paired-block
summaries report median ratios, tail ratios, and excess over `max(5%, 1 ms)`;
they never imply an inferential pass. Partial blocks do not produce ratios.
Timestamped process samples, bounded scheduler checks between blocks and every
60 seconds, and start/end configuration evidence are retained. Ordinary OS
activity is recorded, with no automatic noise exclusions. Review raw competing
compute observations and require successful external `qacct` accounting before
treating even these descriptive results as isolated SCC evidence. Linux
discovery establishes no Mac performance claim.
Malformed observer output or a missing boundary fails closed. The complete
scheduler record is replayed under the existing 75-second coverage bound;
process records must bracket measurement with no gap above 15 seconds (the
five-second cadence plus bounded command time and scheduling margin). These
checks concern missing observation coverage, not the amount of OS activity.
