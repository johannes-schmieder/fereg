# Benchmark suite

The [M26–M35 contract](../docs/cleanup/REPLACEMENTS.md) uses the same runner
with [program-v1.json](program-v1.json). Select it with `--manifest` and a
declared `--tier` (`development-mac`, `development-scc`, `holdout-mac`,
`holdout-scc`, `baseline-pilot` or `capture-pilot`). `--source-root` binds a
frozen source/frontend/dist snapshot. `--paired-source-root` adds the qualified
parent (or fixed baseline), using the same dataset and 32 interleaved fresh-process
pairs. Pair order alternates; `--reverse-order` flips every pair in the second
block. Holdout results cannot be used for tuning while retaining
their holdout label. Pilot successes do not pass the integrated M35 matrix.

New candidates may use the prospectively calibrated `bounded-median-orders-v2`
protocol through the existing runner's `--measurement-plan`. Historical v1
campaigns retain their original rule and remain independently readable.

The frozen JSON declaration contains the complete `[cell, threads, arm]` scope,
manifest hash, candidate/parent/fixed-baseline source and native/frontend
identities, calibration evidence hash, an eight-pair screening budget, and a
32- or 64-additional-pair confirmation budget assigned to every workload before
screening. Each block has a predetermined wall budget. Failure or exhaustion
leaves incomplete evidence and blocks acceptance. No automatic retries occur.
Reference and timed commands use the lesser of their command timeout and the
remaining block budget. Timeout cleanup and capture checkpointing may extend
wall time; an exceeded block budget cannot pass.

Run both screening orders against both references with
`--measurement-stage screening --measurement-comparator parent` or
`fixed_baseline`. A workload unresolved in any of those four blocks receives
its prescribed confirmation against both references and both orders; passing
workloads stop. Confirmation uses `--measurement-stage confirmation` and
`--screening-blocks` with the four screening results in parent-forward,
parent-reverse, fixed-forward, fixed-reverse order. The runner derives the
unresolved scope; it cannot add, omit or select measurements within a workload.
The final checker receives screening then confirmation forward/reverse paths
for each comparator. All eight screening observations remain in the final
40- or 72-pair interval. Exactly two acceptance looks are permitted.

Optional declared `[cell, lower_workers, higher_workers, arm]` scaling edges
apply the same allowance to the candidate's captured worker counts. An
unresolved edge confirms both endpoints. Confirmation of either endpoint also
closes connected declared edges, so final comparisons use both complete stage
histories. These groups join the exact unresolved union across both comparators.

Each look spends one-sided alpha 0.0125. Exact binomial median order-statistic
endpoints use marginal tail probability 0.00625: positions `[1,8]`, `[12,29]`
and `[25,48]` at n=8,40,72. Expand each median endpoint by one microsecond.
For parent interval `[Lp,Up]` and candidate `[Lc,Uc]`, acceptance requires
`Uc-Lp-max(0.001,0.05*Lp) <= 0` in both orders and both comparisons for every
workload. This bounds uncertainty in the parent-dependent allowance as well as
in the difference of command medians. The all-workload gate is an
intersection-union test; the two looks bound false global acceptance by 0.025
under the independent stationary fresh-process measurement model. No family
average can override a slow workload.

Screening failures are measurement uncertainty. A confirmed regression requires
positive lower excess bounds in both orders at the final look, using marginal
tails `0.025/(8*declared_workloads)` to conservatively cover every comparator,
order and median endpoint. With worker edges, replace `declared_workloads` by the sum of
declared workloads and edges; their simultaneous regression claims use the same
endpoint rule. Other final failures remain unresolved and block the
relevant gate. Diagnose time trends, order effects, serial dependence and
competing load using retained captures; violated measurement assumptions block
inference rather than authorize another measurement attempt.

Calibrate once before candidate timing with fresh identical qualified artifacts,
the declared order and stopping rule, and symmetric allowance checks. Ordinarily
use the same workload scope. A broad integration may prospectively declare
representative calibration classes with a reviewed mapping to every timing
class and worker boundary, plus retained drift/dependence diagnostics. This
calibrates measurement resolution; candidate comparisons still cover their
entire separately frozen scope. A null drift test does not prove independent
stationary draws. Saved captures may estimate feasibility and assign
confirmation budgets
before this calibration; they cannot replace it. Retain every calibration and
candidate observation, identity and decision. Astra's bounded protocol review
approved this rule subject to successful calibration. Reserve clean timing
periods: pause our builds and agents and coordinate around unrelated workloads.

