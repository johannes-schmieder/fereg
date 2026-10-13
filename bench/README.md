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

Every run must reproduce exact science and solver signatures; the decision is
the paired-median bootstrap upper bound against max(5%, 1 ms) per workload.
Run only on a quiet host (no builds, agents or Stata). The former three-FE LSMR
timing inputs are not reproducible from a tracked recipe and are not frozen.

The [C2 qualification follow-up](results/c2-20261012-qualification.json) retains
the fixed fifteen-pair Mac panel and separate SCC diagnostics, including all
failed/inconclusive bounds. Johannes accepted the frozen candidate with the
remaining timing uncertainty; see
[the adoption decision and limits](../docs/VALIDATION.md#c2-adoption-with-unresolved-timing-2026-10-12).

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
input tolerance. One single-worker LSMR reference at `1e-12` precedes each input.
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
them as an elapsed-time decomposition. Run on a quiet host after builds, tests
and other agent workloads finish. SCC observations do not qualify Mac speed.

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
aggregate summaries and hashes. All outputs belong under `target/tmp/cmgschur/`;
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
observations do not replace the full scientific gate, native replay or quiet
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
