# Local benchmark

Four fixed-seed OLS/PPML models exercise one and two fixed effects. The `smoke`
tier uses 10,000 rows and one worker; `standard` uses 100,000 rows and 1/8 workers.
Each has a strict-tolerance reference, one warm-up and five fresh Stata processes.
Preparation is outside timing. This is a descriptive development benchmark,
not an acceptance campaign or an independent scientific oracle.

With Python 3.11+, NumPy and pandas installed, and the current plugin built:

```sh
python3 bench/run.py --tier smoke --stata "$STATA_BIN" --output target/tmp/benchmark/smoke
python3 bench/compare.py target/tmp/benchmark/smoke/run.json
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

## SCC package comparison

`campaign.json` freezes the descriptive OLS/PPML comparison against pinned fixest
0.14.2 (default and strict profiles), PyFixest, FixedEffectModels.jl, reghdfe,
ppmlhdfe and Stata 19 areg. At one million rows and four cores it covers the
AKM baseline with every covariance type with and without saved FE, 4 and 64
controls instead of 16, and one-FE and dense/sparse two-/three-FE designs. Core
series at 1/2/4/8/16 workers use the AKM baseline, and size series from
100,000 to ten million rows cover AKM OLS, AKM PPML and sparse three-FE OLS.
Cells up to one million rows have five repetitions and larger cells three.

PyFixest runs a pinned main revision whose default is the `within` LSMR
demeaner, with default estimation tolerances and saved FE recovered at 1e-11
like the fixest arms. FixedEffectModels.jl runs in linear cells only, at its
default tolerance (1e-6) and at 1e-10 (`julia-strict`); `julia --threads` sets
its workers and a warm-up fit of the same specification keeps compilation out
of timing. areg runs in every OLS configuration with at most four workers and
no saved FE (it cannot save individual effects). With several effects it uses
`dfabsorb noabsorbtest tolerance(1e-8) iterate(16000)`, the options of the
August 2026 comparison. Its absorbed count is compared after adding back the
intercept it reports separately; with clustered covariance it does not discount
effects nested in the clusters, so those cells stay descriptive.

`campaign.py prepare RUN_ROOT` packs one model and design per node, so each
size, core and option series compares absolute times on one host, and orders
sizes ascending: a command that reaches the time limit is not attempted at
larger sizes of its series (`skipped-after-timeout`). Every node first times
the same native reference cell, recording its speed.

Comparisons separate counting conventions from numerical differences. Packages
count absorbed degrees of freedom differently (connected components, levels, or
nesting in clusters); this changes reported counts and scales the covariance by
one small-sample factor. A comparison agrees by convention when all other
checks pass and the covariance matches after one scalar within
`covariance_scale_bound` of one; such pairs supply time ratios and are marked
in reports. Strict eligibility is unchanged.

Memory is exact: after an arm loads its input it writes READY and waits; the
harness resets the kernel's peak resident set of the process tree
(`/proc/PID/clear_refs`), writes GO, and after DONE reads VmHWM before writing
ACK. The peak therefore covers the timed command with the loaded data, and the
summary also gives the increase over the loaded state. The 20 ms RSS samples
remain a cross-check, and the smoke gate requires exact peaks.

`scc/setup.sh` builds a frozen source archive and copies the pinned comparator
environment from `REFERENCE_ROOT`; `CAMPAIGN_PLUGIN=dist` measures the published
package binary after checking its checksum instead of rebuilding it.
`scc/pyfixest_environment.sh` installs the PyFixest arms named in
`campaign.json` from the locked requirements in `pyfixest/` (the main revision
builds its Rust extension with the pinned toolchain), and
`scc/julia_environment.sh` instantiates and precompiles `julia/Manifest.toml`
in its own depot. The runtime inventory hashes every environment, including the
Julia project, package sources and artifacts. Supply
`RUN_ROOT/input/frozen-inputs.json` with `remote_root` and `datasets` from the
retained campaign input record. Production copies those datasets, verifies file
and decoded column hashes, and preserves their metadata; other sizes and
control counts, small smoke inputs and warm-up inputs are generated with the
same generator and seed.

Submit setup, smoke and the production bundles with SGE dependencies and
`-P welfgr`. `scc/job.sh` takes `RUN_ROOT`, `MODE` (`smoke` or `run`) and
`UPSTREAM_JOB`, requiring successful upstream accounting and application checks.
The jobs reserve one complete 28-core node; no MPI ranks are launched. The
existing qualification host observer verifies the scheduler reservation,
all-queue census, affinity and absence of known competing compute throughout
smoke and production. Every package receives the same socket-ordered CPU
prefix and NUMA memory binding. There are no automatic timing retries or
interference exclusions.

Timing includes estimation, inference and requested FE recovery, including
fixest's lazy extraction and row materialization. Import, generation and disk
export are excluded. Scientific gates and per-fit limits remain in `campaign.json`;
failed or unavailable observations stay visible. This descriptive comparison
does not qualify a runtime change or establish a universal speed advantage.

`campaign_collect.py CONTEXT --output DIRECTORY` collects terminal accounting,
application results and frozen identities. Set `host_observation_required: true`
in the context for observed runs. `campaign_report.py DIRECTORY --output REPORT`
produces the full PDF and CSVs, enforcing complete coverage, scientific eligibility,
fixed-worker exact repeatability and cross-worker agreement, and
`campaign_export.py DIRECTORY --report REPORT --output RESULTS --prefix PREFIX`
writes the compact summary, observation and input records kept in `results/`.
See the [comparison guide](../docs/fixest_benchmarks.md) for interpretation.

## Private Veneto panels

`panel_campaign.py` uses the same pinned estimator entrypoints, timers, numerical
tolerances and Linux placement as the synthetic campaign, with the actual fevc
retained panels. The long panel is 1996–2001 (5,163,428 observations); it jointly
fits worker/firm effects, `((age-40)/40)^2`, `((age-40)/40)^3` and year indicators
with 2001 omitted. The short panel is the pooled 1999/2001 sample (1,319,994
observations), jointly fitting worker/firm effects and a 2001 indicator. Both
use unadjusted log daily wages and regular OLS covariance. These are the existing
fevc samples; this runner does not change their cleaning or connectivity rules.

Preparation takes private retained CSVs containing `year worker firm
observation_key y` and, for the long panel, `age`. It rejects cardinalities that
differ from fevc, missing/duplicate person-years, disconnected graphs and singleton
FE levels. It encodes common Stata inputs, verifies decoded values and hashes,
and constructs small actual-data explicit-design witnesses:

```sh
python3 bench/panel_campaign.py prepare PRIVATE_RUN \
  --long PRIVATE_LONG_RETAINED.csv --short PRIVATE_SHORT_RETAINED.csv