Johannes's [2026-10-07 M35 decision](history/2026-10-07-m35-self-calibration-waiver.json)
waives the identical-binary calibration prerequisite for the identified integrated
M35 artifacts. Acceptance plans carry `calibration_waiver` with the decision
path/hash and `milestone: M35`; artifact-bound validation replaces that prerequisite.
The completed calibration remains unresolved. Actual M34/M26 comparisons retain
the existing eight screening and 32/64 additional confirmation pairs, allowance,
two looks, accuracy, identity and measurement-assumption checks. No self-calibration
success or revised 128-pair protocol is claimed; actual unresolved gates still block.

The optional `ols-general-slopes-lsmr-v1` reference policy selects explicit LSMR
for strict auto OLS references with at least three FE and absorbed slopes. It
preserves explicit requests, one worker, strict tolerance and all timed options.
The [M35 reference repair](history/2026-10-07-m35-reference-construction.json)
retains the failed original campaign and broader reference trial. An explicit
`--successor-plan` may inherit only its two named, hash-bound M34 screens:
validate the original evidence first, allow only this reference/tooling delta,
and recheck affected captures against qualified, bound replacement references.
Fresh M26 screens and confirmation use the successor declaration. The complete
unresolved union, connected scaling endpoints and original screening provenance
remain required; inherited observations and original JSON are never replaced.

The historical fixed-budget v1 invocation remains:

For each reference artifact, run a forward and reverse block in separate roots:

```sh
python3 bench/build_clock.py --output target/bench-env/command-clock.plugin
python3 bench/run.py --manifest bench/program-v1.json --tier baseline-pilot \
  --source-root target/candidate --paired-source-root target/parent --fereg-only \
  --clock-plugin target/bench-env/command-clock.plugin \
  --environment target/bench-env --root target/pair-forward
python3 bench/run.py --manifest bench/program-v1.json --tier baseline-pilot \
  --source-root target/candidate --paired-source-root target/parent --fereg-only \
  --clock-plugin target/bench-env/command-clock.plugin \
  --environment target/bench-env --root target/pair-reverse --reverse-order
python3 bench/compare.py --parent-blocks target/pair-forward/results.json target/pair-reverse/results.json \
  --baseline-blocks target/fixed-forward/results.json target/fixed-reverse/results.json \
  --fixed-plugin-sha256 FROZEN_PLATFORM_SHA256 --fixed-frontend-sha256 FROZEN_FRONTEND_SHA256 \
  --output target/envelope.json
```

The checker requires both references, matching artifact/tooling/manifest/data
identities, complete accurate pairs and the declared order. When parent and
fixed baseline are the same artifact, the same pair of blocks can serve both.
It evaluates the paired-bootstrap difference of command medians with the frozen
seed, sample budget and quantization allowance; unresolved results cannot pass.
Baseline calibration adds `--calibrate`, requires identical artifacts and both
interval ends within the symmetric allowance in each block.
The monotonic clock brackets the complete command and OLS prediction, with
Stata's timer retained as a secondary witness. It is built in the private
environment, loaded before timing, and hash-checked before and after the run.
Its tick resolution must be at most 500 ns. The frozen 1 μs bound per duration
covers two timestamp endpoints; scheduling and dispatch variability stay in
the paired sample. No overhead is subtracted and no timer fallback is allowed.
Windows stopwatch support remains unqualified until the Windows platform check.
Full program CSVs are bounded to 8 MiB and default to the ignored raw run root.
Committed evidence remains a compact summary of at most 100 KB. A full tier can
also be partitioned deterministically with existing `--cells` selections.

`suite.toml` declares 22 general cells and six bounded M1 cells, immutable seed, comparator policy and tiers.
`generate.py` ports the preserved synthetic generators and writes a single
numeric DTA for every estimator. The first controls are invariant to K. Seeds,
data hashes and generator versions are stored beside the temporary datasets.
Veneto uses an external prepared DTA with `obs_id y x1 x2 x3 x4 g1 g2`; its source data
and preparation stay outside Git (see `real/veneto/prepare.py`). The original
3,159,720-row, four-control model input is hash-pinned; a lossless DTA-format
conversion lets Stata and base R read the same rows and controls.

Prerequisites: Python 3.11+ with NumPy and pandas, R with its base `foreign`
package, the pinned Rust toolchain, and licensed Stata/MP. Use a private run root:

```sh
python3 bench/setup.py --root /tmp/fereg-bench-env
./scripts/build_macos.sh
python3 bench/run.py --tier smoke --engine next \
  --environment /tmp/fereg-bench-env --root /tmp/fereg-bench-smoke
```

Setup verifies the archive hashes in `references.lock.toml`, stages the three
bundled Stata references plus pinned ppmlhdfe, and builds fixest 0.14.2 in a
private R library. It does not change PLUS, PERSONAL or the user's R library.
Acquisition/builds happen before timing. Source-index files stay with their
package; only runtime files share the private Stata reference path.

Each cell/thread/comparator receives one untimed warm-up process followed by
five fresh measurement processes in `suite.toml`; the second program fixes 32.
The reported value is the median of the declared number of
successful estimation times, excluding process startup and dataset loading.
Requested residual/FE outputs are included in estimation work. A separate
fereg fit at tolerance 1e-12 supplies reference coefficients and exact sample
IDs. fixest defaults and matched accuracy are distinct arms (OLS FE tolerance
1e-10; PPML GLM 1e-10 and FE 1e-11). Stata host processors are capped at its
license limit; native/R threads follow the manifest. SCC caps Stata at four.

OLS input/native/total-to-precommit times come from `e(fereg_runtime)`;
legacy PPML exposes total time in `e(feppml_runtime)` but no separate SPI-input
and native-time fields, so those columns stay empty. The wall timer covers the
whole estimation command. Neither unavailable telemetry nor a failed fit is
represented as zero.

Both estimators accept `engine(next|legacy)`; the runner defaults to next.
Campaigns and each measurement record the selected binary SHA-256 and shared
frontend identity. Legacy timing requires its canonical registered hash. The
runner verifies the selected plugin path and returned engine in Stata, and
rejects a binary/frontend change during the campaign. CSV summaries retain
these identities plus median, range and median absolute deviation.

For M1 comparison, run `--fereg-only --engine next` and `--fereg-only --engine legacy`
in separate fresh roots on the same machine. Use both `--tier smoke` and
`--tier m1`. The latter covers OLS/PPML at N=16,383/16,384 with eight regressors
and N=100,000 with sixteen regressors, at 1/2/4/8/16 native threads. These six
cells are excluded from the existing standard/full inventories. Compare input
hashes, coefficients and samples between engines before interpreting timings;
report dispersion and investigate differences beyond noise. The focused engine
comparison holds Stata processors fixed at the licensed cap while varying
native threads. Keep builds and
coverage instrumentation out of timing windows. This focused comparison does
not rerun the external-package timing arms; `--fereg-only` labels its summaries
as engine comparisons.

`--solver lsmr|cmg` is for the separately declared supported-case comparisons;
use it together with `--solver-comparison` for the manifest's strong/weak AKM
and three-FE PPML subset at 100,000 rows and 1/8 threads. Run each of auto,
LSMR and CMG in a separate root. These summaries carry `comparison=solver`
and separate filenames; they do not enter the default-policy aggregate.

`standard` uses all 19 non-SCC cells at 1/8 threads; `full` adds the three SCC
cells and the declared 1/2/4/8/14/28 thread ladder. Use `--plan-only` to inspect
jobs without executing them. `--cells` restricts diagnostic runs and marks their
summaries partial. Standard/full command timeouts are 30 minutes/two hours.
A failed or timed-out command stops that arm's remaining repetitions; there is
no automatic retry. Other independent arms continue. A new invocation requires
a new root, and summaries never overwrite earlier evidence.

`compare.py` writes a CSV of at most 100 KB to `history/` (or `--summary`), and
prints pairwise ratios, geometric means, worst cells and timeouts. Missing
commands/repetitions, reference failures, sample disagreements and nonfinite
coefficients remain visible. Only complete accurate pairs enter aggregates;
the passing/expected count is always printed. Partial or dirty-source smoke
results are diagnostic, not committed-source baseline evidence. The current M1 plan establishes
Mac comparisons and defers large SCC campaigns. Raw logs and datasets stay in the run root.

Harness checks (no licensed command needed):

```sh
python3 -m unittest discover -s bench/tests -v
```

## Historical rebuild benchmarks

The abandoned rewrite's FE-only admission driver is retired. Its implementation
and evidence remain at `legacy-comparator-v1`. The active M0–M1 plan uses actual
plugin replay and the existing command-level benchmark runner.

Sample identities use immutable packed integers for ordinary captures, retaining
CSV field limits and exact ordered comparisons; unbounded integers retain a cold
tuple representation. The 64 MiB reference-cache budget covers retained entries
and actual directory allocations after each completed read; transient parsing
and eviction rebuilds are separate. Every actual capture remains independently
read and every reference reuse retains the artifact/stat checks.