```

Do not put these inputs or row-level outputs in Git or public storage. Preserve
owner-only access and obtain approval for any required transfer. Only aggregate
timing/science summaries belong in committed results.

The fixed schedule has three repetitions at 1/2/4/8/16 cores for fereg, the
default/strict fixest profiles, the PyFixest arms pinned in `campaign.json` and
both FixedEffectModels.jl profiles; reghdfe participates at 1/2/4, within the
SCC Stata license. This gives 198 planned fit positions for both panels.
FixedEffectModels.jl counts every FE level, so its absorbed count is checked
against the sum of levels rather than the graph rank.
`panel_campaign.py reuse NEW_RUN --previous PRIOR_RUN` copies a prior run's
audited panel inputs, smoke witnesses and policy byte for byte and rebuilds only
the per-cell arm lists; `--without-saved-fe` schedules the same cells without
saved FE, so commands time estimation and inference and comparisons cover
slopes, covariance, sample, rank and RSS. A reused run has a 20,000-second fit
budget. The collector retains the reuse job and the prior run's preparation
records. `--panels long` schedules one panel;
`scc/panel_pair.sh` then runs the production task of several such runs back to
back in one whole-node job (for example the long panel with and without saved
FE), so their absolute times share a host. Each panel runs sequentially in
one independently verified whole-host allocation, using common socket-ordered
CPU prefixes and NUMA memory binding. A separate four-slot real-data smoke must
pass before either production task runs. The two panels may use different hosts.
There are no automatic timing or scientific retries; unavailable comparator
positions and partial failures remain visible.

Commands include estimation, inference and saving both worker and firm FEs;
import, disk export and validation are excluded. The runner compares slopes,
covariance, sample, rank, objective and fitted values, and compares both FE
channels after observation-weighted centering on the connected graph. Native
repetitions also require exact raw scientific output and FE channel hashes;
cross-worker results require scientific agreement. Saved effects and full row
captures remain private on SCC.

`panel_collect.py CONTEXT --output NEW_DIRECTORY` collects aggregate results
after all four scheduler units are terminal: setup, smoke and production tasks
1–2. It requires successful accounting, application sentinels, replayed host
isolation observations, the retained-input audit, every planned fit position,
exact native repeat checks and consistency comparisons with the one-core
baseline. It also requires exact repeated reported native RSS.
It writes compact summary and
observation CSVs plus science JSON; failed or ineligible comparator positions
remain in the tables. It does not download row captures or saved FE vectors.

`panel_report.py COLLECTION --output REPORT_DIRECTORY` revalidates native
eligibility and renders the maintained PDF from aggregate evidence, exporting
compact CSV/JSON summaries with the `scc-YYYYMMDD-veneto` prefix, or the
context's `export_prefix` when one date has several runs. Render the PDF
to images and inspect every page before publication. Retain
the report and full captures privately on SCC, and commit only the compact
aggregates. The [October 24 results](../docs/veneto_benchmarks.md) retain all
reference accuracy failures as descriptive timings without eligible speed ratios.

For accuracy magnitudes from an existing campaign, run
`panel_accuracy.py PRIVATE_RUN --panel long --output PRIVATE_AGGREGATE.json`
(and likewise `--panel short`) in a compute-node job. It reads the original
four-core first repetitions without fitting again or changing any gate. It
reports coefficient/covariance, RSS, fitted-value and centered worker/firm FE
differences from fereg, plus independent residual model scores. To cover the
full schedule, add `--cores 1 2 4 8 16 --repetitions 3`. Every planned position,
including skipped positions, stays in the output. Later native repetitions reuse
the first native diagnostic only after exact capture and reported-RSS agreement
is verified; each completed reference repetition is inspected separately. Only aggregate
diagnostics and input hashes may leave private storage; full row and FE vectors
remain on SCC. The native comparison is not a full-panel exact oracle.

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
| `min_distinct_hosts_per_cell` | Optional Linux-only minimum, an integer from 1 through `replicas` (at most 12). Every cell/reference must have the declared number of accounted physical hosts. |
| `seed` | Unsigned 64-bit ordering seed. |
| `timeout_s`, `budget_s` | Fixed per-call and per-job limits, at most 120 and 14,400 seconds. |
| `coverage` | `routing` requires all 42 cells; `smoke` only checks tooling and cannot qualify a candidate. |

Artifact `build` records compiler and build settings in a short string. `request`
is `auto`, `preserve`, `incremental-many`, `many-geometry-v1`, `incremental-two`,
`two-remaining-v1` or `two-gates-v1`. `families` lists
accepted numerical changes relative to the original (`[]`, `["many"]`, etc.).
Original and incumbent use their actual uninjected public invocation. A public
Auto candidate requires the corresponding hidden artifact, with exact agreement
of scientific output and numerical solver signatures. Unchanged explicit OLS
requests use `scope: "exact"` and `request: "preserve"`. This routing runner is
OLS-only; PPML remains covered by the mapped feature/replay suite.
Resource and gate evidence follows the eligible family: two-FE hidden policies
retain accepted many-FE routing outside TWO. A public candidate uses the selected
hidden artifact's policy for that family; original and incumbent references do
not inherit that override. Missing expected gate fields fail reporting.

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
runnable at 20% CPU or more. Lesser activity stays recorded noise. A flag affects
only its overlapping cell attempt.
Fresh plans permit at most one replacement per affected cell and four replaced
cells overall, after two clear sampled intervals within 120 seconds. Their call
cap includes all distinct execution arms, warmups and discarded attempts; the
whole-run budget remains at most four hours. These plans cannot reset their
budgets through resume, continuation or recovery entrypoints.
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

### Results and cleanup

Use one report and retain a concise result. Do not re-run an unchanged candidate
to obtain a favorable timing. See [development](../docs/development_workflow.md)
for stopping rules, scientific requirements and practical Mac measurements.

[`ppml_cases.py`](ppml_cases.py) writes campaign-DGP PPML replay cases for
`time_exact.py` (arithmetic-preserving PPML candidates only):

```
REPLAY_MAX_CELLS=120000000U bash bench/build_timing_harness.sh target/tmp/TASK/harness
python3 bench/ppml_cases.py --output target/tmp/TASK/ppml --threads 4 28
python3 bench/time_exact.py --plugin base=... --plugin candidate=... \
    --cases target/tmp/TASK/ppml/cases/*-t4.case --output target/tmp/TASK/t4 --repetitions 5 \
    --harness target/tmp/TASK/harness
```

`native.py`, `time_exact.py`, the replay tools and the campaign runner remain
maintained diagnostic entrypoints; use their `--help` for bounded runs. Schema
adapters imported by qualification remain for compatibility.

Current acceptance records are indexed in [VALIDATION](../docs/VALIDATION.md).
Remove task outputs after recording concise results. Keep numerical references and
maintained regression fixtures; do not treat an old experiment as an active task.
