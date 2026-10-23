# Validation status

Johannes accepted the integrated runtime on 2026-10-07. That is architectural
acceptance, not a performance pass: the final timing campaign closed incomplete
and historical timing was mixed. The Mac is the qualification host. Linux and
Windows evidence at that acceptance came from bounded build, load and mapped-fixture checks,
which do not establish full platform scientific or performance qualification.
Johannes's 2026-10-14 decision additionally authorizes isolated SCC algorithm
discovery and Linux performance measurements; those do not establish Mac speed.
GitHub testing is paused. Native distribution permissions were confirmed on 2026-10-13;
the [licensing decision](licensing.md) records the distribution terms.

The many-FE candidate now passes the 2026-10-18 compact Linux qualification and
practical Mac recovery, including independent review of actual retained science,
timing and operational evidence. Each platform passes all 126 protected comparisons.
The development-branch activation and its integration checks are recorded below.
The separate two-FE qualification completed, but Linux found regressions on the
four-worker high-degree weak graph against both original and accepted Auto.
Mac passed. Two-FE remains disabled, many-FE remains enabled, and release packages
are unchanged. Johannes's 2026-10-21 direction authorizes the setup and upgrade
strategy work below; the historical failed comparisons remain failed.

This page keeps accepted evidence, open failures and standing caveats. The full
narrative ledger, with every run identifier, archive and receipt digest,
campaign parameter and failed attempt, is in Git at commit `140dda27c42a`:
`git show 140dda27c42a:docs/VALIDATION.md`. Ignored raw captures it mentions
were deleted; hashes and summaries are the remaining record.

## Selected two-FE gates: fixed qualification (2026-10-22)

The [Mac result](../bench/results/two-fe-policy-gates-qualification-mac-20261022.json)
passes all 84 protected performance bounds across 42 cells and 6,174 calls.
Its two linked call tables retain every complete call object losslessly.
There were no scientific/native failures, discarded observations or cell
retries. Independent review verified the frozen bindings, all 126 retained
reference sets, cross-arm and cross-worker science, policy/resource traces,
and independently reconstructed every bound from `NATIVE_SECONDS`.
The closest upper excess was -0.746312 ms, repeated-cells W4 against original
Auto. These are per-comparison bounds from 24 mirrored blocks on one active
Mac, not simultaneous coverage or a general speedup claim.

The initial Mac observer attempt failed because sandbox access to `ps` was
denied, before any native call. That failure remains retained. The unchanged
plan ran with host-observer access and a reduced external deadline of 14,399
seconds, deducting the failed attempt's 0.719856 seconds from the original
budget. It completed in 2,447.765 seconds. All 533 observer samples recorded
no competing compute; ordinary desktop activity remains included. Sampled
observation is not continuous proof of absence.

The [Linux result](../bench/results/two-fe-policy-gates-qualification-linux-20261022.json)
also passes all 84 protected bounds across 42 cells, 12 distinct physical hosts
per cell and 7,560 calls. Its linked call tables retain every complete call
object losslessly. All 36 successful shard accountings are 0/0. Independent
verification reconstructed all 1,512 retained full-column fingerprints, checked
the independent references, cross-arm/worker/host agreement, fixed-worker
repeats, warmup timers, observers and resource/policy traces, and reproduced all
84 bounds. The closest upper excess was -0.721883 ms, no-FE W4 against accepted
Auto. These are per-comparison one-sided 0.025 bounds using 12 physical host
units; they do not claim simultaneous coverage or independence across platforms.

Two analysis-only verification attempts exhausted their fixed 600-second
budgets. Both failures, helper sources and accountings remain in the result.
Moving temporary reconstruction files to job-local scratch did not resolve the
timeout. The reviewed final repair materializes each bound NPZ archive once,
retains the comparison baselines and current cell, and preserves raw hashes,
drift checks, comparison bytecode and all scientific checks. It completed in
447.177 seconds with 996,667,200 bytes of cached arrays below its 1.2 GB limit;
job `8253766` has 0/0 accounting. All three analysis attempts made zero native
calls. The root report and native observations were never rerun or resampled.
Exact public-versus-hidden equivalence remains required before activation is
accepted on the development branch.

The [qualification freeze](../bench/results/two-fe-policy-gates-qualification-freeze-20261022.json)
keeps native candidate `e9be9e64` / CMG `fd77d98d`, the selected structural gates
and both protected public baselines unchanged. All 21 families run at W1/W4.
Seventeen generated bodies use the separate seed 2026105201; four imported
feature fixtures retain their independent replay references. Sixteen bounded
categorical references and the inactive-column analytic witness cover the
generated inputs. Independent review verified both plans, all 82 bound files
per platform, actual plugins and harnesses, and all 20 execution-source hashes.
Five helper files initially missing from the standalone runner archive were
added and reverified before execution. No native source changed.

Ordering seed is 2026102206. Linux fixes 12 distinct physical hosts per cell,
three shards and two mirrored blocks: 36 tasks and 7,560 calls, with 3,600-second
application and 4,200-second scheduler limits per task. Mac fixes 24 blocks and
6,174 calls within 14,400 seconds; its existing external-interference policy
permits at most four whole-cell replacements, up to 6,762 calls in total. Each
native call has a 120-second limit. Ordinary desktop noise stays in the data;
scientific failures, native failures, slow results and inconclusive bounds do
not authorize resampling. A separate 90-call SCC smoke checks the deployed
runner across TWO, MANY and unaffected feature paths before qualification.
Queued host assignments do not establish isolation or satisfy the host count.

The [operational evidence](../bench/results/two-fe-policy-gates-qualification-operations-20261022.json)
retains the successful 90-call SCC smoke, all its call objects, scheduler
accounting, observation summaries and preparation failures. Independent review
verified the retained calls, reference heads and operational evidence; local
smoke arrays were not collected, so no independent array recheck is claimed.
The smoke supplies no performance qualification. Subsequent scheduler diagnosis
found that the frozen `scc-wp3` assignment is barred by a user restriction;
its exact enabled rule is retained. Other assigned hosts were occupied.
Idle public hosts use a different PE with MPI startup/cleanup hooks. Johannes
authorized a bounded validation and prospective allocation amendment on
2026-10-22. The [amendment evidence](../bench/results/two-fe-policy-gates-allocation-20261022.json)
retains one zero-estimation probe, job `8250416.1`, on `scc-uf1`: scheduler
failed/exit 0/0, 28 seconds and 345.344M maximum virtual memory. Two reservations
and two all-queue censuses verified one 28-core physical host, full affinity,
the consumable quota and exclusive job/task allocation. Five full process
snapshots showed no MPI launcher or rank daemon. An administrative package
update consumed about 20 CPU seconds during the probe; the host was not idle,
and this probe supplies no timing evidence. Audited hook hashes, startup files
and subsequent removal of the exact temporary files were checked.

Only the eight remaining replicas change to `mpi_28_tasks_per_node_u`.
Their 24 task indices and call budgets are retained;
four completed replicas retain all 12 original shard results and 2,520 calls.
The amended plan differs only in the reviewed observer/report source hashes.
The reporter binds both complete source versions, selects the expected plan
by a frozen replica partition, and rechecks reservation/census/accounting
evidence before applying the unchanged scientific checks and host-level
performance bounds. Both cohorts still require 12 distinct physical hosts
per cell. The guarded launcher checks audited hook and wrapper hashes at
entry and exit. All 88 focused tooling tests passed; the production-identity
check found all 198 runtime modules and 58 artifact inputs unchanged.

The record also retains the probe's pre-dispatch host-placement changes,
normal accounting lag, a short-host SSH verification failure, and two
never-started cleanup requests that were cancelled. The one-slot cleanup
request was barred by the MPI queue policy; attempts to change that serial
request's PE were rejected. Using the canonical hostname verified cleanup
with existing hostname trust and without relaxing SSH verification. No
qualification observation was discarded or repeated for these operational
repairs, and the completed Mac qualification remains unchanged.

The [launcher repair](../bench/results/two-fe-policy-gates-launch-repair-20261022.json)
retains three subsequent pre-native failures: the amended archive's runner
rejected the original archive's output directory. The failures occurred before
output creation, source verification or native execution. An exact `target/tmp`
link now gives both archives the same canonical task subtree; the containment
guard and all 20 execution-source hashes remain unchanged. A read-only preflight
in the actual SCC Python environment checks those hashes, the canonical paths
and absence of all 24 pending task outputs before release. The 21 never-started
tasks keep their original jobs; three replacement jobs keep the original hosts
and task indices. Their recorded 4, 6 and 4 elapsed seconds are deducted from
both application and scheduler budgets. No timing observation is resampled.

Completed held-out captures and inputs were hash-checked against committed
records before deletion, along with the completed original-build cache. Current
qualification inputs and actual artifacts remain available. Public two-FE Auto
is still disabled. Both platform qualifications now pass; the next step is the
authorized branch-only invocation wiring and exact public-versus-hidden
integration checks. Release packages remain unchanged.

## Selected two-FE gates: held-out development (2026-10-22)

The [frozen development comparison](../bench/results/two-fe-policy-gates-development-freeze-20261022.json)
completed all 864 calls, 432 per platform, with the unchanged native candidate
`e9be9e64` and CMG `fd77d98d`. The
[complete compact results](../bench/results/two-fe-policy-gates-development-20261022.json)
retain every warmup and recorded call, both platform reports, actual binary and
source bindings, scientific fingerprints, gate/resource evidence and failures.
All scientific, own-repeat and cross-worker checks passed. Independent review
rehashed all retained reference arrays and heads, reparsed warmup traces and
timers, and reproduced the paired scores. All 144 cross-platform reference
fingerprints and 48 candidate policy signatures agree. Each platform installed
18 upgrades at iteration 16, deferred nine Final actions and had 21 cells without
a policy episode. These counts include all three worker counts.

All 96 Mac point comparisons stayed within max(5%, 1 ms). Linux had seven
positive point excesses across four cells: dense Low W8, inactive-columns Fast
W8, wide-rhs Fast W4 and wide-rhs Low W8. The largest excess against accepted
Auto was 37.213 ms for wide-rhs Low W8 (ratio 1.09099). Weak-connectivity cases
showed gains, but the aggregate medians are not workload-weighted speedups.
One isolated Linux host and one Mac mirrored block cannot establish the required
uncertainty bounds: all 192 bounds remain inconclusive by design. No timing was
excluded or rerun, and no model was selected or refitted on these observations.
The separate 42-cell qualification and exact public-hidden equivalence remain
required; public two-FE Auto remains disabled.

Linux job `8245855` completed with scheduler failed/exit 0/0, 697 seconds and
1.161G maximum virtual memory. Its first report failed because the observer
canonicalized scalar SGE task `undefined` to `0`, while the reporter compared
the spelling literally. A tested and independently reviewed accounting-only
adapter repaired the report using the same frozen source, raw accounting and
calls. The failed report is retained. FQDN shortening already worked.
The observers recorded 90 Mac and 139 Linux process samples, with maximum gaps
5.008 and 6.694 seconds, plus two Linux reservations and 13 queue censuses.
No competing compute was observed; this is sampled coverage, not continuous
proof. Ordinary desktop and OS activity remains in the measurements.

Prospective qualification tooling now resolves the actual eligible family:
hidden two-FE gates fall back to accepted many-FE routing outside TWO, and a
public candidate uses its selected hidden policy for the relevant family.
Protected references retain their own policies. Expected gate evidence fails
closed in both run and report. Scalar SGE task normalization and the accurate
`NATIVE_SECONDS` metadata label are also maintained. The scored `seconds` value
already used that complete-call timer; auxiliary `native_s` was never the score.
Ninety focused tests and bounded Astra review passed. Production identity is
unchanged, so the completed source/native integration evidence is reused.

## Two-FE remaining-cost policy foundation (2026-10-21)

Johannes accepted the ninther setup for policy development. Independent GPT Pro
and Claude Fable proposals and cross-critique selected the formulation in
[the development specification](two_fe_auto_policy.md). The hidden successor
now carries actual Fast/Low/final demand, bounded completed-action history,
admitted workers and cancellable progress observations into checked fixed-point
cost arithmetic. Its predictive coefficients remain absent, so prediction
defers; public two-FE Auto remains disabled. Never/force16/force32 controls and
disjoint economic labels support the frozen prospective calibration only.

The [compact development evidence](../bench/results/two-fe-policy-development-20261021.json)
retains commands, exits, source/log hashes and all failed attempts. The complete
source-only suite passes, including 315 general Python tests, seven portable
harness tests, Rust workspace/FFI checks, isolated allocator audits, Clippy,
fuzz compilation, complexity and mock bridge guards. All 366 benchmark-tool
tests pass. The full standard entrypoint still fails its known installed-plugin
binding; no installed or release artifact was changed. Source validation is not
package admission or native qualification.

Four real callsite mutations are killed: omitted correction reset, omitted
observer cancellation poll, omitted parent-memory charge and a refreshed
iteration budget at handoff. Sources were restored byte-for-byte and all six
focused witnesses pass. The observer witness interrupts after 2,048 inspected
entries following a sticky request at entry 1,025; the restart witness retains
17 PCG plus five MAP iterations under the existing total cap. Independent
analytic projections and exact memory boundaries remain in the tests.

A prior broad run failed the old h16 test without printing its internal
assertion. A separate deterministic witness demonstrates that nested actions
on the cached worker pool can overwrite a worker-local test observation.
Four affected witnesses now run in bounded child processes with complete
assertions, inherited failure output and kill/reap on timeout. The later focused
and full passes do not erase the failure or prove its exact historical cause.

The reviewed complexity increase is 1,398 core production lines and 15 plugin
lines; no new function/file size violation is introduced. The sidecar and labels
are scoped to sanctioned successor injections. Actual native builds,
old/outside-scope replay and Stata transactions are recorded at the stable
artifact boundary below. The completed 1,728-call calibration is assessed below.
Held-out validation and separate fixed 42-cell qualification remain required
before public activation or a performance claim.

### Native integration and frozen calibration (2026-10-22)

The [frozen protocol and native evidence](../bench/results/two-fe-policy-calibration-20261022.json)
bind source `cdc478dd0cac7a6d4d5c0861fb4e380820987022` and unchanged
CMG `fd77d98d`, Rust 1.85.1, locked fat-LTO builds, bridge O3 and the fixed
2026-10-21 build date. Fresh Mac/Linux artifacts pass binary audits and actual
ABI/load/self-tests with injected-panic containment. Linux build job 8243812
used four slots on `scc-gd4`, completed with scheduler failed/exit 0/0, and its
actual plugin was collected and independently rehashed locally.

Mac preserves exact scientific results on all 73 accepted-runtime replays;
all 146 W4/W8 checks and independent numerical checks pass. Six mapped Stata
fixtures pass, including nonconvergence and independent residual witnesses.
Actual W4 optional-builder Break tests pass twice with fresh follow-up fits;
explicit sampled-CMG native and publication interruption checks also pass.
The initial Stata command used an artifact alias; the runner rejected its
filename before launch. Using the identical bytes at their required platform
filename passed. That failed attempt remains recorded.

The prospective calibration has 16 independently generated input bodies and
32 Fast/Low cases, with fixed independent references and separate calibration,
development and qualification seed streams. Both platform plans bind the same
full inputs and 19 execution modules. Independent freeze review rehashed all
108 Mac plan files and 242 preparation sources and found no blocker. Each
platform has exactly 864 calls: 96 case/worker cells, three controls, one retained
warmup plus two mirrored recorded executions. The controls share one actual
binary. Limits are 120 seconds per call and 14,400 seconds per platform, with
no retries or continuation. The Linux plan requires a verified whole physical
host; ordinary Mac desktop activity remains recorded noise.

The freeze contains no calibration observations and establishes no fitted
coefficient, support coverage or forecast accuracy. Predictive calibration is
still absent and public two-FE Auto is disabled. The installed-package binding
remains failed; these isolated development artifacts do not update distribution
packages. Previous timing uncertainties and failures remain unchanged.

### Fixed calibration outcome (2026-10-22)

The [complete outcome](../bench/results/two-fe-policy-calibration-outcome-20261022.json)
retains all 1,728 parsed native calls in four bounded files, with the frozen
source/artifact bindings, host observations, accounting, commands and failures.
Each platform completed 864 calls without a native failure, exclusion or retry.
Linux job 8244019 reserved all 28 physical cores on `scc-we4` and completed with
scheduler failed/exit 0/0. Sampled observers found no competing compute; this
is sampled coverage rather than continuous proof. Ordinary Mac activity remains
in the observations. All maintained numerical, repeatability, cross-worker,
resource and route checks pass. An additional comparison of 288 retained
Mac/Linux warmup outputs passes the maintained scientific tolerances and exact
discrete gates; their routes, hierarchy and scientific fingerprints also agree.
Recorded raw logs and numerical arrays were removed after online checks;
independent raw reparse is limited to retained warmups.

The [single prescribed pooled fit](../bench/results/two-fe-policy-fit-20261022.json)
uses 180 matched intervention observations from 108 input/mode/worker/platform
cells and 144 complete suffixes. All 204 unavailable intervention observations
are explicitly classified as no intervention. Exact rational fitting,
one-time Q32.32 quantization and every encoded inequality pass. Seven of the
96 current support keys have the required seed/platform/worker coverage;
150 matched observations are supported. No completed compatible-history,
mixed-width-history or correction-epoch support was obtained.

The fitted decision selects **zero builds in those 150 supported observations**.
This is a completed calibration with no demonstrated useful upgrade decision,
not a qualification pass or proof that all possible inputs would defer.
Independent high-effort review verifies each rational LP optimum with exact
active-constraint dual certificates, as well as every encoded inequality and
saved price. It also confirms Astra's analytic diagnosis: with this fit and
support table, every supported forecast of at most 1,007 remaining shared steps
must defer, even before charging setup and startup. The largest supported
calibration forecast is 125 steps. Larger admissible horizons are not ruled out
by this bound. The fitted continuation coefficient is `t_0 = 16.74439684` and
`t_1 = 0`; no compatible-history credit is supported.
The pooled incumbent fit is bound by two Mac stagnation observations forecasting
11,968 remaining steps where the matched Never action executes 20. These rows
are outside the final support table but remain constraints under the frozen
global fitting rule. Across supported observations, predicted incumbent cost
is a median 0.078% of its measured label. Separate expensive Linux observations
bind setup, startup, reconstruction and continuation; the dimension predictors
also assign identical setup exposures to some cheap and expensive inputs.
Actual paired means show positive current-action savings for 107 of 180
interventions and positive savings including the suffix for 133; these are
descriptive calibration observations, not confidence bounds or unseen-input
guarantees. No row, margin or coefficient was changed after this diagnosis.
The coefficients are retained as evidence and have not been installed in the
runtime. Held-out and final qualification runs have not started; public two-FE
Auto remains disabled. Following this diagnosis, Johannes authorized careful
revision to optimize complete-command speed, explicitly rejecting conservatism
as an independent objective. The [revision objective](two_fe_auto_policy.md#revision-objective-2026-10-22)
opens the empirical modeling choices to independent review while retaining this
completed outcome and the separate validation and activation gates.

Two operational defects are retained. Linux job 8243850 rejected an output
directory outside the permitted task tree before creating outputs or making a
native call; correcting only that location enabled job 8244019 with identical
artifacts, inputs and budgets. The first offline Mac label collection also
failed because repeat matching required equality of host available-memory
limits. The narrow repair retains and validates each actual limit while still
requiring exact accounted memory, iteration work, routes and non-timing policy
labels. All 66 focused policy-tool tests pass, including an unmocked witness
that permits valid census variation but rejects changed accounted work and
over-limit use. The same observations were reused without any native rerun.

## Measured CMG setup and two-FE upgrade strategy (2026-10-21)

The initial diagnostic compares accepted source `3f9d54ee` with the pre-refactor
two-FE candidate `d5b06cf3` plus only `DEFAULT_TWO=true`, and with current public
Auto using the same current binary. The [compact evidence](../bench/results/cmg-setup-diagnostic-20261021.json)
retains all 1,980 complete-call timing observations, identities, comparisons,
phase observations, scheduler summaries and operational failures. No new runtime
or upgrade-policy change is included in these measurements.

All 24 comparison bounds pass on each platform. Linux uses six distinct isolated
28-core hosts, two mirrored blocks per cell and 1,080 calls. Mac uses 12 mirrored
blocks per cell on the active desktop and 900 calls. Input seed is 2026101821;
balanced order seed is 2026102103. There are no exclusions or measurement retries.
Independent numerical witnesses, exact own repeats and cross-worker science and
discrete checks pass. The scored timer is complete-call `NATIVE_SECONDS`; the
retained auxiliary `timing_field=native_s` label does not describe that score.

| High-degree weak graph | Linux current/public | Linux current/pre-refactor | Mac current/public | Mac current/pre-refactor |
| --- | ---: | ---: | ---: | ---: |
| W1 | 0.7458 | 0.9244 | 0.6693 | 0.9837 |
| W4 | 0.8612 | 0.7953 | 0.7978 | 0.8618 |
| W8 | 0.9706 | 0.7423 | 0.8976 | 0.8248 |

Ratios are paired median complete-call comparisons. This repeats the exact input
that previously failed on Linux at W4. The weak-low fixture also improves, but
its historical route refused the now-removed construction-work cap: those gains
combine cap removal with implementation changes. Repeated-cells and wide-rhs
finish before h16 and therefore do not exercise setup or RHS amortization.

A separate 18-call profile per platform (one warmup and two recorded observations
for each of two families at W1/W4/W8) preserves exact baseline science/signatures.
The [instrumentation patch](../bench/cmg_setup_profile.patch) uses the ordinary
construction path without additional admitted banks. On Mac, high-degree setup
falls from 51.465 to 23.957 to 19.153 ms, while hierarchy admission/build remains
9.890/10.010/10.049 ms. Linux on a separate reserved host reports setup
218.385/115.263/93.093 ms and hierarchy 30.297/46.151/46.179 ms. These small,
sequential attribution samples identify a hierarchy scaling limit; they do not
separate forest, contraction and factor costs or establish a causal worker-speed
comparison. Opus 5.5's source review prioritizes contraction sorting and initial
Laplacian sorting for the next setup changes, with h16 held fixed.

Independent review reproduced all timing summaries and inspected schedules,
signatures, host observations and accounting. Mac retained numerical arrays were
also independently checked locally. Linux's complete numerical-array check ran
in the successful remote report; the local reviewer inspected compact evidence.
All six primary Linux jobs `8225587`–`8225592`, report `8225643` and profile
`8225645` have scheduler failure/exit status 0/0. The failed initial smoke/deploy
and profile-copy attempts remain recorded. The executed profile helper had
weaker runtime identity guards than the diagnostic runner; retained identities
were checked separately. The maintained helper now enforces these guards and
has 12 focused binding/drift tests, without rerunning the completed profile.

These are instrumented diagnostic solves, not default-activation qualification
or uninstrumented release-speed claims. Per-cell intervals are not simultaneous
24-comparison coverage; distinct Linux hosts do not prove independence. The new
setup changes and deterministic RHS/core-aware policy still need implementation,
review and the fixed 42-cell qualification before public two-FE activation.

### Setup candidate 1: one counted in-place edge sort (pending review)

Cloud implementation of the first authorized setup candidate (ideas 1 and 2 of
the setup review) on runtime `3f9d54ee` / CMG `35a095f3` (fereg `fa1fbbe5`).
It is not self-accepted. CMG `7b610e1e37bc8a8b01f98a9957089d8fc10367ae` is
pinned by fereg `7fe3e7e` (manifest and both lockfiles); there is no fereg
source change. No timing, panel or profile ran in the cloud. Root owns the
controlled Linux/Mac comparison.

Declared before the edits:

- Change. Every canonical edge sort under a cancellation query uses one
  counted in-place introsort, `graph/edge_sort.rs` (renamed from
  `team_sort.rs`, now always compiled). It uses three-way partitions around a
  median-of-three pivot, insertion below 17 records and a heapsort fallback at
  depth `2 log2(n)`. A partition advances its meter by 1,024 units (16 per
  step) every 64 steps; the meter queries cancellation every 65,536 units,
  about every 4,096 partition steps plus bounded segment slack. (Corrected by
  the repair below; this bullet first said "polling every 64 steps".)
- Migrated paths. Serial `canonical` (contraction, serial initial graphs and
  owned compact edges) replaces the counted heapsort. The hosted initial
  graph keeps its fixed 8-record mapping chunks, then sorts in place on the
  team instead of using insertion runs and 14 merge passes through a spare
  bank; that merge code and its tests are removed. The prepared and LDL key
  heapsorts are unchanged.
- Order. The total comparator (endpoint key, then `total_cmp` weight) and
  bit-identical equal records leave every order, duplicate reduction,
  hierarchy, factor, repeat, representation and bit unchanged.
- Resources. Only the spare sort bank is gone. The unchanged public
  construction bound still counts two edge banks; the second covers the
  transient compaction copy that `from_sorted_edges` allocates while the
  merged bank is live. (Corrected by the repair below; this bullet first said
  the second bank is never allocated.) Retained and peak accounting, the
  thresholds (hosted path at 65,536 or more edges with any team, contraction
  team threshold) and worker selection are unchanged. No pool and no
  allocation are added.
- Polling. A serial sort counts on the caller's meter. On a team, partitions
  of at least 16,384 records fork within the ceiling, and every logical node
  of the partition tree also queries on entry with its own lane counter; the
  coordinator queries once more after the join. A successful run's query
  total and each node's logical checkpoints are ceiling-independent; the
  chronological order of queries and where a cancellation stops are not. The first
  team version shared counters across unforked recursion. It changed the
  hosted build's poll count with the ceiling (8,458 against 8,459), which the
  existing "poll schedule depends on the input, never on the ceiling" witness
  rejected. The per-node entry poll restores that property. The hosted
  sort's cancellation phase is now "team edge sort"; the serial phase stays
  "coarse edge sorting".

Witnesses and results (compact commands, real exits and log SHA-256 values
are in the JSON record; raw logs were session-local and are not retained in
the repository):

- New CMG tests:
  - Serial and team sorts equal the standard sort on random, duplicate-heavy,
    sorted, reversed, all-equal, two-value, organ-pipe, sawtooth and
    median-of-three-killer inputs, with signed zeros and a subnormal weight, at
    ceilings 1/2/3/8.
  - Cancellation observed inside the first partition of a 400,000-record sort
    stops untouched work. Serially no step follows; each team lane runs at
    most one cadence plus one segment. Lanes join and a fresh sort is exact.
    (Review: every stop here falls in the root partition before any fork, so
    this witness says nothing about concurrent lanes; the repair below adds
    the post-fork witness.)
  - The widest gap between polls is bounded serially and at ceilings 1 and 4,
    deep in the partition tree.
  - Successful-run team query totals are identical at ceilings 1, 2, 3 and 8.
  - A test-only range visit counter measures partition steps on subslices.
- Mutations (all exit 101):
  - s1, partitions never poll: killed by the in-partition cancellation
    witness.
  - s2, weight-blind comparator: killed by the equality witnesses.
  - s3a, one counter across tail iterations: killed by the poll-schedule
    witnesses (edge sort and hosted graph).
  - s3b, no node entry poll: killed by the gap witness. Its first single-file
    run survived because the gap witness did not exist yet; it is retained as
    superseded.
- CMG matrix on the final source: all 27 steps exit 0.
  - Tests: default 132, `parallel` 201, all features 258, each in debug and
    release.
  - Frozen bits pass in all eight configurations with the fixture file
    unmodified, so every frozen hierarchy, cycle, solve and plan digest is
    bit-identical.
  - Clippy for seven feature sets, rustdoc, and the benchmark crate pass.
  - Earlier full-suite passes (corrected attribution): run 100 failed the
    hosted-graph poll-schedule witness (8,458 against 8,459, the shared-counter
    defect above, fixed before run 101); run 101 failed the plan-cancellation
    sampling witness ("CSR construction finish" not sampled); run 102 passed
    (257 tests). The plan failure is a defect of the pre-existing witness
    design (sampled query ordinals under dynamic CSR chunk claiming), not a
    baseline failure that was observed: the base passed three isolated runs,
    which does not reproduce a flake. The repair below replaces it.
- fereg on the pinned tree: ratchet (no change), fmt and Clippy pass. The
  core lib passes 1,388 executions (1,374 top-level tests plus 14 child
  re-executions), 3 ignored. These include the 42 frozen no-team/W1/W4
  adapter digests, R1/R2, the above-floor projection and the allocation,
  publication, error, cancellation and fatal-classification tests.
- Actual-plugin replay of the candidate (`24ac8d1c...3308`) against a cloud
  build of `fa1fbbe5` (`a4c9bd11...42b6`): 73 of 73 cases exact. 46 derived
  explicit OLS and PPML runs (both bias modes, 1/4 workers) and 75 injected
  two-FE runs are row-identical to the M3d runs.
- Standard `./scripts/run_tests.sh`: 315 Python tests pass, then the
  installed-plugin binding check fails (exit 1). `--source-only` (labelled
  source-only; installed-package admission not performed) passes 322 Python
  tests and 1,965 Rust test executions in 83 summary groups (14 ignored).

Limits: no cloud timing. The candidate removes about 14 merge passes and the
spare-bank first touch in the hosted initial graph, and replaces about 1.35M
counted heap-sift steps in contraction with introsort partitions. Whether
this lowers the measured initial-Laplacian and hierarchy phases is for
root's controlled comparison. Contraction remains serial below 65,536 edges.
The terminal factor, forest granularity, component reuse and hosted emission
are untouched. Identities are in
[`cmg-setup-candidate1-20261021.json`](../bench/results/cmg-setup-candidate1-20261021.json).

### Setup candidate 1 repair: witnesses and evidence (pending review)

Root's review found no production sorting, order or resource defect but five
evidence gaps (R1 to R5). The repair is two CMG commits,
`b0b9aaf5c71f2f1ec669be73995c4f74da3c8abb` (parent `7b610e1e`) and
`1bed1672dd80ece3ff9c6c2074d3ff48588b722d`, pinned by fereg in the manifest
and both lockfiles on top of root's tooling commit
`1ee45cc4` (whose `bench/cmg_setup.py`, qualify hooks, tests and README are
untouched). It changes tests, test-only probes and documentation only:
`-Zunpretty=expanded` non-test release expansions of the CMG library at
`7b610e1e` and `1bed1672` are token-identical apart from doc comments for the
default, `parallel`, `experimental-components` and
`parallel,experimental-components` feature sets. No timing ran.

- R1, post-fork cancellation. The earlier in-partition witness stops in the
  root partition before any fork, so its W4 claims were vacuous. The new
  witness `started_lanes_cancel_inside_partitions_after_the_fork` parks every
  lane at its first query inside actual partition work past the root
  partition, using a test-only per-thread partition probe and the lane gauge.
  The lanes are released only when all `ceiling` lanes have arrived on
  distinct threads; at W4 these are the four one-lane leaves after the second
  fork, with four lanes active at firing. Parking is a mutex/condvar barrier
  with a 30-second escape that fails the witness instead of hanging; success
  depends on no sleep or schedule. After the firing, no partition step and
  no lane query follow (the only later query is the coordinator's poll after
  the join). Each parked partition still had unclassified records, all lanes
  have joined on return, query and visit counts do not move afterwards, and
  fresh team and serial sorts are exact. It runs at W4 and W1. The early-root
  and deep-gap witnesses are kept.
- R2, signed zeros. `Edge`'s `PartialEq` equates `+0` and `-0`. Every
  serial/team standard-sort and fresh-rebuild comparison now uses endpoints
  and `weight.to_bits`. A signed-zero duplicate-group input was added, and an
  explicit check shows that the bit comparison separates a swapped signed-zero
  pair that `PartialEq` accepts. The frozen fixtures are unchanged.
- R3, plan preparation. The full-suite failure of
  `plan_and_workspace_preparation...` (run 101) came from the witness design:
  it calibrates a global query ordinal and samples indices, but CSR fill
  claims chunks dynamically with persistent per-lane counters, so ordinals
  move and a single checkpoint can fall between samples. A test-only,
  allocation-free thread-local probe now records the phase of the
  `CancelMeter::poll` whose production query runs, and a guard restores it on
  return or unwind. Each required caller phase is cancelled by a sticky query
  that latches when the probe names it ("plan operators", "CSR row counts",
  "CSR row offsets", "CSR incoming scatter", "parallel CSR fill", "CSR
  construction finish"). For each, the witness requires that the phase was
  reached, that exactly that phase is reported, that nothing is published and
  that "plan publication" is never queried afterwards; the existing exact
  rebuild follows. The production query decides; the probe only observes.
  - The first latch design let both CSR-finish mutations survive (exit 0).
    The serial CSR path polls the same phase name, so the latch landed there.
    The repaired latch identifies the parallel finish (`csr.rs` near line 404)
    as the meter poll directly after the parallel fill's join poll, which the
    serial path never produces.
  - The construction witness latches "team edge sort" and the contraction's
    coordinator phases the same way. The mapping stage polls the raw query on
    its lanes, without a meter phase, so it stays covered by the bracketing
    sweep.
  - In plan preparation, ordinal sampling remains only as supplementary
    coverage. The contraction mapping phase is still required from its
    sampled sweep, as stated above.
- R4, documentation (CMG doc comments, `controlled-construction.md`,
  CHANGELOG, and the candidate entry above).
  - Cadence. A partition advances the meter by 1,024 units every 64 steps, and
    the meter queries every 65,536 units: about every 4,096 steps plus bounded
    segment slack, with extra node-entry and final queries on a team.
  - Ceiling independence. Successful-run query totals and per-node logical
    checkpoints do not depend on the ceiling; the chronological order of
    queries and where a cancellation stops do.
  - Edge banks. Only the spare sort bank was removed. The unchanged
    two-bank bound still covers the transient compaction copy that
    `from_sorted_edges` allocates while the merged bank is live.
- R5, failure evidence. The attribution of the earlier failures is corrected
  above. Run 100 was the 8,458/8,459 poll-count failure of the first team
  version; run 101 was the plan witness; run 102 succeeded.
  - The candidate-entry claim of three isolated passes on each tree came from
    unlogged runs: only their summary lines were seen, so no hashes exist.
    That claim is narrowed to a logged reproduction: three runs each on
    `7b610e1e` and `35a095f3`, all exit 0. No baseline failure was observed,
    and passing isolated runs do not reproduce a flake.
  - The initially surviving entry-poll mutation and the superseded mutation
    attempts are recorded with their commands, exits and hashes.
  - 257 against 258: run 102 predates `work_between_polls_is_bounded_deep_in_the_partition_tree`,
    which was added after the first entry-poll mutation survived. That is
    the only test-count difference between runs 102 and 323 (lib 97 → 98).
- Mutations, all applied to a copy of the repaired tree:
  - Started-lanes witness, first with W1 checked first (501–503), then with
    W4 first (521–523): a partition that swallows cancellation (W4: 1,199,971
    steps against 816,376 at firing), no in-partition poll (W4 returns
    `Ok`), and no coordinator poll after the join (W4: 204 queries against
    205). All exit 101.
  - CSR finish: deletion and a counting-only bypass of the parallel
    checkpoint. With the first latch design both exit 0 (retained); with the
    repaired witness both exit 101 ("CSR construction finish (after parallel
    CSR fill) was never polled"). Two earlier attempts whose anchor was not
    unique did not apply (exit 2) and are recorded as superseded.
- Deep-gap witness race (second CMG commit). The first repair matrix's
  all-features debug run (623, exit 101) failed the existing candidate
  witness `work_between_polls_is_bounded_deep_in_the_partition_tree` at W4
  (47,022 steps between polls against 16,640). That witness read the global
  step counter and stored it with a separate atomic swap, so a lane preempted
  between the two stores a stale reading that inflates the next gap. Lane
  segments depend only on the input's partition tree, and W1 runs the same
  nodes within its one-lane bound, so this was the measurement, not the sort.
  `1bed1672` reads and records under one lock. s1 (no in-partition poll,
  1,048,576 steps between polls serially) and s3b (no node-entry poll, 72,660
  at W1) still fail it (541/542, exit 101). The failed log is retained.
- CMG matrix on `1bed1672`: all 27 steps exit 0.
  - Focused repair witnesses (edge sort and bounded team): 10 of 10, three
    consecutive runs.
  - Tests: default 132, `parallel` 202, all features 259, each in debug and
    release. The one-test increase over the candidate's 201/258 is the new
    started-lanes witness.
  - Frozen bits pass in eight feature/profile configurations with the fixture
    unmodified.
  - Clippy for seven feature sets, rustfmt and rustdoc pass. The benchmark
    crate is not rerun: the non-test library code is token-identical.
  - The first repair matrix on `b0b9aaf5` passed every step except 623 (above).
- fereg on the pinned tree (no fereg source change; the ratchet inventory
  is unchanged): ratchet, fmt and Clippy pass, and the core lib passes 1,388
  executions with 3 ignored.
  - Plugin. A plugin built from the repair (`040e85a4...aa7b3`) differs in
    bytes from the candidate's (`24ac8d1c...3308`). Its build inputs differ in
    the build label, the CMG checkout revision and the line numbers in the
    edited files; the non-test CMG source is token-identical.
  - Replay against the `fa1fbbe5` cloud baseline (`a4c9bd11...42b6`) is
    exact again: 73 of 73 cases, plus 46 derived explicit OLS and PPML rows
    (both bias modes, 1/4 workers) and 75 injected two-FE rows with zero
    differences, matching the candidate's counts.
  - Standard `./scripts/run_tests.sh`: 315 Python tests pass, then the known
    installed-plugin binding check fails (exit 1). Not green.
  - `--source-only`: exit 0, 322 Python tests and 1,965 Rust executions in 83
    summary groups (14 ignored), counted from this run's summary tuples.
    Root's separate local source-only run on `c796a3b6`/CMG `7b610e1e`
    reported 1,951 Rust executions in 83 groups (exit 0). These are distinct
    runs; the cause of the difference has not been established, and each
    count stands for its own run. (Corrected: this bullet first attributed
    the difference to the counting methods.)

### Setup candidate 1 repair 2: post-fork witness accounting (pending review)

Root's second review found one remaining race in the R1 witness. It took
each query's ordinal before the barrier lock and recorded the firing lane's
ordinal plus one as the count at firing. A lane that took an early ordinal
but reached the lock last fired with a stale count, so the strict check that
only the coordinator queries after the firing could reject a correct join.
CMG `355d328a33639b5fa50c3c1954c8ee62e59e6b20` (parent `1bed1672`) is
test-only and pinned in the manifest and both lockfiles. Non-test expansions
for the four feature sets remain token-identical to `7b610e1e`.

- Queries are counted under the barrier lock, and the count at firing is
  read there once all lanes have entered. The strict `+1` coordinator check,
  the post-fork, untouched-work, join, quiescence and rebuild checks, and the
  30-second escape are unchanged. The other snapshots were reviewed:
  visits and active lanes are read under the lock while every lane is in the
  query, and the post-return reads follow the join.
- New regression
  `started_lanes_witness_counts_queries_under_the_lock_despite_ordinal_inversion`
  forces the inversion in the actual witness at W4. The first qualifying
  lane takes its ordinal under a selection lock and is held before the
  barrier lock until the three other lanes have parked with later ordinals.
  It requires the old snapshot to be stale by at least three queries and the
  locked count to be exact. Synchronization only, no sleeps for success.
- Checks (all exit 0 unless stated):
  - The two started-lanes tests, three times each in debug and in release
    (`parallel,experimental-components`).
  - All edge-sort tests in debug and release with `parallel` and with all
    features, and the default-feature edge-sort test.
  - rustfmt, and Clippy for seven feature sets including all features.
  - fereg: the pin resolves (`cargo fetch --locked`; fuzz
    `cargo metadata --locked`) and `cargo check --locked --workspace
    --all-targets` passes.
- Mutations, all exit 101:
  - Restoring the old pre-lock snapshot fails the inversion regression with
    205 queries against 202.
  - r1a, r1b and r1c fail the started-lanes tests.
  - s1 and s3b fail the deep-gap witness.
- Reused rather than rerun: the runtime, replay and native evidence of the
  repair above (production code unchanged), and the broad CMG matrix on
  `1bed1672` (only this test module changed).
- Documentation: the plan-preparation sampling sentence above is narrowed,
  and the 1,965/1,951 source-only counts are kept as two distinct runs
  without an attributed cause.

### Setup candidate 1: root validation and frozen comparison

The focused source-review gate passes at CMG `355d328a` and fereg
`0407060c`; both independent reviewers found no remaining issue in the
lock-counted witness. Root ran all seven edge-sort tests in debug and release.
The prior source-only integration, allocation, analytic, frozen-adapter and
portable evidence remains applicable because the repairs changed tests and
documentation only. Root did not rerun the broad matrix or claim independent
execution of the cloud expansion check.

Root's actual Mac universal build passed binary audit and host ABI/load/panic
checks. The 73-case replay passed with zero exact differences against accepted
`3f9d54ee`, with 146 additional worker-count checks. Eighteen explicit-solver
cases (54 calls) cover CMG-Schur and CMG/LSMR PPML with both bias modes, exact
same-worker repeats, independent mathematics and cross-worker decisions. All
six mapped Stata fixtures and the native optional-builder and sampled explicit
CMG-Schur Break/rollback checks passed. The Linux build/audit/load job
`8227206` exited 0 on four slots; this is build evidence, not timing.

The task smoke adapter initially discarded validated route records, so its
first Mac attempt failed the exact comparison after two scientifically valid
calls. Attaching those records, as the maintained runner already does, makes
those same saved calls pass without new execution. A bounded eight-call repaired
Mac smoke passed. The queued Linux smoke `8227525` was cancelled before execution;
its corrected eight-call replacement is `8227531`. Failures, commands, hashes,
compact native results and the small diagnostic helpers are retained in
[`cmg-setup-candidate1-20261021.json`](../bench/results/cmg-setup-candidate1-20261021.json)
under `root_validation`.

Before timing, the maintained `cmg-setup-candidate-v1` protocol is frozen:
accepted `3f9d54ee` versus candidate `0407060c`, both hidden h16 invocations,
four existing families, W1/4/8, input seed 2026101821 and order seed 2026102104.
Linux uses six distinct isolated 28-core hosts and 720 total calls; Mac uses
12 balanced blocks and 600 calls. The call timeout is 120 seconds, job budgets
are 3,600/14,400 seconds, and there is no resume, recovery or retry. Complete
native calls are scored; exact science, solver decisions and route/hierarchy
identity remain required. Ordinary desktop noise stays. The explicit-arm
profile helper passed 21 tests and independent review; its separate 18-call
panels provide attribution only. The following results complete that frozen
diagnostic; they do not qualify public two-FE activation.

### Setup candidate 1: completed paired measurement

All **1,320 complete calls** are retained in the candidate JSON's
`root_measurement`: 720 on six distinct isolated Linux hosts and 600 on Mac.
Every scientific, exact repeat, route/hierarchy, cross-worker and resource gate
passed. All 12 nonregression bounds per platform passed, with no exclusions,
retries or scientific/native failures. Independent reviews recomputed the
bounds from complete-call `NATIVE_SECONDS`, aggregating the two Linux blocks
within each host. Mac's 12 blocks share one active desktop. These are conditional
per-cell intervals, not simultaneous coverage or proof that hosts are independent.

High-degree-weak candidate/accepted median complete-call ratios were:

| Platform | W1 | W4 | W8 |
| --- | ---: | ---: | ---: |
| Linux | 0.997469 | 1.015255 | 1.027198 |
| Mac | 0.984409 | 1.005425 | 1.012140 |

This passes the declared max(5%, 1 ms) allowance; it does **not** establish a
general speedup. Repeated-cells and wide-rhs still finish before h16, so they
do not supply construction or RHS-amortization evidence.

The separate accepted/candidate profiles retain 36 calls per platform: one
warmup and two recorded calls for each arm, family and worker count. All match
the corresponding main-panel scientific fingerprints and route/hierarchy.
The high-degree phase medians below are descriptive milliseconds from those
two recorded calls, with accepted followed by candidate:

| Platform and phase | W1 | W4 | W8 |
| --- | ---: | ---: | ---: |
| Mac initial Laplacian | 16.027 → 8.918 | 5.337 → 4.389 | 3.608 → 3.814 |
| Mac hierarchy/finalization | 9.820 → 11.515 | 9.942 → 11.622 | 9.987 → 11.665 |
| Linux initial Laplacian | 48.660 → 34.825 | 21.269 → 18.592 | 14.294 → 16.028 |
| Linux hierarchy/finalization | 30.241 → 38.042 | 46.525 → 55.013 | 46.402 → 55.245 |

The profile is not a causal timing experiment or qualification. Source review
narrows a useful diagnostic: the high-degree graph has 41,722 canonical edges,
and all subsequent graphs are below the unchanged 65,536-edge parallel
contraction floor. Its coarse sorts therefore use the changed serial
heapsort-to-introsort path at every worker count. `hierarchy_team=true` does
not imply parallel contraction, and the broad hierarchy phase also includes
components, centering and terminal factorization. These samples did not identify
the responsible kernel; the attribution exercise below separates sort operation
counts from the other hierarchy and finalization costs.

Operational failures remain separate from these measurements. Linux profile
build `8227507` exited zero but reused the uninstrumented Cargo artifact because
the patched archive preserved old source mtimes. Identical binary hashes caught
this before profiling. A fresh isolated build cache in `8227545` produced the
distinct, audited profile artifact used above. The first six comparison host
requests selected a queue that does not support `omp28`; all six jobs were still
queued with no calls when their host requests were corrected. The same job IDs
and frozen budget then ran on xa1–xa4/xb1–xb2. A Python 3.6 inspection-script
failure occurred before any host alteration and is also retained.

The six comparison jobs `8227633`–`8227638`, report `8227775`, and separate
profile `8227776` all have complete 0/0 accounting. Remote reporting checked
Linux native heads and arrays; the independent local Linux audit checked compact
records and host evidence. Mac retained heads/arrays were independently checked.
The maintained runner deletes timed raw logs after validation; their complete
observations remain in the compact record. Profile archive/build hashes are
bound, while source-to-binary provenance additionally rests on root's source,
patch and build-command checks. No separate Mac profile host observer was
recorded. Neither setup diagnosis nor these profiles activate public two-FE Auto.

### Setup attribution tooling and fixed diagnostic protocol

This is maintained diagnostic tooling. It separates two explanations of the
candidate's broader descriptive hierarchy/finalization phase: serial
raw-edge sort operations, and the remaining hierarchy and finalization
stages. Claude's tooling delivery changed no production code and ran no real
workload timing, SCC job, plugin build or replay. It started at root's
`25a68ead`; CMG stays at `355d328a`. Root's subsequent controlled diagnostic
execution is recorded below.

`bench/cmg_setup_attribution.py` instruments only private copies.

- **Preparation.** It exports the exact arm commits (accepted
  `3f9d54ee`/`35a095f3`, candidate `0407060c`/`355d328a`) with `git archive`
  and checks pristine file hashes. It then applies root's unchanged
  `cmg_setup_profile.patch` (SHA-256 pinned). The attribution edits follow;
  every anchor must occur exactly once.
- **CMG dependency.** The copy's CMG dependency points to the patched copy,
  and builds stay `cargo --locked`. A diff, a deterministic source archive
  and an `attribution-source.json` are written.
- **Added module.** `setup_attribution.rs` holds fixed atomic tables and
  per-thread counter cells. There is no admitted bank, factor walk,
  constructor, pool or graph copy, and no comparison, query or meter
  arithmetic changes.
- **Modes.** A compile-time constant selects a counts-only build or a
  timers-only build. Counter-build time is never evidence.
- **Counts.** One record per canonical raw-edge sort, with its contraction
  level or outside site and its serial/team/hosted path. Only serial sorts
  on the calling thread are counted:
  - comparator calls;
  - partitions, partition steps and swaps;
  - insertion leaves, records and swaps;
  - heapsorts, records, sift steps and swaps;
  - the caller meter's units and queries.

  The shared heapsort counts only inside an active canonical scope, so
  prepared and LDL sorts stay out.
- **Timers.** Every existing `measure_hierarchy_phase` boundary per level,
  pruning, `hierarchy_total`, finest components, coarse centering, the
  terminal factor, `finalize_total` and `build_with_total`, plus sort and
  sort-with-post-sort-graph-finalization times per record. This latter span
  includes allocation, duplicate reduction, compaction, invariants and graph
  publication, not duplicate summation alone. The validator checks the nesting and
  returns disjoint `exclusive_ms` pieces whose sum equals the fereg profile's
  hierarchy phase. Inclusive timers are never added.
- **`run`.** It binds through `cmg_setup_profile.load_context` and runs that
  helper's fixed 18 calls with exact science, signature, route and hierarchy
  checks. In addition it requires per call:
  - one built six-phase profile line and one attribution line;
  - no overflow or unknown phase;
  - the arm's algorithm identities;
  - contraction records that match the contraction timers;
  - the unchanged meter cadence.

  Within a case, every call must also have identical sorts, and identical
  counters in counts builds.

Focused checks, all exit 0 on the final tool:

- Preparation and build inputs, for each of the four arm × mode copies:
  `prepare`, `cargo metadata --locked` and `cargo check --locked -p
  fereg-plugin`.
- CMG `frozen_bits` in release, and the graph/hierarchy/preconditioner lib
  tests (31 accepted, 34 candidate), on each instrumented copy.
- Smoke harness. Two synthetic graphs at W1/W4 built through the route's
  calls. Hierarchy, repeat, factor and apply-bit digests are identical to the
  pristine source and across arms and modes. The banded graph reaches a direct
  terminal factor. Real attribution lines validate.
- Tests: 23 new tests (real fixtures, fail-closed rejections, a mocked
  18-call `run`); 73 setup tests and the full bench suite (264, 1 skipped)
  pass under `discover`; the complexity ratchet passes.

Earlier passing runs on older tool revisions, an invocation error (root
setup tests need `discover`) and one cloud command refused by the session
safety check are recorded. The record is
[`cmg-setup-attribution-tool-20261021.json`](../bench/results/cmg-setup-attribution-tool-20261021.json).
It holds file and prepared-source hashes, commands, exits, log hashes,
deterministic synthetic-smoke counts, root's proposed protocol (four builds
per platform, 18 calls each, high-degree-weak and weak-low) and the limits.

- **Can distinguish:** per-level operation counts of the accepted heapsort
  against the candidate introsort, and which disjoint stage holds an
  accepted-versus-candidate difference, descriptively.
- **Cannot distinguish:** per-operation cost, causality, or work inside team
  lanes. Hosted and team sorts are recorded by size and timed only, and timer
  builds carry a small overhead.

Root review of `0fc4661` found missing mandatory timer coverage, permitted
negative exclusive durations, nonfinite profile values, delayed determinism
checks and two overbroad labels. The repaired parser requires each phase dictated
by independent route metadata, strict nesting at each level and in aggregate,
and finite nonnegative durations. It stops on the first changed sort/counter
record and retains that call. Swap counts are logical calls, not exact moves;
post-sort graph finalization includes allocation and compaction. Source archives
also preserve executable file status. All 80 root setup-tool tests pass; both
independent reviewers cleared the repairs. The four prepared Rust patches match
the delivered patches exactly, so valid unchanged compilation/smoke evidence is
reused. Commands, exits, hashes and the rejected atomic patch attempt are retained
in the tool evidence's `root_review`.

The [fixed diagnostic protocol](../bench/results/cmg-setup-attribution-20261021.json)
freezes four separate arm/mode builds per platform and 72 calls each on Linux and
Mac: two families, W1/W4/W8, one retained warmup and two recorded calls. Each build
uses a fresh cache and actual native audit/load checks. One isolated Linux host
and the active Mac desktop receive process observation. There are no measurement
retries or exclusions; a failed gate stops dependent calls. Count-build elapsed
times are not performance evidence. Two sequential timer observations provide
descriptive attribution only. The completed main comparison remains immutable.

### Measured hierarchy attribution, 2026-10-21

The diagnostic record retains every call, including warmups, with separate
counts-only and timers-only artifacts. Fresh builds perform actual binary
audit, ABI/load and injected-panic checks. The numerical oracle, selected hidden
scientific fingerprint, route and hierarchy checks run before each call is
accepted. Independent review reparses retained attribution logs and verifies
fingerprints and traces; raw numerical arrays were removed after the maintained
checks, so this later review does not independently recalculate their oracles.

All 144 calls pass: 72 on Mac and 72 on Linux. Counts repeat exactly at W1/W4/W8
and match across the two platforms. High-degree-weak's three serial contraction
sorts show the following work:

| Level | Input records | Accepted comparisons | Candidate comparisons | Candidate fallback calls / records |
| --- | ---: | ---: | ---: | ---: |
| 0 | 41,502 | 1,181,027 | 1,891,031 | 61 / 29,365 |
| 1 | 27,634 | 751,998 | 1,136,978 | 50 / 18,583 |
| 2 | 21,172 | 557,175 | 827,274 | 20 / 12,792 |
| Total | 90,308 | 2,490,200 | 3,855,283 | 131 / 60,740 |

The candidate classifies 2,410,573 records through partitions before falling
back to heapsort on 67.3% of the total sort input. Comparisons increase 54.8%;
logical swap calls increase from 1,324,608 to 3,189,945. These include self-swaps
and do not measure memory traffic. Cancellation queries change only from 1,359
to 1,368. The evidence identifies partition work followed by substantial
fallback, but the precise bad split pattern is not recorded.

Timer-only means of the two recorded calls, in milliseconds:

| Platform and span, accepted → candidate | W1 | W4 | W8 |
| --- | ---: | ---: | ---: |
| Mac contraction sorting | 5.145 → 6.665 | 4.932 → 6.750 | 4.916 → 6.492 |
| Mac complete hierarchy/finalization | 10.462 → 12.018 | 10.384 → 12.262 | 10.402 → 11.915 |
| Linux contraction sorting | 16.940 → 27.891 | 25.786 → 32.875 | 26.427 → 33.764 |
| Linux complete hierarchy/finalization | 30.649 → 43.076 | 47.462 → 54.367 | 47.414 → 55.087 |

Mac terminal factorization remains near 4 ms; forest work and post-sort graph
finalization show no comparable increase. Weak-low has one initial serial sort
and no contractions: comparisons fall 31.0% with no fallback, yet its observed
Mac sort time increases about 0.10–0.11 ms. Counts alone do not predict wall time.
These sequential timer observations locate a useful repair target; they do not
establish causality, a general speedup or qualification. Individual fallback
sizes and per-operation costs are unavailable. The 13 Mac process observations
have a maximum gap of 5.212 seconds, with no recorded competing compute or
observer error; coverage is sampled and calls lack individual observation spans.
Linux records 42 process observations (maximum gap 6.761 seconds), five all-queue
censuses and two physical-host reservation checks, with no competing compute or
observer error. Both independent reviewers verified the retained Linux evidence
and the cross-platform counts. SCC build `8229404` and run `8229425` have complete
0/0 accounting, taking 489 and 224 seconds respectively; the run used all 28
physical cores of wo4 for the four artifacts.

The initial preferred wp3 placement remained queued. A placement-only adjustment
to idle wo4 first replaced the hard resource list; root caught this while the
job was still queued and restored all frozen limits before execution. No calls
ran under the incomplete request. The scheduler cause of the original wait was
not established. Read-only inspection failures and a retention-script privacy
guard false positive are also recorded; no measurement was retried or excluded.

The next selected formulation is a fixed deterministic ninther pivot on large
partitions, with median-of-three on small partitions and the current depth-limit
heapsort fallback. This is a generic sort repair motivated by the observed work,
not a demonstrated speed improvement. It must count added pivot work, retain the
maximum unpolled-work bound and exact final edge order, and preserve host-team,
resource, failure and cancellation behavior. Root will validate the stable
implementation and measure it separately. No two-FE policy or public activation
follows from these diagnostics.

### Ninther pivot repair (accepted for policy development; timing limits retained)

On 2026-10-21, after reviewing the complete Mac/Linux comparison and the
alternatives, Johannes accepted this setup version and authorized development
of the new deterministic two-FE OLS Auto policy (option 1). The accepted
implementation is fereg native source `1cdb225e2460e391ad38f856755ae3ea5efb2e5b`
with CMG `fd77d98da1f74009b3e5b634e1944433b647a9b1`; the complete evidence is
recorded at fereg `4e7217a8d060414a715d06d604ee67a39f1d847b`.
This is milestone acceptance with disclosed uncertainty: the two inconclusive
bounds below remain inconclusive, and no observations are added or excluded.
Proceed with independent GPT Pro and Claude Fable proposals and cross-critique,
then implementation and the separate fixed-candidate qualification. Public
two-FE activation still requires all applicable scientific, resource,
transaction, 42-cell Mac/Linux and exact public-versus-hidden gates. Main,
release packages and GitHub testing remain unchanged.

This is the one selected generic repair to the shared canonical raw-edge
introsort, CMG `aaf6ec4c6b88b69c58024cc18f60e6abd5754a5a` (parent
`355d328a`). The delivery on fereg `606b6f6b` pinned it in the manifest and both lockfiles
after root's `04d99221`. The current pin `fd77d98d` adds only the reviewed
documentation corrections; CMG source, manifest and lock bytes are unchanged. Its scope was declared before the edits: the final order, duplicate
reduction and every successful downstream scientific bit are unchanged.
Pivot sampling and the resulting partition tree change work counts and query
positions; the cadence, boundedness and worker-ceiling independence remain.
No timing ran during the delegated repair.

- **Rule (frozen).** Partitions of more than 40 records take Tukey's ninther
  (Bentley and McIlroy 1993). With `s = len / 8`, `last = len - 1` and
  `m = last / 2`, the pivot is the median of the medians of positions (0, s,
  2s), (m - s, m, m + s) and (last - 2s, last - s, last). Partitions of 17–40
  records keep the median of first, middle and last records, now an explicit
  median using at most three comparisons with the same value. The insertion
  cutoff, depth
  limit and heapsort fallback, fork threshold, lane ceiling, comparator and
  hierarchy thresholds are unchanged.
- **Rationale.** After three-way partitioning, first/middle/last sampling
  can misjudge the median of structured subranges, split lopsidedly and
  exhaust the depth limit. Nine evenly spaced samples provide a broader
  estimate using at most twelve comparisons per large partition. This is a
  generic motivation, not a proven diagnosis of the retained contraction
  arrays. The rule is not adversary-proof.
- **Counted work.** Pivot selection is conservatively charged after it runs:
  192 units for at most twelve comparisons in a ninther and 48 for at most
  three in a median of three (16 units per possible comparison). Both fit
  within one 1,024-unit segment, and the 65,536-unit query cadence is
  unchanged. Between two queries there are at most 65,535 + 1,024 counted
  units, plus one uncounted bounded segment: 64 partition steps, one pivot
  selection (at most twelve comparisons and nine record copies), one insertion
  record or
  one heap step.
- **New witnesses.**
  - The pivot equals the declared sample rule, computed independently, for
    lengths 17 to 4,097.
  - At 65,536 records, structured orderings have no depth-limit fallback
    and at most 32 counted units per record per level, with every advance at
    most 1,024 units. The orderings are sorted, reversed, organ pipe, one
    record moved, descending blocks, interleaved runs, sawtooth, few values,
    duplicates and Musser's median-of-three killer.
  - A McIlroy adaptive adversary built against this algorithm still reaches
    the fallback. Its output is exact serially and at W1/W4. A finite fixture
    checks a counted-work bound; the general O(n log n) guarantee follows
    from the retained depth limit and heapsort fallback.
  - Cancelling at a partition's first advance stops it after exactly the
    pivot's units, with no record classified.
  - The new orderings were added to the serial and team bit oracles.
  - The early-root, deep-gap, post-fork inside-partition, ordinal-inversion
    and public phase-query witnesses are kept.
- **Mutations.** All are killed (exit 101): the old median-of-three rule
  (sorted: 14 fallbacks over 61,688 records), uncounted pivot work, an
  undercounted ninther, no depth fallback, a middle-only ninther,
  swallowed/unpolled/unjoined cancellation, the pre-lock snapshot, partitions
  without polls and no node-entry poll.
  - The middle-only ninther first survived; the positional witness was
    added and kills it.
  - The no-fallback mutation first ran a quadratic 200,000-record adversary
    and was stopped (SIGTERM, not counted as a kill); its focused rerun
    fails.
- **CMG matrix: 30/30 exit 0.**
  - Focused witnesses run three times.
  - Tests: default 135, `parallel` 207, all features 264, each in debug and
    release.
  - Frozen bits pass in eight configurations.
  - Clippy for seven feature sets, rustdoc, and the bench crate pass.
- **fereg on the pin.**
  - Ratchet, fmt and Clippy pass; the core library passes 1,388 tests with 3
    ignored.
  - The replay is exact against both the stable pre-repair candidate
    (`0407060c`/`355d328a`) and the accepted `fa1fbbe` baseline: 73/73
    cases, 46 derived explicit OLS/PPML rows (both bias modes) and 75
    injected two-FE rows, with no route change.
  - Standard `run_tests.sh`: 315 Python tests pass, then the known
    installed-plugin binding check fails (exit 1).
  - `--source-only`: exit 0, 322 Python and 1,965 Rust executions in 83
    groups, 14 ignored.
  - Bench tests pass (one skipped).

Independent high-effort and bounded Astra source/evidence reviews found no
production correctness blocker. Both confirmed the sample rule, conservative
pivot accounting, bounded polling, exact total order, retained fallback and
cancellation witnesses, and matching pins. They reviewed the reported checks
as recorded evidence without rerunning tests or native calls. Their reporting
corrections are incorporated here and in the compact record.

Root's local source integration passed on the synchronized pin: 322 Python
tests and 1951 Rust executions in 83 result groups, 14 ignored.
The complexity ratchet and all 11 focused edge-sort witnesses passed in debug
and release. These counts are separate from the cloud report above. The standard
installed-package binding remains a known failed check; source-only success
does not qualify release packages. Exact commands, exits and log hashes are in
the compact record.

Root then built the exact fereg `1cdb225e` / CMG `fd77d98d` pair with Rust
1.85.1, locked release/fat LTO and bridge O3 in fresh caches. Actual Mac and
Linux plugins passed audit, ABI/load and injected-panic containment; Linux also
passed the input/output guards. Linux job 8232322 ended with failed 0 / exit 0. The Mac replay passed
73 accepted-baseline cases and 146 W4/W8 checks; 18 explicit OLS/PPML rows
(both bias modes, 54 calls) passed mathematics, exactness and worker checks.
All six mapped Stata fixtures and actual builder/explicit-CMG Break and rollback
checks passed. Native hashes and check identities are retained in the record.

The maintained comparison and attribution helpers now bind a separate
three-arm pivot mode; historical definitions remain intact. Root's 104 focused
tooling tests passed, and the attribution helper's synthetic pristine versus
instrumented W1/W4 smoke passed all 16 calls. One initial fixture test failure
is retained: newly captured Mac application hashes differ from historical
fixture hashes. New modes bind their own pristine Mac reference exactly;
historical equality and live smoke checks remain exact. All fixture hierarchy
fields agree. No cause for the cross-origin application-bit difference is claimed.

Before collecting new performance data, the
[`cmg-setup-pivot-v1` protocol](../bench/results/cmg-setup-pivot-20261021.json)
freezes three distinct actual artifacts: accepted `3f9d54ee`, pre-pivot
`0407060c`, and repaired `1cdb225e`. The same four inputs and W1/W4/W8 run in
six isolated Linux host allocations with two mirrored blocks (1,080 calls),
and twelve Mac blocks (900 calls). One warmup per arm/cell/host is retained.
Both references receive separate exact-science and max(5%, 1 ms) comparisons.
There is no retry or resume. A separate 144-call attribution round covers
pre-pivot and repaired timer/count variants on the two setup-sensitive families.
Unchanged pre-pivot instrumentation archives and binaries are reused with
their original build evidence; new aliases cannot change either source revision.
The Linux preflight has twelve scientific calls whose times are excluded.
This protocol does not qualify public activation.

The frozen Mac comparison completed all 900 calls with exact science,
repeatability, cross-worker and resource checks passing. Of the 24 performance
bounds (twelve cells against two references), 23 passed and one is inconclusive:
weak-low at W8 against the accepted baseline. Its median candidate/reference
ratio is 0.993812, but the upper bound for excess time beyond max(5%, 1 ms) is
+0.783814 ms (median -2.451567 ms, lower -3.579015 ms, twelve mirrored-block
units). This neither establishes a regression nor satisfies the nonregression
gate. The runner's `diagnostic_complete` status means the observations are
complete; it does not mean all bounds passed.

The independent high-effort review recomputed all 288 paired block scores and
24 bounds and verified the frozen schedule, identities and scientific checks.
The 100 process observations have a maximum gap of 5.283 seconds, with no
recorded competing compute or observer error. Coverage is sampled, ordinary
desktop noise is retained, and the per-cell bounds have no simultaneous
24-comparison guarantee. No calls were retried or discarded. All 900 call
objects, the full report and the review are retained in
[`cmg-setup-pivot-mac-20261021.json`](../bench/results/cmg-setup-pivot-mac-20261021.json).

Linux attribution build 8232539 and twelve-call scientific preflight 8232540
completed with failed 0 / exit 0. The fresh Linux instrumented plugins passed
native checks and were collected and independently rehashed. After reviewing
the inconclusive Mac result, Johannes authorized completing the frozen Linux
comparison and separate attribution. The fixed Mac comparison budget remains
exhausted; no extra samples are added. Policy advancement and activation remain
held for review of the combined evidence.

The separate Mac attribution completed all 72 calls with exact science,
route/hierarchy and count-repeat checks. An independent review reparsed every
raw log and verified the source, actual binary, build and ordinary-panel
baseline bindings. High-degree-weak's three serial contraction sorts use
1,362,820 comparisons versus 3,855,283 before the pivot repair; partition steps
fall from 2,410,573 to 1,081,481, depth-limit fallbacks from 131 to zero, and
cancellation queries from 1,368 to 337. Counts repeat exactly across W1/W4/W8.
These inputs demonstrate less sorting work, not elimination of adversarial
behavior. The fixed depth-limit fallback remains protected by generic tests.

Timer-only means of two recorded calls put contraction sorting at 3.436,
3.352 and 3.346 ms (W1/W4/W8), versus 6.587, 6.386 and 6.564 ms before the
repair. Entire hierarchy means are 8.586/8.550/8.609 ms versus
11.845/11.623/11.926 ms. Weak-low has no contractions; its initial serial sort
uses 189,827 rather than 212,770 comparisons, with no fallback in either
version. Its total setup means are mixed, so fewer comparisons alone do not
predict elapsed time. The twelve process observations have a maximum gap of
5.221 seconds, with no recorded competing compute or error. These sequential
two-call observations are descriptive; the earlier Mac comparison remains
inconclusive. Full calls and evidence are retained in
[`cmg-setup-pivot-attribution-20261021.json`](../bench/results/cmg-setup-pivot-attribution-20261021.json).

Linux attribution also completed all 72 calls, making 144 retained calls across
the two platforms. An independent review reparsed all Linux raw logs and
matched every retained call to its source result. Counts agree exactly across
platforms, workers and repeats. The fresh Linux pivot binaries and source
captures were independently rehashed; the reused pre-pivot binaries are bound
by their original build and run records, without a separate local rehash of
those two binary files.

Linux timer-only means for contraction sorting are 9.850/14.867/14.892 ms
(W1/W4/W8), versus 22.018/32.890/33.117 ms before the repair. Entire hierarchy
means are 23.951/36.723/36.538 ms versus 35.986/54.475/54.633 ms. Weak-low
hierarchy means remain approximately 4.9 ms. These two recorded calls per
cell are descriptive. Run 8233861 completed on `scc-wo4` with 28 reserved
physical cores, failed 0 / exit 0, 230 seconds and 1.231G maximum virtual
memory. Its 42 process samples, five queue censuses and two reservation checks
passed, with a maximum process-sample gap of 6.920 seconds. No calls were
retried or excluded. The Mac comparison's inconclusive bound remains unchanged.

The ordinary Linux comparison completed all 1,080 calls on six distinct
28-core physical hosts. All scientific, repeat, cross-worker, cross-host and
resource checks passed; every job has failed 0 / exit 0. The maintained
aggregate report passes 23 of 24 bounds. High-degree-weak at W1 against the
accepted baseline is inconclusive: median ratio 0.984011 and excess-time
median -41.314716 ms, lower -58.041011 ms, upper +28.535722 ms. The six host
medians, each based on two mirrored blocks, are the analysis units. This does
not establish a regression, but it does not satisfy that nonregression bound.
All twelve Linux comparisons against the pre-pivot candidate pass.

An independent statistical reconstruction reproduced all 288 paired block
scores and 24 bounds within floating-point rounding (maximum difference
7.8e-16). The upper bound for the inconclusive cell comes from `wo4`: its two
block excesses are +60.438579 and -3.367134 ms, yielding a host median of
+28.535722 ms. These observations establish neither a cause nor an exclusion
basis. At six hosts, the selected one-sided .025 upper bound is the maximum
host excess.

All 1,080 complete call objects, the full report, accounting and host summaries
are retained in
[`cmg-setup-pivot-linux-part1-20261021.json`](../bench/results/cmg-setup-pivot-linux-part1-20261021.json)
and [`cmg-setup-pivot-linux-part2-20261021.json`](../bench/results/cmg-setup-pivot-linux-part2-20261021.json).
The 232 process observations have a maximum gap of 6.710 seconds, with no
recorded competing compute or observer errors. All reservation and all-queue
census checks pass. Coverage is sampled; six distinct hosts do not by
themselves prove independent scores, and the per-cell intervals do not provide
a simultaneous guarantee for all comparisons. No calls were retried or excluded.

Independent execution/science review matched all six full result objects and
their source hashes to the retained parts, rehashed all 216 retained numerical
heads/array sets, and verified the schedules, cross-arm/repeat/worker/host
science, accounting and raw host observations. All 216 retained warmup timers
match `NATIVE_SECONDS`; scored `seconds` is the complete-call timer, not the
auxiliary `native_s`. Timed raw logs were deleted by the maintained runner.
The candidate binary, available inputs/references and execution sources were
independently verified; the two reference binaries and Linux timing wrapper
were unavailable for independent local rehash. Independent numerical oracle
execution was not repeated.

Queued-only placement changes preserved the same job IDs, all resource limits,
the six-distinct-host requirement and the fixed call budget. A later quota
inspection explained the abandoned `wp3` placement: that host is restricted to
two other users. An independent scheduler check found valid requests and idle,
eligible public queue instances for the five delayed jobs, but could not
establish their rejection reason. Scheduler rejection details were disabled
and the available schedule file was stale. These jobs subsequently dispatched
at 17:28-17:29 and completed during 17:31-17:32 on their unchanged final hosts; no
measurements were resubmitted or added. The earlier delay remains unexplained.

Across Mac and Linux, 46 of 48 bounds pass and two remain inconclusive, both
against the accepted baseline. The pivot repair demonstrably reduces sorting
work on the diagnosed inputs, but the fixed comparison does not establish
full nonregression. Johannes subsequently accepted this disclosed setup
milestone for policy development as recorded above. Public activation remains
subject to the separate qualification; neither platform is resampled to obtain
a pass.

Limits: counted units are generic work, not cost or speed. The real
contraction split pattern was not observed. Neither the native checks nor this
inconclusive comparison establish a general speed improvement or authorize
activation. The source/native repair record is
[`cmg-pivot-repair-20261021.json`](../bench/results/cmg-pivot-repair-20261021.json).

### Prospective policy qualification: distinct Linux hosts per cell

The new `min_distinct_hosts_per_cell` option in `bench/qualify.py` makes the
authorized twelve-host requirement explicit in the frozen plan. It checks
every expected workload/worker/reference combination against normalized,
accounted hostnames, retaining all observations and grouping repeated
allocations on the same host into one uncertainty unit. Missing coverage
cannot pass through favorable timings or host diversity elsewhere in the
panel. Existing manifests without this option retain their original semantics;
the prior setup measurements and their two inconclusive bounds are unchanged.

The focused command `python -m unittest discover -s bench/tests -p test_qualify.py`
passed all 54 tests (exit 0, Python 3.13, numerical-library thread limits set to
one). Its output SHA256 is
`71465bdcbcced15cdc1f51e2392ae47f5e908442410fa6197a31b8f8e88741f1`.
The discriminating fixture supplies twelve hosts globally but six per cell:
timing bounds pass, while the new coverage gate rejects every reference.
Adequate coverage, hostname aliases, missing cells, malformed minima and
historical omission are also covered. Independent source review found no
blocker. Syntax and whitespace checks pass. This tooling change makes no
native calls and establishes neither statistical independence nor a policy
performance result.

## CMG successor milestone 3d: worker-independent team arithmetic

**Johannes accepted M3d's disclosed final scope on 2026-10-21 after independent
source/native review and the completed fixed host round.** M3d starts
from the accepted M3c pair (fereg `27fa40cd`, tested source `71092c38`,
production correction `c89ab124`; CMG `82ae1c86`, production `65bd817a`),
including the root R1 lookup correction and the cross-thread allocation
witness. It pins CMG `35a095f3e26972fcfa9e58c2222a1098efb8e607`. Root owns the
independent review, native, 42-cell and fixed Linux/Mac checks on the stable
candidate. No timing, panel or profile ran in the cloud.

Survey before the edits (read-only). With fixed inputs and thresholds, CMG
result bits differed only between one worker and two or more. Three gates
caused this: PCG dot and norm sums used the fixed-chunk tree only with
`worker_ceiling() > 1`; single-component centering (the finest PCG level and
scalar-cycle coarse levels) did the same; and `ParallelPcgSolver` gave a team
only to the planned mode, while serial solves and across-RHS lanes ran the
team-free recurrence. The tree's shape already depended only on the leaf
count. CSR row operators (gather) already reproduced the edge-list products
(scatter) bit for bit, and construction was worker-independent.

Declared before the edits:

- Design. A team-carrying solve or application chooses its arithmetic from
  lengths and the team's fixed thresholds only: the fixed-chunk tree from
  `max(min_parallel_len, 8 * reduction_chunk_size)` entries (131,072 with
  fereg's defaults), also at one worker. Serial solves and each concurrent
  right-hand-side lane run on `Team::single_lane()`, a one-lane view of the
  same pool (no pool, no allocation, no nested forks beyond the ceiling).
  Team-free calls keep the serial compensated sums. Mode selection stays a
  scheduling decision.
- Paths. CMG `pcg.rs` (reduction team, concurrent lanes, owned concurrent
  solves), `components.rs` (centering gate), `parallel_solver.rs` (serial
  context), `execution.rs` (`single_lane`; a test-only thread-aware lane
  gauge); fereg `fe/cmg.rs` (the caller-buffer serial context uses
  `ParallelPcgSolver::serial`).
- Numerical scope. Explicit `cmg` (OLS and PPML, both bias modes) changes
  rounding where a reduced or centered vector reaches 131,072 entries: at one
  worker, and in serial or across-RHS solves at any worker count. Multi-worker
  planned results are unchanged. Explicit `cmgschur` changes only in one-worker
  single-RHS cycles with a single-component coarse level of that size. After
  the change, CMG solves and applications with identical inputs, hierarchy,
  options and fixed thresholds are bit-identical across worker ceilings and
  solver modes on one platform. This does not imply identity of whole fereg
  estimates; pre-existing outer arithmetic differences are disclosed below.
  Auto (team-free cycle), injected two-FE and public Auto routes, construction
  and hierarchies are exact. Iteration logic,
  certification, tolerances and discrete decisions are unchanged.
- Resources. No new allocation, bank, pool, admission or memory policy.
- Representation (narrowed from the plan's M3d list). Row-operator selection
  stays worker-dependent. It is bit-neutral, so making it worker-independent
  would only add CSR memory at one worker, a resource policy not authorized
  here. A witness pins the bit neutrality.
- Optional Auto RHS batching: not taken (new arithmetic and storage scope for
  a measured low ceiling).
- Witnesses. Reductions and centering identical at ceilings 1/2/3/4/8 and on
  the one-lane view. Solver modes (serial, planned, across-RHS) and
  caller-buffer contexts identical at 1/2/4/8 workers, with all three modes
  exercised. Row operators vs one-lane applications identical. An
  independent edge-list residual reproduces the reported residual, and the
  certificates hold. Old-vs-new frozen digest mapping. Many-leaf cancellation
  on the one-lane view. A fereg explicit `cmg` projection above the threshold
  is bit-identical at one and four workers, checked against an independent CG
  oracle and the full-FE certificate. Explicit OLS/PPML replay in both bias
  modes.

Validation reported by Claude (cloud; commands, exits, log hashes and compact
outcomes are retained in the result record):

- CMG `35a095f3`: the 27-step matrix runs on the final source. fmt, tests
  (default 131, `parallel` 200, all features 257, each in debug and release),
  frozen bits in eight configurations, rustdoc and the benchmark crate pass.
  Clippy first failed on a type-complexity lint in the new test file; after
  the test-only alias, Clippy passes for all seven feature sets. The failed
  logs are retained.
- First passes before the final source failed as intended: the old
  one-worker-equals-serial reduction test, and the lane gauge. The gauge
  counted an inline one-lane primitive inside a concurrent right-hand-side
  lane as a second lane (6 on a ceiling of 3) although no extra thread ran.
  It now counts threads. The frozen digests then changed as declared.
- Frozen mapping over all twelve sections. Non-parallel sections are
  unchanged. 180 parallel digests changed: 88 equal the old planned
  four-worker digest of the same fixture, 48 owned-concurrent digests now
  agree between one and four workers, 40 policy digests hash the execution
  report, and 4 plan digests hash the operator count. In the new fixture the
  remaining one- vs four-worker digest differences are 56 policy digests
  (execution/concurrency metadata) and 16 plan digests (operator count).
  Root independently reconstructed both this count and the 88/48/40/4 mapping.
- CMG mutations (all exit 101): restoring the reduction gate (d1), the
  centering gate (d2), team-free concurrent lanes (d3) or a team-free serial
  mode (d4). d3 first survived because the witness covered only
  `solve_batch`. The caller-buffer witness added for fereg's path kills it;
  the earlier runs are retained as superseded.
- fereg: ratchet (no ratcheted metric changes), fmt and Clippy pass. The
  core lib passes 1,388 executions (1,374 top-level tests plus 14 isolated
  child re-executions, counted by summary tuple), 3 ignored. The new
  above-threshold projection test (about 142,000 vertices) passes in 127 s
  in debug. On the accepted M3c pair (CMG `82ae1c86` with fereg's team-free
  serial mapping) it fails: one- and four-worker single-column projections
  differ. Mutation e1 (new CMG, fereg's serial mapping team-free) fails it
  too. An earlier run of the accepted-pair check failed only because CMG
  `82ae1c86` was missing from the offline cache; it is retained as invalid.
- Actual-plugin replay of the candidate (`aa65874e...1ee0`) against a cloud
  build of the accepted M3c source (`27fa40cd`, plugin `88a73c1f...a72f`):
  73 of 73 cases exact with no science error. 46 derived explicit OLS and
  PPML runs (both bias modes, 1/4 workers) and 75 injected two-FE runs are
  row-identical to the M3c runs. All replay inputs are below the 131,072
  threshold, so exactness is expected there.
- Within the candidate, explicit outputs at one and four workers are
  identical for 5 of 10 derived pairs. Two pairs differ only in runtime and
  memory signature fields. ols-26 `cmgschur` and both PPML bias runs differ
  in data columns. Those three differ identically in the accepted M3c
  baseline (510, 620 and 628 fields). They come from worker-dependent fereg
  arithmetic outside CMG, which SPEC §20 permits and which M3d did not
  declare or change.
- Standard `./scripts/run_tests.sh`: 315 Python tests pass, then the
  installed-plugin binding check fails (exit 1). `--source-only` (labelled
  source-only; installed-package admission not performed) passes 322 Python
  tests, 1,965 Rust test executions in 83 summary groups (14 ignored), the
  allocator audits, the fuzz check, the portable checks and the ratchet. Earlier
  cloud handoffs counted line-anchored summaries and could undercount
  interleaved child summaries, as root corrected for M3c.

Cloud handoff limits (before root review and the host round below):

- No timing, panel or profile ran. One-worker and serial-mode explicit
  solves above the threshold now run the fixed-chunk tree on one lane
  instead of a single running sum; the cost is unmeasured.
- Replay coverage of the changed arithmetic is nil (inputs below the
  threshold). The change is covered by CMG fixtures run with a 512-entry
  threshold and by one fereg projection above 131,072. Explicit `cmgschur`'s
  one-worker coarse-centering change is covered only by the CMG application
  witness.
- Worker-dependent fereg arithmetic outside CMG remains; see above.
- Row-operator selection stays worker-dependent (bit-neutral). Auto RHS
  batching was not taken.
- Root owns the independent review, native and 42-cell checks and the fixed
  Linux/Mac panels on the changed artifact.

Identities and per-step exits are in
[`cmg-m3d-20261020.json`](../bench/results/cmg-m3d-20261020.json).

Root source review (2026-10-20) found no concrete production defect. The two
independent reviewers checked reduction order, one-lane contexts, resources,
inner cancellation, joins, frozen changes and the fereg boundary. Root narrowed
the overbroad whole-estimate claim above, corrected the frozen metadata count,
and strengthened the above-floor projection assertion to compare floating-point
bits, including signed zero. The focused witness passes. The source-only suite
passes 322 Python tests and 1,951 Rust executions in 83 groups, with 14 ignored;
CMG release/all-features passes 257 executions in 39 groups. The standard entrypoint
still fails at installed-package binding after 315 Python tests (exit 1).
Production comparison to the handoff confirms 192 runtime modules and 58 artifact
inputs unchanged. Exact commands, durations, hashes, review findings and limits
are in the M3d result record. This source checkpoint preceded the native and
host round below; prior milestone exceptions do not accept M3d.

A supplemental test-only extension reuses the 72,000-worker/70,000-firm,
360,000-row fixture for two prepared weight frames at W1/W4. Four projections
and two independent CG projections pass: actual graphs exceed 131,072 vertices,
the certified control seed is consumed in the second frame, all four full-FE
certificates pass, outputs agree bitwise across workers, and the existing
5e-8 oracle allowance holds. Both reviewers cleared the exact addition. The
control comes first and the cold response last; with two FEs there is no
nuisance block. This is prepared/reweighted control-seed coverage, not an
above-floor end-to-end PPML or bias claim. Production and artifact inputs are
unchanged; the fixed native candidate remains c283b3ff. The exact tested overlay
hash and execution are recorded separately from that native-source identity.

Root M3d native and fixed host round (2026-10-20). The measured source is
fereg `c283b3ff3839bdf955cc749b54e9674c74e9001c` with CMG `35a095f3`;
`7a769eea` adds the prepared control-seed test and its evidence without changing
192 runtime modules or 58 artifact inputs. Rust 1.85.1 locked release builds
have actual Mac SHA-256 `21c9fb801eed54de5952b31b4c0ec52c5e568e472654a54d35c19997496c716e`
and Linux SHA-256 `d62e9ee8858ba901465938f2336b198c636dc558196d0727533dcd1e27b5d515`.
Both are compared with the actual qualified M3c binaries, not the cloud builds.
Binary audits and actual ABI/load/panic checks pass on each platform; the Linux
portable frozen-adapter witness also passes.

All six mapped Stata fixtures, 73 exact Mac replays, and 16 explicit CMG PPML
calls (both bias modes, W1/W4, both arms, two repeats) pass. Native builder Break
checks cover W2/W4 twice each at the first actual task/ordinal plus recovery;
the five PPML Break markers and rollback/recovery pass. Separate actual-work
unit tests cover deep inner bounds. Linux's 42-cell, 168-call audit is exact,
with maximum output difference zero and the same six iteration-16 CMG
installation cells in both arms. Build `8218899`, preparation `8219509` and
all three tasks of `8219510` have complete failed=0/exit=0 accounting and
application success markers.

The fixed complete-call `NATIVE_SECONDS` panel passes 12/12 Mac and 12/12 Linux
nonregression bounds against max(5%, 1 ms). Mac used 312 calls on the active
desktop; Linux used 720 calls in six isolated 28-core allocations (jobs
`8219571`–`8219576`, report `8219577`). All 1,032 calls preserve exact science
and solver signatures, including fixed-worker repeats and the applicable
cross-worker checks. No observations were discarded and no timing retries or
additional sampling occurred. Independent reviewers reconstructed timers,
schedules, intervals, science evidence and host observations; exact review
coverage and limits are recorded in the JSON. All seven Linux panel/report
accounting records have failed=0/exit=0 and their application markers.

This establishes the fixed Auto nonregression comparison. It is not an
above-threshold explicit CMG/PPML performance claim: these complete instrumented
solves include tracing and do not measure isolated kernels or an uninstrumented
release. Six Linux host units do not prove independence; intervals are per cell,
not simultaneous across twelve. Changed arithmetic is covered by the above-floor
OLS and prepared control-seed witnesses and lower-level CMG tests. Above-floor
end-to-end PPML/bias and explicit CMGSchur remain outside native coverage.
The standard installed-package binding failure remains; release packages and
public two-FE activation are unchanged.

The root record preserves the earlier failed cloud attempts, helper-reference
substitution repair, an audit-log reader corrected to the observed
`NATIVE_SECONDS=` format, and a login-shell Python 3.6 submission failure before
any job was created. Loading the declared Python 3.12.4 module resolved the last
failure; no native calls or timing observations were repeated. A pre-freeze host
substitution avoided an already occupied node and did not use timing outcomes.
Commands, real exits, identities, compact observations and reviews are retained
in `root_review` and `root_host_validation`; temporary raw task captures were
removed at closure.

On 2026-10-21, after discussion of the representation alternatives and their
memory/construction tradeoffs, Johannes instructed: “Ok, accept the current
scope”. CMG arithmetic is independent of worker count and solve mode for
identical supplied inputs, hierarchy and fixed options/thresholds on one
platform. Row-operator storage remains worker-dependent and bit-neutral;
optional Auto RHS batching was not taken. Existing resource and memory policy
is preserved. The reviewed evidence is committed at `1a3515e0`; the measured
source remains `c283b3ff`, with the test-only addition `7a769eea` and CMG
`35a095f3`. Acceptance changes no runtime or artifact inputs and needs no new
runtime execution.

M3a through M3d and their required independent reviews and repairs are complete.
The implementation-and-review program is complete and stopped, with no active
workflow monitor or further sampling. The installed-package
failure, coverage limits and historical timing uncertainties above remain
recorded. This acceptance does not authorize a representation redesign, main
merge, release-package update or public two-FE activation. The compact
[M3d result record](../bench/results/cmg-m3d-20261020.json) preserves the
pre-acceptance decision alongside Johannes's acceptance (`1871abb7`).

Final cleanup removed completed local task outputs (about 3.5 GB) and the six
completed SCC capture directories for Auto qualification and CMG M2/M3a–M3d,
after their compact evidence and acceptance were committed. No build or
estimation tasks or SCC jobs were active. The cleanup dry run now finds zero
removable entries; Cargo caches, the current replay baseline, shared cluster
tools and unrelated worktrees/automations remain. Raw captures are no longer
retained evidence; committed identities, observations, failures and summaries
remain in the result records and Git history.

## CMG successor milestone 3c: parallel construction on the host team

**The first handoff (fereg `ffc1b014`, CMG `8fafe5a7`) was unaccepted by
independent review on 2026-10-20. Claude's repair and the additional root
corrections below have been reviewed. Root source-only/native checks pass, and the
fixed Linux panel passes all 12 cells. Three Mac cells remain inconclusive.
Johannes explicitly accepted M3c with those uncertainties on 2026-10-20 and
authorized progression to M3d. The timing verdicts and historical failures
remain unchanged.**
M3c starts from the accepted M3b pair (fereg `b9cc6b75`, i.e. source `671239d4` plus
records, with CMG `ccd7177f`). It pins CMG
`8fafe5a7701e4e0d3d35772dd69a5f2b3d39b3d9`: `65bd817a` adds team forest
selection, and `8fafe5a7` makes its hub-row cancellation witness reach the
lanes (test-only). Root owns the source review, the native, scientific,
transaction and 42-cell checks and the Linux/Mac panels. No host panel or
profile ran in the cloud, and no timing claim is made.

Declared before the edits. Affected paths:

- CMG heavy-forest selection on the build's team, which reaches every team
  build: explicit `cmgschur`, explicit OLS and PPML `cmg`, and now the
  automatic upgrade.
- fereg's bounded Schur adapter: the cached census, sampled-row suffix
  preparation, emission offsets, and the automatic hierarchy on the team.

Unchanged: arithmetic and emission/reduction order, hierarchies, factors,
repeats, representations and solver signatures. Iteration 16, cutoff 5, one
sampled tree, seed 0, Auto eligibility, the small-direct criterion, native
limits, certification, restart, fallback, Auto `Never`/width 1 and explicit
`Eligible`/width 4 are also unchanged. Independent witnesses: serial-vs-team
forest parents, weights and hierarchies; serial-vs-team census, emission and
graph; earliest-failure equality; refusal-to-serial bit identity; Auto
team-vs-serial hierarchy and application bits; lane-level cancellation; the
42 frozen W1/W4 adapter digests; and actual-plugin replay against the accepted
M3b plugin.

Implementation:

- CMG `heavy_forest` uses a row-owned team path with the build's team on
  levels with at least 65,536 directed entries and more than 1.5 edges per
  vertex. A counted serial pass gathers incoming edges into a transient index
  (row starts plus one `u32` or `usize` per edge). The path runs only when the
  index is no larger than the contraction slot bank that the parallel
  construction bound already adds to the serial forest scratch, so admission
  and peak are unchanged. Lanes own fixed 4,096-vertex blocks through a
  paired dispatcher on the existing claim loop and count every scanned entry,
  inside a hub row too. Selection is a maximum under a total order (weight,
  then lowest neighbor), so parents and weights equal the serial scan at any
  ceiling.
- fereg census with the canonical cache (the automatic route) and at least
  65,536 observations: consecutive workers form tasks of at least 4,096 raw
  cells (a longer row is its own task). Each task fills, sorts (counted
  heapsort), deduplicates (Neumaier) and totals its rows in its own range of
  the admitted cache bank. Lane counters use the serial units and the
  65,536-unit cadence and poll inside a single long row. After the join, rows
  are compacted in worker order into the serial census's exact layout, and
  the earliest failing worker's error is returned, as the serial census
  reports it. The decision depends on item counts, never on the worker count
  (a one-worker team runs the same tasks).
- Sampled-row preparation (sort by weight, suffix masses) runs in the same
  kind of task on the team. Edge emission locates each chunk's first worker
  by binary search over prefix offsets instead of the serial placeholder
  pre-pass.
- Only the census and preparation task tables and the offsets are new
  scratch. They are admitted; a refusal selects the serial stage before any
  numerical write.
- The automatic upgrade builds its hierarchy on the team whenever the
  parallel construction bound fits after the live parents (checked without
  recording). Otherwise it admits and builds under the serial bound, as
  before. Both build identical hierarchies.
- Explicit `cmgschur` keeps its uncached two-pass census, so its census and
  emission stay serial; its hierarchy already used the team.
- The team stages live in `fe/cmg_schur/bounded/team.rs`. The heapsort and
  row loader are shared with the serial census, and `Work::sort` is removed.

Witness repairs and a defect found before commit:

- The first hub-row forest witness stopped at the first host polls. These
  belong to the serial incoming-index pass, so the witness never reached a
  lane, and a mutation that scans team rows unmetered still passed. Its query
  now turns true after the lanes have scanned 1, 20,000 or 100,000 entries.
  Lane 0 starts in the 199,999-entry hub row, and every lane must stop within
  one poll interval plus one bounded segment and the row-block slack (CMG
  `8fafe5a7`, test-only).
- The first earliest-failure witness used nonfinite and nonpositive weights.
  The adapter rejects these before the census, so no lane ever failed. The
  replacement uses the two failures the census itself detects: an
  out-of-range firm, and a positive weight that underflows after scaling. It
  places them, with different messages, in different census tasks in both
  orders. This exposed a real defect in the uncommitted team census: on a
  lane failure, compaction stopped before writing the failing worker's row
  start. The census then read the preceding row with a stale end and
  panicked instead of returning the error. The failing worker's start now
  closes the preceding row. The unrepaired code fails the new witness with
  the slice panic (mutation f5).
- The explicit `cmgschur` build/application poll witness requires the single
  `plan publication` checkpoint. Its 400-stop grid (step 2) no longer landed
  on it after the counted serial row loader shifted the poll schedule. The
  witness now also visits every stop between the first sampled plan stop and
  the first sampled workspace stop. The requirement is unchanged.

Validation reported by Claude (cloud; compact outcomes and real exit codes
in the linked JSON; raw logs are not retained evidence):

- CMG `65bd817a`: all 27 matrix steps exit 0. These are fmt; tests (default
  131, `parallel` 197, all features 254, each in debug and release); frozen
  bits in eight feature/profile configurations; Clippy `-D warnings` for
  seven feature sets; rustdoc; benchmark crate Clippy and fmt. The crate has
  no doctests. After the test-only `8fafe5a7`: fmt, Clippy (none, `parallel`,
  all features) and tests (`parallel` 197 debug and release, all features
  254) exit 0.
- Mutations on scratch copies (expected nonzero; all exit 101): f1 team
  forest rows unmetered; f2 fereg lane counters never poll; f3 team census
  reports the last failing task; f4 automatic build ignores the parallel
  bound fit; f5 failing worker's row start not written. Each fails its named
  witness. Earlier runs of f1 (weak witness), f3 (weak and unreached
  witnesses) and f2–f4 (on copies without the new module file, or on the
  pre-fix source) are retained separately as invalid, not as evidence.
- fereg on the pinned tree: complexity ratchet (declared growth
  104,701 → 105,148 production lines and 1,354 → 1,365 dispatch candidates;
  no file over 1,500 lines and no function over 150 added), fmt, Clippy
  `-D warnings` and 1,371 fereg-core lib tests (3 ignored) pass. These
  include the 42 frozen W1/W4 adapter digests and the team-stage, route
  cancellation and fatal-classification witnesses.
- Actual-plugin replay of the candidate (`f9214488...cb3b`) against the
  accepted M3b cloud plugin (`2c751c49...2cd7`): 73 of 73 cases exact with
  no science error. 46 derived explicit runs (`cmgschur` at 1/4 workers,
  sampled and degree-4 variants; `cmg` at 4 workers; PPML `cmg` with exact
  and randomized bias at 1/4 workers): no exact, discrete or repeat
  differences, identical setup traces. The 5 unsupported `cmgschur` inputs
  return 198 on both plugins. 75 injected two-FE runs (`routing_incremental_two`,
  `routing_h4_k1`, `routing_direct`): no exact differences and equal routes.
  The known-defect fixture notes are identical. Every per-run row equals the
  accepted M3b run's row.
- Standard `./scripts/run_tests.sh`: 315 Python tests pass, then the
  installed-plugin binding check fails (exit 1), as on every M3b handoff.
  `--source-only` (labelled source-only; installed-package admission not
  performed) passes 322 Python tests, 1,961 Rust test executions in 86
  groups (14 ignored), the allocator audits, the fuzz check, the portable
  checks and the ratchet.

Limits and open items:

- No timing, panel or profile ran; M3c makes no performance claim. Gains are
  bounded by the remaining serial passes: the forest's incoming-index count
  and scatter (O(m)); the census's task planning and worker-order compaction
  copy (O(N)), which a one-worker team also pays because the decision is
  worker-independent; and serial union-find and canonical graph merging.
- The replay fixtures are below the 65,536-item team thresholds. The team
  census, preparation and forest paths are therefore witnessed by unit tests
  on synthetic fixtures (about 100,000 observations, a 60,000-cell worker, a
  199,999-entry hub row), not by the replayed transactions.
- Explicit `cmgschur` keeps its uncached census, so its census and emission
  stay serial. Giving it the team census needs the canonical cache and would
  raise its admitted peak by the cache bank. That is a resource change left
  for a decision, not made here.
- A long row is sorted by one lane (counted and polled), not split across
  lanes by merge passes, so no extra ping-pong bank is admitted.
- Root owns the independent source review, native Break and fixture checks,
  the 42-cell scientific and transaction checks and the Linux/Mac panels on
  the changed artifact.

Identities, per-step exits and limits are in
[`cmg-m3c-20261020.json`](../bench/results/cmg-m3c-20261020.json).

### Independent review: unaccepted; review repair (pending re-review)

Root fast-forwarded to the handoff and the two independent reviewers
finished their source review. Root checked two findings against source as
source proofs and test recipes, not runtime reproductions. No other concrete
defect was found in CMG paired dispatch, maximum-edge and tie selection,
incoming-index sizing, hub polling or joins. That is a bounded review, not a
proof that unexecuted root gates pass.

1. Unbounded work without cancellation in hosted edge emission. The edge
   kernel advanced the worker with
   `while offsets[worker + 1].0 <= index { worker += 1; }` inside one
   `ControlledStage` chunk. Workers of canonical degree one (duplicate
   observations at one firm, which singleton removal need not drop) emit no
   edges. A run of them between two edges of one chunk therefore cost an
   arbitrarily long scan before the next cancellation query. The declared
   per-edge work also counted binary-search steps from `max_rows`, not the
   worker count.
2. Transient parallel construction scratch kept in the persistent automatic
   charge. With a team, the automatic route admitted the parallel bound
   (serial bound S plus the contraction slot bank D) and then added all of
   it to the live parents before admitting completion storage W. It also
   published that larger retained bound. A budget of P + S + max(D, W) admits
   accepted serial completion and the parallel construction, but refused the
   team completion. Unlimited team builds overcharged retained storage by D.
   The earlier finish-level test only covered P + S + D/2.

Declared before the edits (fereg only; CMG production unchanged, so the CMG
`65bd817a`/`8fafe5a7` evidence above is reused):

- Paths. R1: the emission plan's worker lookup and its declared per-edge
  work. R2: the automatic route's persistent charge in `finish`. Explicit
  accounting, edge order, draws and arithmetic are unchanged.
- Witnesses. R1 is an isolated child-process test on 70,000 emitting
  workers around 100,000 duplicate-only workers, which emit at least
  65,536 edges. It checks probes per edge, that the declared chunk covers its
  actual worst edge, work after the cancelling poll, untouched edges, joins,
  an exact rebuild, and identical graphs with no team, one lane and four
  lanes. R2 is a finish-level test that derives W from the accepted serial
  result and covers P + S + max(D, W), both sides of P + S + D, and true
  refusals below P + S + W and P + S, with no team, W1 and W4.

Pre-fix failures on the submitted code (plus test instrumentation only):

- R1: one edge probed 100,002 worker offsets (bound 19).
- R2: an unlimited W1 team build retained 43,068,776 bytes against the
  accepted 42,675,896 (D = 392,880). With the P + S + max(D, W) case first,
  the budget of 43,043,376 bytes was refused (`MemoryBudget`) for W1.
- The first R1 run failed on a wrong stage expectation in the new test
  (sampling also prepares rows). It is retained, and the fixture now runs
  without sampling.

Repair:

- Each edge checks the next worker offset. Otherwise it bisects the
  remaining offsets, so it takes at most one probe plus the bit length of
  the worker count. The declared per-edge work adds 16 units per such step,
  so every hosted chunk of the gap fixture holds one edge. An earlier
  version used `partition_point`, which made one more comparison than the
  declared steps (20 against 19 under mutation m1c). It was replaced by an
  explicit bisection whose step count is fixed.
- The automatic route adds the serial bound S of the identical hierarchy to
  its persistent charge. The admitted construction peak keeps the parallel
  bound. Explicit builds keep their bound.
- Hosted emission moves into `fe/cmg_schur/bounded/team.rs` with the other
  team stages, keeping `bounded.rs` under the 1,500-line limit.

Mutations on the final source (expected nonzero unless declared):

- m1, the submitted defect (linear scan and accounting without lookup
  steps): killed, 100,002 probes.
- m1c, accounting without lookup steps but bisection kept: killed, 3 edges
  per chunk need 1,728 units.
- m1b, the linear scan alone: survives as a declared equivalent mutant.
  Under the corrected accounting every hosted chunk of the fixture holds one
  edge, whose worker the per-chunk search already finds, so a scan never
  crosses workers. The bisection keeps the bound independent of chunk length.
- m2, the parallel bound retained: killed (43,068,776 against 42,675,896).
- f2–f5 from the first handoff are killed again on the final source. f4 is
  now also caught by the R2 witness.
- Superseded mutation runs (on the `partition_point` version, and before the
  module move) and the first run of the scan-only mutation are retained
  separately.

Count clarification: the first handoff's JSON field `63-fereg-core-lib`
reported 1,384 passed. That is 1,371 top-level tests plus 13 isolated
child-process re-executions; each child prints its own one-test result line.
The record now reports the two separately.

Validation of the repair (cloud; compact outcomes, commands and real exits
in the linked JSON; raw logs are not retained evidence):

- fereg: complexity ratchet (declared growth 105,148 → 105,165 production
  lines), fmt and Clippy `-D warnings` pass. fereg-core lib: 1,373
  top-level tests pass (3 ignored), plus 14 isolated child-process
  re-executions. These include both new witnesses, the 42 frozen W1/W4
  builder, cycle, solve and adapter digests, and the allocation, publication,
  error, route-cancellation and fatal-classification tests.
- Actual-plugin replay of the repaired candidate (`77838f0d...4f7f`) against
  the accepted M3b cloud plugin (`2c751c49...2cd7`): 73 of 73 cases exact
  with no science error. 46 derived explicit OLS and PPML runs (both bias
  modes, 1/4 workers) and 75 injected two-FE runs: every row identical to
  the accepted M3b and first M3c runs; the known-fixture notes are identical.
- Standard `./scripts/run_tests.sh`: 315 Python tests pass, then the
  installed-plugin binding check fails (exit 1). `--source-only` (labelled
  source-only; installed-package admission not performed) passes 322 Python
  tests, 1,964 Rust test executions in 87 groups (14 ignored), the allocator
  audits, the fuzz check, the portable checks and the ratchet.
- CMG production is unchanged at `65bd817a` (pin `8fafe5a7`); its matrix
  above is reused.

Remaining limits are those of the first handoff, with explicit `cmgschur`'s
uncached census and one-worker team behavior retained as instructed. Replay
fixtures remain below the team thresholds. No timing, panel or profile ran.

### Root review and additional corrections

Root reviewed fereg `0c3ea098` with CMG `8fafe5a7`. The two independent
reviewers cleared the R2 production correction. They found that R1 still
located the first edge of each chunk with an uncounted `partition_point`.
Since corrected accounting made the long-gap fixture's chunks one edge long,
the test never exercised the new bisection; its scan-only mutation survived.
This was an incomplete accounting and witness issue, not an executed
demonstration that the repaired production code exceeded its cancellation bound.

The root correction starts each chunk at worker zero and sends its first and
later edges through the same counted, bounded lookup. Edge order, draws and
arithmetic remain exact; there is no allocation, resource or pool change.
Explicit offset witnesses cover leading, internal and trailing zero-edge
workers and both fresh and continuing lookups. A direct 100,000-worker gap
requires the exact owner and at most 18 probes. The hosted fixture must
actually execute bisection and retains its actual-work cancellation, untouched
work, joins, exact rebuild and no-team/W1/W4 checks.

The finite root mutation set kills the restored linear scan, removed lookup
accounting (a three-edge chunk needs 1,776 units against 1,024), and restored
parallel persistent charge (43,068,776 against 42,675,896 bytes). Each is an
executed witness failure, not a compilation failure. The independent
high-effort review found no further actionable production defect. The root
change adds no production complexity metric growth.

The standalone CMG release check then failed in
`parallel_construction_admission`: after a W4 interrupted build with 612
vertices and 65,586 edges, the test's caller-thread byte count was 3,229,314
against 3,229,378 before it. Both reviewers independently found that the
allocator credited the allocating thread and debited the freeing thread;
cross-thread ownership makes that balance invalid. Forest dispatch can
precede the injected mapping-bank allocation. The negative 64-byte result
does not establish a leak or a join defect, and the particular allocation
was not identified.

CMG `82ae1c86d1e7d8c04f35d723b5affae8a031e283` changes only that test and
its documentation. A fixed, allocation-free pointer ledger tracks the
initiating caller's allocation cohort through frees and reallocations on
any thread. Cleanup requires exactly zero outstanding allocations and no
ledger overflow. Cross-thread, foreign-origin, retained-allocation and
failed/successful reallocation controls verify the accounting. The successful
resize may happen in place; pointer movement is source-reviewed rather than
guaranteed by that control. Process-wide peak observations stay separate.
The manifest and both lockfiles pin this test-only commit; CMG production
still equals `65bd817a`.

After that diagnosed repair, CMG release/all-features passes 254 executions
in 38 result groups; the focused debug test and all-target/all-feature Clippy
with warnings denied pass. Root fereg source integration before the test-only
pin change passes 322 Python tests and 1,950 Rust executions in 83 groups
(14 ignored), including the frozen and deep cancellation witnesses, allocator,
portable, fuzz and numeric checks. These are the Mac run's counts; the cloud
counts above describe different runs. The standard entrypoint still fails
installed-package binding after 315 Python tests. The original standalone
CMG failure, an initial sandbox cache-fetch denial and a missing `cargo-fmt`
PATH invocation remain recorded with the successful corrections.

Final integration with all three pins at `82ae1c8` also passes the source-only
entrypoint (336.301217 seconds, the same 322 Python / 1,950 Rust / 83 group
counts). The standard entrypoint retains its installed-binding failure
(exit 1, 19.814272 seconds). The first summary parser missed two child
result prefixes interrupted by parallel parent output and reported 1,948/81.
The unchanged log contains all 1,950 passed results in 83 numeric summary
tuples; the JSON corrects that counting error and retains the failed assertion.
No runtime check was rerun for this record correction.

The JSON `root_review` records commands, actual exits, durations, log hashes,
source hashes, mutation outcomes and final pinned integration checks. No root
native or performance qualification is claimed by these source checks.

### Root native and fixed host qualification

The tested candidate is fereg
`71092c3823f0488ecbd906761a52f142cffbf612`, with production correction
`c89ab124e99b5ece97d93747bd03ef1c8c4cf6bb`, and CMG
`82ae1c86d1e7d8c04f35d723b5affae8a031e283`. The intervening record correction
is production-identical: 192 runtime modules and 58 artifact inputs match.
Both root and fuzz locks and the manifest pin that CMG revision.
The comparison uses the accepted M3b source `671239d4` / CMG `ccd7177f`,
not an earlier rejected or cloud-only artifact.

Rust 1.85.1 locked release builds passed actual Mac and Linux binary audits,
ABI/load and panic-containment checks. Actual candidate plugin SHA-256:

- Mac: `c0a8d9c7355494e4ea40b5ac95a979d883d38f2544cecc2966892a180302c5dd`
- Linux: `4d3a78dea4239f59e3491abac4a90448d9181ff8b9ed7152380e02a2be1fef11`

The source archive hash is
`527c5ddf637f2f5315f2e902ff6aa81533f9083cd336f191dbf9b6f37db85580`.
The Linux portable frozen witness also passes. Mac checks pass all six mapped
Stata fixtures, 73 exact replay cases, and 16 explicit CMG PPML calls across
both bias modes, W1/W4, both arms and two repeats. Native builder Break at W2/W4
passes twice each at actual task 0/ordinal 0 with recovery; this does not claim
arbitrary deep native-phase latency. Separate actual-work unit witnesses cover
deep cancellation, including the long emission gap. Public PPML Break passes
all five markers and transaction recovery.

The Linux 42-cell audit passes all 168 calls exactly, with maximum output
difference zero and the same six h16 installations in both arms. Build job
8210060, preparation 8210607 and array 8210608 tasks 1–3 have complete
`failed=0` / `exit_status=0` accounting. The analytic construction/install/
certification and bounded Schur witnesses passed in the source suite.

Before measurement, the actual binaries, source, harnesses, inputs, references,
schedules and budgets were frozen. The fixed round retained all 1,032 calls:
312 on one active Mac and 720 across six isolated 28-core Linux hosts.
All science and repeats pass, with exact paired outputs and workflow.
There were no exclusions, replacement cells or additional samples.
Linux timing jobs 8210650–8210655 and report job 8210656 have complete
`failed=0` / `exit_status=0` accounting and verified completion markers.
Linux passes all 12 nonregression bounds. Mac passes nine; three are
inconclusive against the unchanged max(5%, 1 ms) allowance:

| Mac cell | Upper excess beyond allowance |
| --- | ---: |
| repeated-cells W4 | +0.036463 ms |
| repeated-cells W8 | +1.150387 ms |
| wide-rhs W4 | +0.284941 ms |

Independent reviews reconstructed the complete-call `NATIVE_SECONDS` results;
the plugin's `native_s` component remains a diagnostic. The Linux reviewer
verified all raw timers, perf CSVs, signatures and digests locally; full
scientific heads/arrays were rechecked by the remote report rather than
downloaded and revalidated by that reviewer. The three inconclusive
Mac cells finish before iteration 16 and do not install CMG. Their retained
phase observations do not identify a construction-path defect, and no recorded
external interference supports discarding them. Inconclusive bounds remain
inconclusive. The Linux high-degree W4/W8 median complete-call ratios are
0.9343/0.8959, and the Mac ratios are 0.9441/0.9090. These are instrumented
complete-solve observations, not isolated-kernel or uninstrumented release claims.
Intervals are per cell, not simultaneous across 12 cells; six hosts do not
establish independence.

The JSON `root_host_validation` retains compact observations, actual identities,
job accounting, independent reviews and limits. The existing standard
installed-package binding failure and earlier rejected handoffs, mutations,
test-accounting failure and operational failures remain recorded.
At the review boundary, M3c remained unaccepted and Claude stopped with no
monitor active. M3b's earlier exception did not waive the M3c prerequisite.

### Explicit acceptance with retained timing limits

After the three Mac bounds above and the known installed-package binding
failure were disclosed, Johannes instructed: “Yes, I accept. Continue with M3d”.
M3c is accepted by that specific decision. All three bounds remain
inconclusive; no observations are excluded and the fixed measurement budget
is not reopened. The standard entrypoint remains failed at installed-package
binding. This decision does not waive future prerequisites.

The accepted source/artifact pair is fereg `71092c38` (production correction
`c89ab124`) and CMG `82ae1c86`, with the actual Mac/Linux plugin and archive
hashes above. Root evidence is committed at
`a28d48b806867fbceb6766a13e2d14ce348fa08d`. This is the accepted comparison
for M3d; M3b, M3a and their historical failures remain recorded.

M3d is authorized within final plan revision 3: worker-independent
representation/reductions and optional new RHS batching, with affected
arithmetic and storage declared before edits and independent mathematical
witnesses. It must incorporate the root fixes and preserve the consolidated
pipeline, host-team ownership, fixed scientific parameters, resource controls,
actual inner-work cancellation and atomic transactions. No construction-work
cap, ledger, predictive gate or factor preflight returns. Claude must report
and stop for independent review and applicable native/host validation; public
two-FE activation remains unauthorized.


## CMG successor milestone 3b: one construction pipeline, cycle and adapter

**Johannes accepted M3b on 2026-10-20 and authorized M3c, explicitly retaining
three inconclusive timing cells.** The accepted source is fereg `671239d4`
with CMG `ccd7177f`; root review is recorded at `49a3aefa`. Source, native,
scientific and transaction checks pass. The final fixed panels pass 21 of
24 platform cells, with Mac weak-low W8, Mac wide-rhs W8 and Linux
repeated-cells W4 inconclusive. This is milestone acceptance by explicit
exception, not a full performance pass. The standard installed-package
binding failure and all earlier rejected handoffs and failed panels remain
recorded below. M3d remains gated on M3c review. M3b is a behaviour-preserving
consolidation. The first handoff pinned CMG
`c6f9e8224503273a20b47cf42ec8bc09d50fefd9`
(`69862653ed5d4815c05fe85a91c8b625626f3e3c` freezes pre-consolidation digests,
then `c6f9e822` consolidates; the first records abbreviated the witness as
`6986265e`). The first repair pinned `b9f2585a00ceefd9f27abb98be8828a856854864`; the second
repair pins `be1b89d3137467b076dc61e2622dfb7dcef520a2`. CMG now has one
construction pipeline (`build_with` with `BuildOptions` and a `BuildContext` of
borrowed `Team`, optional cancellation and optional layout budget), one
stationary cycle recursion over the two existing operator kernels
(`ApplyContext { plan, team, cancel }`) and one PCG recurrence (`SolveContext`).
The serial/budgeted/executor/controlled/component builder cross-product, the
17 `solve_pcg_*_with_*` variants, separate profiled copies, `HostExecutor`,
`InstalledTeam` and `with_installed` are removed with their tests, examples and
benchmarks migrated. No CMG construction, application or solve creates a pool.

fereg has one CMG-Schur adapter: explicit `cmgschur` now runs the automatic
route's bounded census, clique/tree emission, initial graph and hierarchy
pipeline, differing only in installation (explicit: host-team contraction, row
operators for the team's eligible levels, up to four interleaved lanes; Auto:
serial hierarchy, team-free scalar cycle, width one). The explicit hybrid
builder, worker census, iterator edge stream, unbounded sampler and executor
application owner are deleted. Explicit OLS/PPML `solver(cmg)` builds, plans and
solves on the action's host team through `parallel::with_cmg_team`, which
requires the team width to equal the requested threads (outside a native action
it installs fereg's process-local team). The first handoff claimed that
construction, application and PCG polled the host flag at a bounded interval
on every route; review showed that claim was overbroad (seven defects listed in
the repair below). CMG's `Cancelled` maps to `Interrupted` after all lanes
join. Auto keeps `Never`/width 1, explicit routes
keep `Eligible` (worker-dependent) and width 4, reduction orders, hierarchies,
factors, repeats and cycle bits are unchanged. Iteration 16, cutoff 5, k = 1,
seed 0, eligibility, the small-direct criterion, restart, certificate, fallback
and public memory controls are unchanged.

Declared non-arithmetic scope: explicit `cmgschur` memory admission now follows
the bounded adapter (no CMG pool runtime allowance or forecast; a memory refusal
remains an allocation error with a new message); explicit `cmg` keeps its
runtime allowance but no longer counts an owned pool's handle bank; owner byte
reports follow the new struct layouts. Memory tables are outside exact replay.

First-handoff evidence (cloud, Linux x86_64; full stdout/stderr and real exit
codes retained under `target/tmp/auto-routing/m3b/logs` until review, not
committed):

- Frozen witnesses. CMG `tests/frozen_bits.rs` reproduces digests frozen from
  the pre-consolidation builders, cycles and solves at one and four workers in
  eight feature/profile configurations. A fereg witness records 42 digests
  (graph edges, firm labels, hierarchy trace fields, application policy and
  every output bit of scalar, masked, scaled and interleaved applications) of
  the old explicit executor builder and the bounded Auto builder on the
  post-M3a tree (`e9efda55` plus the test only), at 1 and 4 workers and widths
  1, 2 and 5 over sampled, star and multilevel torus fixtures (row operators at
  four workers); the consolidated adapter reproduces all 42.
- Cancellation. CMG unit tests cancel a scalar cycle at every bounded poll,
  cancel serial PCG mid-recurrence with an exact restart, and cancel planned
  and interleaved cycles and planned and concurrent PCG with all lanes joined;
  a new test cancels batch-workspace preparation at every poll. A fereg witness raises
  host cancellation inside a planned interleaved cycle and a planned PCG solve
  on a four-worker host team at early, middle and late polls, observes
  `Interrupted`, and then reproduces reference bits from the same installed
  hierarchy, plan and workspaces.
- CMG checks: tests default 120, `parallel` 171 and all features 227 (debug
  and release), doc tests, Clippy `-D warnings` for no features, `parallel`,
  `experimental-components`, `profiling`, `cycle-profiling`,
  `parallel,experimental-components` and all features, fmt, rustdoc
  `-D warnings`, and the benchmark crate (Clippy default/all features, fmt) in
  a standalone copy.
- Actual-plugin replay of the candidate (`5f9fd1ed...87d4`) against the
  post-M3a plugin (`971d3e30...fb97`): 73 of 73 cases exact with no science
  error, including ten explicit `solver(cmg)` OLS/PPML cases. Derived explicit
  variants (46 runs: `cmgschur` at 1 and 4 workers, sampled and degree-4;
  `cmg` at 4 workers; `cmg` with both PPML bias modes at 1 and 4 workers) are
  exact with equal repeats and identical `cmgschur_setup` traces (graph,
  hierarchy, threads, row operators, width); five unsupported `cmgschur`
  inputs return the same code 198 on both. Injected two-FE runs
  (`routing_incremental_two`, `routing_h4_k1`, `routing_direct`; 75 runs) are
  exact with equal routes and repeats; none reaches a CMG build, so the source
  witnesses remain the installation evidence. Their 18 science notes are the
  three known-defect fixtures, identical on both plugins (that ad-hoc runner
  does not apply the replay's baseline-fix labels).
- Complexity: fereg-core 104,943 -> 104,553 production lines and 1,371 ->
  1,354 dispatch candidates; no growth.
- `./scripts/run_tests.sh` passes 315 Python tests and fails at the expected
  installed-plugin binding check (exit 1); Codex's local run on `b4b53577`
  matches (315 passed, then the binding check failed with exit 1 in 20.4167 s,
  full output retained locally). `--source-only` passes 322 Python
  tests, 1,947 Rust test executions in 82 groups (14 ignored), the allocator
  audits, the fuzz check, seven portable checks and the ratchet.

Logging limitation: the first CMG frozen-witness run filtered cargo output
through grep/head and kept no per-command exit status; that filtered stream and
a limitation note are retained, the run was not repeated, and every later check
saved complete output and its real exit code.

Outstanding host checks (Codex): six mapped Stata fixtures, builder Break at
two and four workers, the PPML Break fixture, native Mac build and replay, and
the Linux/Mac nonregression diagnostics. No performance claim is made from this
cloud host.

### M3b review repair (rejected on re-review)

Codex's independent review of the first handoff found that bounded
cancellation was not yet true of every kernel and route, and that the worker
ceiling was enforced only by `ControlledStage`:

1. Team construction ran graph-sized work uninterrupted: contraction filled
   its slot bank unmetered, mapped edges on Rayon without the query, compacted
   unpolled, and team sorts polled only before and after Rayon's sort.
2. CSR rows, terminal-factor rows (scalar and interleaved) and configurable
   reduction leaves were counted before an unpolled traversal; parallel lanes
   polled only between groups of rows.
3. Plan construction and PCG workspace banks had no cancellation plumbing, and
   `CmgBatchWorkspace::try_new_controlled` used the unmetered layout helper.
4. Concurrent PCG chose errors and published solutions after the join with no
   poll, so a numerical error could mask a lane's cancellation and a final-copy
   cancellation could return success.
5. `Team::worker_ceiling` was not enforced outside `ControlledStage`: Rayon
   kernels could run more lanes than the ceiling on a larger borrowed pool.
6. fereg passed no query to the explicit route's initial graph, plan and
   workspace banks; caller buffers, Schur auxiliary fills, component clears and
   means were unpolled, and `apply_batch` had no final checkpoint.
7. Profiled builds grew their phase records infallibly and unaccounted.

Repair, CMG `b9f2585a` (bits, hierarchies, factors, repeats, representations,
reduction orders and memory admission unchanged):

- Long traversals advance a shared `Progress` in segments of at most 1024
  units while they run; parallel lanes keep a lane-local count and poll the
  sticky query every 65,536 units, also inside a row or leaf. Accumulators,
  compensation state and leaf boundaries are unchanged.
- Every parallel stage runs through one capped dispatcher (fixed chunks
  claimed by at most `worker_ceiling` lanes), one fixed balanced reduction tree
  whose forks stop when the lane budget is spent, a lane-budget token for
  recursive work, or `ControlledStage`; a source test forbids forking anywhere
  else.
- Team contraction maps in fixed logical chunks and fills and compacts its slot
  bank in counted segments; team canonicalization sorts in place with a counted
  introsort (three-way partitions, heapsort fallback). It allocates nothing,
  as the earlier in-place Rayon sort did. An intermediate design that reused the
  initial-graph merge sort was dropped before commit because its spare edge
  bank is not covered by fereg's explicit-route admission when observations
  repeat worker-firm pairs.
- Context forms with metered preparation: `ParallelCmgPlan::build_with`,
  `ParallelPcgSolver::try_plan_with`, `PcgBatchWorkspace::new_bank_with` and
  `bank_allocation_bytes_with`, `CmgBatchWorkspace::allocation_bytes_controlled`
  (now used by `try_new_controlled`), `PreparedLaplacianTopology::prepare_with`
  and `assemble_with`; each polls before every allocation and once before
  publication.
- Concurrent PCG polls after the join, before error selection, publishes
  through the bounded meter and polls before success.
- Profiled builds reserve their record banks fallibly under the meter and
  admit them with the build bound.

The fereg repair, on the same branch after `b4b53577`, passes the host
query to every explicit CMG stage named above, fills its own buffers in polled
segments with a final checkpoint, names the cancelled stage in bounded-adapter
errors, and propagates a cancelled setup or projection as `Interrupted`: the
route witness found that the explicit route had retained a CMG cancellation as
a setup failure (CG fallback for OLS, `NonConvergence` for unpolished PPML),
which is now fixed. Production lines grow by 71 with scoped ratchet entries.

Repair evidence (cloud; full output and real exit codes under
`target/tmp/auto-routing/m3b-r1/logs`, not committed):

- New CMG witnesses that fail on the reviewed gaps: a 200,000-leaf hub row
  cancelled at segment k leaves exactly k×1024 terms summed (serial), and a
  lane stops inside the hub row at its first poll with 65,536 terms; long
  sparse and packed factor rows (scalar and interleaved) leave exactly the
  earlier terms summed and otherwise equal `Iterator::sum` bit for bit; a
  single 2^20-term reduction leaf polls 16 times inside the leaf, and the tree
  is ceiling-independent; cancellation sweeps over team construction reach
  contraction slot fill, mapping, compaction and the team sort, over plan
  construction reach every CSR stage and publication, and over PCG banks,
  interleaved workspaces, prepared topology and assembly and profiled builds
  reach their fills and final polls, each followed by an exact rebuild;
  concurrent and sequential PCG report cancellation at each of their last
  three polls (join, publication, final) and a cancelled lane is not masked by
  an earlier RHS's non-finite error; on an eight-thread pool with ceiling
  three, initial graphs, contraction, plan, planned and interleaved cycles and
  planned and concurrent PCG never exceed three concurrent lanes (and reach at
  least two).
- fereg witnesses: host cancellation sampled over the explicit OLS and
  prepared PPML routes on four workers is `Interrupted` and reaches the team
  sort, contraction mapping, workspace banks, prepared topology sort and
  duplicate sort and fereg checkpoints, with exact bits afterwards (a
  poll-sampling witness: it shows where cancellation is observed, not how
  much inner work runs between polls); the
  explicit `cmgschur` build and application reach plan CSR stages, plan
  publication and batch workspaces; mid-cycle and mid-PCG cancellation recovers
  exact bits; the 42 frozen adapter digests still match.
- CMG: tests default 126, `parallel` 186, all features 243 (debug and
  release), doc tests, frozen bits in eight configurations, Clippy for seven
  feature sets, fmt, rustdoc and the benchmark crate pass. The first repair
  matrix failed Clippy in every feature set (a needless range loop, a
  recursion-only parameter and a test struct update); those logs are retained,
  the fixes were made and the whole matrix was rerun.
- fereg-core: 1,363 lib tests pass (3 ignored); fmt, Clippy and the complexity
  ratchet pass. Earlier route-witness iterations that failed while their
  sampling and expectations were corrected are retained.
- Actual-plugin replay of the repaired candidate (`f7b40a9e...dfd68`) against
  the post-M3a plugin: 73 of 73 exact; 46 derived explicit runs (including both
  PPML bias modes at one and four workers) exact with identical setup traces;
  75 injected two-FE runs exact with equal routes and no CMG build.
- Final source boundary on the stable tree: `./scripts/run_tests.sh` passes
  315 Python tests and fails at the expected installed-plugin binding check
  (exit 1). `--source-only` (labelled source-only; installed-package admission
  not performed) passes 322 Python tests, 1,951 Rust test executions in 84
  groups (14 ignored), the allocator audits, the fuzz check, the portable
  checks and the ratchet.

Remaining cancellation limits as recorded for this repair (the second repair
below corrects them): allocator, operating-system and join latency;
per-row elementwise work proportional to the caller's fixed tile width (at
most four in fereg); O(levels) layout and metadata loops polled only at their
checkpoints; ordinary context-free CMG entry points and the owned-result
`ParallelPcgSolver` methods remain unmetered by design and are unused by
fereg's routes.

Outstanding host checks (Codex): the native analytic h16 install/certification
witness (the 75 injected cases built no CMG and cannot discharge it), six
mapped Stata fixtures, builder and PPML Break, native Mac build and replay, and
the Linux/Mac nonregression diagnostics. The team sort changed from Rayon's
parallel sort to the counted introsort, so its timing belongs to those
diagnostics; no performance claim is made from this cloud host.

### M3b second review repair (pending re-review)

Codex's source re-review of the first repair (CMG `b9f2585a`, fereg
`5ba16717`) rejected it again on 2026-10-20; M3c is not released. Codex's
root standard check on `5ba16717` passed 315 Python tests and then failed the
installed-plugin binding check (exit 1, 20.780299459 s), as on `b4b53577`
(exit 1, 20.4167 s); the standard entrypoint has not passed on any M3b
handoff. Findings:

1. The fixed reduction tree always evaluated both serial subtrees, a lane
   poll did not retain a stopped state, and PCG leaves returned zero on
   cancellation, so a cancelled reduction kept traversing untouched leaves.
2. Prepared numeric assembly took its final poll before publication, whose
   diagonal metadata scan and shared handles were unmetered; topology
   preparation polled once before four reserves; the duplicate-weight
   workspace had no controlled constructor.
3. `solve_pcg_with` cloned its owned solution infallibly after the last poll.
4. Batch kernels counted a row of `columns` lanes as one segment, so widths
   above 1024 lanes defeated the bound.
5. fereg's prepared route left the weight-ratio loop, the seed finiteness scan,
   certified seed retention and the endpoint reserve unpolled, used the
   budget-only workspace constructor, and `filled_vec` had no final poll.
6. Every explicit-route CMG error other than cancellation (including internal
   runtime and hierarchy-invariant failures) was retained as a setup failure or
   CG fallback.

Second repair, CMG `004d1c4d` (with the test-only witness fix `be1b89d3` below) (bits, hierarchies, factors, repeats,
representations, reduction trees and orders, worker eligibility and memory
admission unchanged):

- The fixed balanced reduction tree takes fallible leaves. A lane poll that
  observes cancellation keeps a stopped state; its lane visits none of its
  remaining leaves and a shared flag stops every other lane at its next leaf;
  started lanes join. Without cancellation the leftmost failing leaf's error is
  returned. PCG dot, norm scale and scaled-square leaves and centering sums
  propagate `Cancelled` instead of returning zero.
- Prepared numeric publication counts its diagonal metadata scan, polls before
  allocating the shared handles and polls again after publication; topology
  preparation polls before each of its four map reserves;
  `PreparedLaplacianWorkspace::new_with` constructs assembly scratch under a
  `BuildContext` (poll before allocating, poll before returning).
- `solve_pcg_with` copies its owned solution through a fallible counted copy
  and then takes the final poll.
- Every lane kernel (edge-list and CSR products, elementwise and parallel rows,
  restriction, prolongation, pruned transfers, centering, terminal batch
  solves, diagonal solve and scatter) processes and counts a row wider than one
  segment in lane tiles of at most 1024 lanes, each lane keeping its operation
  order. No width cap or batching change; widths up to 1024 lanes, including
  fereg's widths one and four, run the same segments as before.

The fereg repair (after `5ba16717`):

- The prepared route polls before the endpoint reserve, constructs its
  assembly scratch with `new_with` and the host query, polls the weight-ratio
  loop every 1,024 rows and the seed finiteness scan by 65,536-entry segment,
  and publishes a frame's seed admission only after CMG returns the frame.
  Certified seed retention and control-seed admission and retention return
  `Result` and scan by polled segment; a cancelled scan keeps no seed.
  `filled_vec` polls once more after its last segment.
- Graph- and observation-sized scans on the CMG routes poll the host flag:
  input and RHS finiteness checks, weight validation, value and nuisance RHS
  accumulation, nuisance moments, Schur contributions, correction and residual
  updates, column norms and the reused nuisance solution (every 1,024 rows or
  65,536 entries).
- Only `NonConvergence` (numerical breakdown, failed certification),
  `Allocation` (resource refusal) and `InvalidData` (nonfinite numerical data)
  are retained as setup failures or select the explicit route's certified CG
  fallback. `Interrupted`, `Internal` (including CMG's `ParallelRuntime` and
  `InvalidHierarchy`), `InvalidArgument` and `Unsupported` fail the estimation.
  This follows SPEC's rule that Break and fatal errors are never swallowed
  while preserving the documented breakdown fallback; no rule conflict was
  found. Test-only `fault_plan!` statics inject each class at setup and at
  projection.

Second-repair evidence (cloud; full output and real exit codes under
`target/tmp/auto-routing/m3b-r2/logs`, not committed). What each witness
establishes:

- Inner work after observation (reductions). On a two-lane ceiling over a
  four-worker pool, 2^21-entry PCG dot, scaled-square and norm-scale
  reductions and centering sums over 256-entry leaves count every entry
  visited (a test-only counter keyed to the reduced slice). With the query
  turning true at its first or third call, entries visited after the first
  true answer stay within one poll interval plus 1024 entries per lane and the
  total stays below a quarter of the vector; uncancelled reruns visit every
  entry once and reproduce the bits.
- Wide lanes. At 1,100 lanes every counted operation of a complete serial
  interleaved cycle (edge list, rows, restriction, prolongation, centering,
  terminal solves) reports at most 1024 units, with identical counts on two
  runs; cancellation at sampled counts returns `Cancelled` and the rerun
  matches. Every lane equals the matching width-four lane bit for bit, serial
  and planned on a team. Partial-output witnesses for a 100,000-entry factor
  row and a CSR row show that only the first 1024-lane tile's first entry is
  applied before the first count, and the counts are exact (a 1024-lane and a
  76-lane tile). Parallel elementwise rows receive tiles of at most 1024 lanes.
  Widths one and four are covered by the frozen-bit witnesses.
- Prepared publication and the owned PCG result. Cancelling at every poll of
  topology preparation hits each of the four map reserves exactly once;
  numeric assembly on a 200,000-vertex path polls at least five times inside
  the metadata scan, once before the shared handles and once after
  publication; `new_with` polls exactly twice. Cancelling at each of the last
  eight polls of `solve_pcg_with` on 40,000 vertices hits at least five polls
  inside the owned copy and the final poll; the same workspace reproduces the
  reference after every cancellation.
- fereg prepared route (child process, every host poll): preparation polls
  exactly once before the endpoint reserve plus once per 1,024 rows before
  CMG's preparation, then CMG's workspace construction polls, then exactly the
  fill polls (before, per 65,536 entries, final) of the numeric weights and
  the 31,600-entry nuisance seed; assembly polls the weight-ratio loop every
  1,024 rows and the seed scan by segment before CMG's assembly and ends at
  CMG's frame poll; a cancelled assembly leaves no seed admitted; control-seed
  admission and retention and certified seed retention return `Interrupted`
  and keep no seed. These establish poll positions and counts, not timing.
- Classification. Injected `NonConvergence`, `Allocation` and `InvalidData`
  at setup and at projection select the certified CG fallback; injected
  `Interrupted`, `Internal`, `InvalidArgument` and `Unsupported` fail with the
  same code and a later session reproduces the reference.
- Mutation checks: restoring each pre-repair behaviour in a scratch copy
  (reduction without stop state, unmetered metadata scan, missing reserve
  polls, missing workspace publication poll, infallible owned clone, no lane
  tiles) makes the corresponding witnesses fail (logs 160-165).
- CMG at `004d1c4d`: fmt, rustdoc, frozen bits in eight configurations,
  Clippy for seven feature sets and the benchmark crate pass; tests default
  131 (debug and release) and all features 251 (release) pass. The debug
  `parallel` and all-features runs (logs 122, 123) failed in the new planned
  wide-batch witness, which expected cancellation at a threshold near the
  uncancelled query count although lane polls vary with chunk claiming; the
  test-only fix `be1b89d3` accepts success only when the query never reached
  the threshold, and the reruns pass (`parallel` 194, all features 251 debug
  and release, fmt, Clippy; logs 220-246).
- fereg at the `be1b89d3` pin: fmt, Clippy, the ratchet (104,624 -> 104,701
  production lines, declared) and 1,365 fereg-core lib tests (3 ignored),
  including the 42 frozen adapter digests, pass. Before the pin, the R1 route
  witness failed once because its sampling no longer met the prepared
  duplicate sort, which this mostly singleton fixture rarely counts; it now
  requires the prepared numeric-edge stage, and the duplicate sort is
  witnessed at every poll by CMG's assembly tests.
- Actual-plugin replay of the candidate (`835ae095...bbc6`) against the
  post-M3a plugin: 73 of 73 exact; 46 derived explicit runs (including both
  PPML bias modes at one and four workers) exact with identical setup traces
  and the same baseline science notes as before on both plugins; 75 injected
  two-FE runs exact with equal routes (the same nine known-defect rows as the
  first repair, identical on both plugins).
- Standard `./scripts/run_tests.sh`: 315 Python tests pass, then the
  installed-plugin binding check fails (exit 1; "main's runtime changed"). It
  is not green on any M3b handoff: Codex recorded the same failure on
  `b4b53577` (20.4167 s) and `5ba16717` (20.780299459 s). `--source-only`
  (labelled source-only; installed-package admission not performed) passes
  322 Python tests, 1,954 Rust test executions in 85 groups (14 ignored), the
  allocator audits, the fuzz check, the portable checks and the ratchet.

Remaining cancellation limits: allocator, operating-system and join latency;
after a lane observes cancellation, every other lane may finish up to one poll
interval plus one segment (the bound is per lane, not global); fereg row loops
poll every 1,024 rows, so work between polls grows with the row's columns or
nuisance width; fereg's dense-matrix allocations (`DenseMatrix::zeros`) and
O(levels) layout loops are polled only at their surrounding checkpoints; the
ordinary context-free CMG entry points remain unmetered by design and are
unused by fereg's routes. No witness measures wall-clock latency.

Outstanding host checks (Codex, unchanged): the native analytic h16
install/certification witness and the explicit bounded witness, six mapped
Stata fixtures, deep cancellation and builder Break at two and four workers,
the PPML Break fixture, Mac73 and the required 42-cell coverage, and the fixed
Linux/Mac nonregression diagnostics. The 75 injected cases do not substitute
for any of them. No performance claim is made from this cloud host.

### Codex source re-review and prepared-publication follow-up

The second repair was re-reviewed at fereg `26d46327` and CMG `be1b89d3`.
Bounded Astra review found no actionable arithmetic, hierarchy, order, admission
or seed-validity defect. Independent high-effort review confirmed closure of
the four substantive CMG findings and found one remaining constant-size tail:
prepared topology performed its final checkpoint before constructing its two
shared owners. The fereg fatal-error and prepared-seed changes passed source
review; the documented fixed-row-stride limits remain limits, not inner-work
latency claims.

CMG `efb5339459617b1aba99b34195240689deeab61c` constructs the private topology
before its final checkpoint and polls before each shared-owner allocation in
topology and numeric-frame publication. Independent review confirmed unchanged
arithmetic, allocation order/count, layouts and admission bounds. The existing
every-poll test now checks the individual allocation phases. All eight frozen-bit
feature/profile configurations and prepared tests in default, parallel and
all-feature debug/release configurations passed locally; seven Clippy feature
sets and formatting passed with Rust 1.85.1. The initial all-feature Clippy run
failed on two explicit lifetime annotations, and the next matrix stopped on an
unparenthesized bit-pack expression in the experimental-components frozen test;
those failures remain recorded. Eliding the lifetimes and adding parentheses
fixed the lints without changing values. An earlier formatter invocation failed
to locate cargo-fmt before the pinned toolchain was added to PATH.

The standard entrypoint on `26d46327` passed 315 Python tests and failed the
installed-plugin binding check with exit 1 in 20.563819208 seconds. M3b remains
unaccepted pending source integration and the outstanding host checks above.
The final CMG pin is updated in the manifest and both lockfiles. No M3c work
has been released, and none of these source repairs consumes a qualification
retry or resets its budget.

Identities are in [`cmg-m3b-20261020.json`](../bench/results/cmg-m3b-20261020.json).

### Mac source-integration failure and bounded diagnostic proposal

At fereg `dad54c1198f2444684a3ede6d0d4e4713ca55861`, pinning CMG
`efb5339459617b1aba99b34195240689deeab61c`, the standard entrypoint passed
315 Python tests and failed the installed-plugin binding check, exit 1 after
19.525626041 seconds. The separately labelled source-only entrypoint passed
315 Python tests, formatting, fuzz compilation and the two Clippy configurations,
then failed the core library group: 1,363 passed, two failed, three ignored.
The process exited 101 after 205.769801041 seconds. Subsequent integration groups
were not reached. The analytic uncapped h16 build/install/certification unit
witness passed in this run; it does not replace the outstanding native checks.
The host was arm64 macOS, rustc 1.85.1, LLVM 19.1.7, Cargo's `ci` profile.

The frozen pre-consolidation adapter witness failed at explicit
`sampled0/t1/c2`: actual digest `10196290651989369033`, expected
`12891933761922034549`. The preceding capacity-one explicit and Auto comparisons
passed. Independent source review found a portability assumption: the single
expected table was recorded on Linux x86_64, while residual inputs are generated
with `f64::sin()` and every cycle-output bit is hashed with graph and hierarchy
data. The digest does not identify which field differs. This does not prove that
platform-dependent input bits caused the failure, and does not exclude a
Mac-specific arithmetic regression. No expected value was replaced, assertion
relaxed or failed test rerun.

The end-to-end cancellation test failed because its PPML sample did not contain
the phase `prepared numeric edge`. Its query-index subsampling cannot guarantee
observing a particular phase: worker scheduling and additional valid checkpoints
change query indices. The earlier repair changed the required sampled phase
without removing this assumption. Independent review recommends retaining the
end-to-end test as sampled route coverage and strengthening the already-passing
exhaustive serial prepared-route sweep with guaranteed allocation/publication
phase assertions. CMG's inner-work witnesses remain separate evidence. This
diagnosis does not clear the independent digest failure.

Native execution, host qualification and M3c are stopped. No SCC jobs or timing
panels were started; no qualification budget or harness-repair allowance was
reset or consumed by this diagnosis. The monitor was deleted at the preceding
Claude terminal boundary. Both failed entrypoints remain failed.

Pending Johannes's direction under the no-scientific-retry instruction, the
proposed diagnostic is one execution per arm of the existing 42-configuration
adapter witness on this same Mac: accepted pre-M3b fereg `e9efda55` with CMG
`03a26041`, and candidate `dad54c11` with CMG `efb53394`. Both use rustc 1.85.1
and the same `ci` profile. Test-only reporting would separate original input
bits, graph/label/hierarchy bits and every cycle output, while preserving the
fixture, operations and expected table. Each arm has a ten-minute compilation
limit with four build jobs and a 120-second witness limit, with zero retries.
This is a diagnostic comparison, not a qualification pass or new timing panel.
Any same-host difference remains a blocker; no candidate-derived rebaseline,
scientific tolerance change or automatic rerun is proposed.

The compact JSON record contains the full-log and status hashes. Ignored raw
logs remain resumable working material, not retained committed evidence.

### Authorized Mac diagnosis and test repairs

Johannes subsequently authorized retries until Claude completes all milestones.
This permits diagnosed repairs and affected reruns through M3d; it supersedes
the earlier retry prohibition and remaining harness-repair count. Failed
attempts remain failed. Scientific semantics and tolerances, milestone review,
and the fixed timing protocol remain unchanged; retries do not authorize
additional sampling to obtain a performance pass.

The proposed same-Mac comparison completed using the original sine-derived
fixture. Accepted M3a (`e9efda55` / `03a26041`) and the M3b candidate
(`dad54c11` / `efb53394`) matched all 42 configuration digests and all 767
captured input, graph, label, hierarchy and cycle-output sections exactly.
Both differed from the old Linux table in the same 26 configurations. This
isolates the failure to that table's cross-platform assumption for this witness;
it provides no evidence of candidate arithmetic drift on this Mac.

The maintained witness now uses small dyadic residual inputs, avoiding a
platform math-library dependency. Its 42 replacement expectations were
captured exclusively from accepted M3a. A separate candidate capture matched
all 42 digests and 767 sections exactly. Configuration, masks, scaling,
worker counts, application policy and exact output assertions are preserved.
Linux execution of the revised fixture remains a separate required check.

The end-to-end cancellation test now asserts interruption at reached sampled
query thresholds and exact reconstruction, while recording observed phases.
Guaranteed preparation coverage belongs to the existing exhaustive serial
query sweep, which now additionally requires all six unconditional numeric
allocation/publication phases. CMG's actual-inner-work tests retain their
separate role; sampled host-query hits are not inner-latency evidence.
Independent high-effort and bounded Astra review found no actionable defect
in these test repairs. Production comparison with `dad54c11` confirms identical
tokens in 191 runtime modules and identical bytes in 58 artifact inputs.

The diagnostic preparation first failed because the system Python did not
support the archive-filter argument; no source extraction or test ran. The
first reference compilation then failed on an ambiguous float type in added
reporting code; an explicit `Vec<f64>` annotation repaired that reporting.
An initial comparison-script invocation also failed on a missing qualified
`Path` name before comparing results. These operational failures remain in the
compact record. They did not execute native scientific calls or consume timing
observations.

The repaired standard entrypoint passed 315 Python tests and retained its
installed-plugin binding failure (exit 1, 20.083275666 seconds). The separately
labelled source-only entrypoint passed with exit 0 in 241.973364875 seconds:
322 Python tests, 1,939 Rust test executions in 80 groups, 14 ignored, both
Clippy configurations, formatting, fuzz compilation, allocator audits, portable
checks, the complexity ratchet, mock bridge and numeric guards. The core group
passed 1,365 tests with three ignored, including both repaired tests. Linux
mock-plugin audit was skipped on Mac; installed-package admission was explicitly
omitted. M3b remains unaccepted pending the actual native and host checks.

### Root host validation and Linux nonregression failure

The reviewed candidate is fereg
`2073c21a5ec07c7d56cdcd4d0a2d7d778518f27d`, pinning CMG
`efb5339459617b1aba99b34195240689deeab61c` in the manifest and both lockfiles.
Its production tokens and artifact inputs equal `dad54c11`; the intervening
repair changes tests and documentation. The comparison is accepted M3a
`e9efda55` (measured production `d33a1575`) with CMG `03a26041`.
The source archive SHA-256 is
`9b259921ad4b235e4e8427ec8d343ba477c8784a6438335b22b5423f8da85504`.
The compact JSON linked above records full artifact identities, check hashes,
all 1,032 timing observations, uncertainty summaries and scheduler accounting.
Ignored working captures are not retained committed evidence.

The actual universal Mac plugin, SHA-256
`a8acfffd3787d9eb50b1a52f3e6ee175d70403fb2a67be13be9343daa62de960`,
passes binary audit, ABI/load and panic containment. All six mapped Stata
fixtures pass. Builder Break at W2 and W4 passes twice each at task 0,
ordinal 0, with follow-up fits; this covers the first actual builder poll,
not every interior construction point. Public PPML Break passes its five
armed markers and transaction recovery. The runners' final process status
`-15` is their intentional shutdown after the fixtures pass. Mac replay is
exact on all 73 retained cases against the accepted M3a plugin. A separate
16-call explicit-CMG PPML check covers exact/randomized bias, W1/W4, both
artifacts and two repeats: selected CMG, independent numerical checks,
fixed-worker repeats and paired science/signatures all pass.

The final source integration includes the actual analytic uncapped h16
build/install/certification witness. Root additionally ran nine focused CMG
tests on the final pin with Rust 1.85.1, all features and the release profile:
many-leaf PCG and centering reductions, late owned-result copy, wide CSR/LDL
lane tiles, serial/planned wide cycles and cycle/PCG joins all pass. These
actual-inner-work witnesses remain distinct from sampled host-query coverage
and native first-poll Break checks. The initial driver stopped after one
filter matched two passing CSR/LDL tests instead of one; completed tests were
not repeated, and the four remaining commands passed.

SCC build job 8203375 passes the portable 42-configuration frozen witness,
locked release build, actual ABI/load/panic and numeric guards. Its plugin
SHA-256 is
`6c6973be1a0bb2a276d7b2fedb6051f74f45af67f1c1288c06e5d94c0da354b6`.
The audit preparation job 8203392 and three tasks of 8203393 have successful
application results and zero failure/exit accounting. All 42 routing cells
match exactly over 168 calls, including the same six h16 CMG installations
in both arms; maximum paired output difference is zero. Cross-worker checks
cover scientific agreement and discrete/hierarchy decisions, not every
iteration count or output bit.

The Mac plan was frozen before timing (SHA-256
`de96ba1c43f9ca7bac7dbda21a702a05d1ff54d09366e2c53d904f1bbf18cb23`).
All 312 calls completed: 12 cells, one warmup per arm and six mirrored
blocks per cell, with no discarded cells or replacements. All 12
max(5%, 1 ms) upper excess bounds pass. Independent review recomputed those
bounds, verified the schedule, artifact/log bindings and 49 process samples;
maximum observation gap was 5.23 seconds. Ordinary desktop noise remains
included. These are six blocks on one desktop, not six independent hosts.
The initial sandboxed observer failed at `ps` before any native call; the
same plan ran once with the required observation access, retaining that
zero-call startup failure.

The Linux plan was also frozen before timing (SHA-256
`4e91cb8f2798dde8079ff554ad62dc319f85779798774a008ab70e269315c42d`).
Jobs 8203413–8203418 each completed 120 calls on a distinct observed,
isolated 28-core host; report job 8203419 rechecked the retained science,
logs and observations. All seven jobs have zero failure/exit accounting.
No observations were removed or repeated. The initial submission helper
failed under system Python 3.6 before submitting a job; loading the declared
Python 3.12.4 module fixed submission without consuming measurements.

Nine Linux cells pass. The remaining comparisons are:

| Cell | Candidate/reference median | Per-cell ratio interval | Upper excess over allowance | Result |
| --- | ---: | ---: | ---: | --- |
| high-degree weak, W4 | 1.05512 | [1.05168, 1.05967] | +3.597 ms | regression |
| high-degree weak, W8 | 1.06738 | [1.05425, 1.07301] | +7.512 ms | regression |
| weak low, W4 | 0.99952 | [0.96732, 1.10203] | +6.118 ms | inconclusive |

All 720 Linux calls passed independent science and fixed-worker repeat
checks; paired science and workflow remain exact. Whole-process instruction
ratios on the high-degree family are about 1.018 across worker counts.
Other instruction medians are near one. These instrumented complete solves
include parsing/tracing costs; they do not establish uninstrumented release
speed or a material control-cost reduction. Linux uncertainty uses six host
units with two mirrored blocks each. Distinct hosts do not prove independence,
and neither platform's per-cell intervals provide simultaneous 12-cell coverage.

Independent bounded Astra review reproduced all 12 Linux bounds and checked
the 720 log hashes, native timers, traces, repeat signatures, reservations,
censuses and process coverage. The useful diagnostic signal is high-degree
weak's aggregate Schur recurrence phase: about 9–10 ms in the reference and
30 ms in the candidate at W1/W4/W8, with approximately 313 million additional
whole-harness instructions. Hierarchy, iterations and repeats match. This
points toward the scalar nonterminal application path (Auto uses no CMG team
and the edge-list operator), but does not establish a precise source cause.
The largest weak-low W4 block ratio is about 1.198 on `scc-xa4`; no recorded
interference permits excluding it.

M3b remains unaccepted because Linux nonregression is not established.
The standard entrypoint's installed-package binding failure also remains
failed; release packages were intentionally not updated, and private native
checks are separate evidence. The next authorized work is diagnosis and an
arithmetic-preserving M3b performance repair, followed by review and applicable
validation on the changed artifact. This panel remains failed. No unchanged
candidate resampling, timing-budget reset, tolerance change, public two-FE
activation or M3c release follows from it.

### Scalar lane-kernel performance repair

Cloud-reported source attribution. The second review repair's lane tiling (`row_lanes`, CMG
`004d1c4d`) changed the codegen shape of every lane kernel the Auto scalar
cycle uses, without changing its work or arithmetic. Three effects compound:
(1) every row or edge became a callback with a runtime lane range, so the
narrow (width one) loops lost their slice-iterator form; (2) under a polling
meter the row operations of `jacobi` and `scale` captured the width by
reference; the same closure reaches the out-of-line parallel row path, so the
stack slot's address escaped and the serial loop reloaded the width after every
polled segment (the meter's opaque query call intervenes), which kept the
scalar row loops from specializing and vectorizing; and (3) the edge-list
product no longer carried the input/output length equality that the M3a
kernel's dimension checks provided, so each edge paid two extra bounds checks
(24 instructions per edge against 20).
Root reviewed the source changes and successful-path order, polling bounds,
dimensions and resource behavior. The cloud's raw profiles, disassembly and
diagnostic binary/harness identities were not independently reviewed; the
code-generation attribution and instruction counts below remain reported
diagnostics.

Cloud instruction diagnostic using callgrind; diagnostic only, no timing
qualification claim. Workload: a 200x200 weighted
grid Laplacian with default options (levels 40,000/14,102/3,452/1,077/348
vertices, repeats 1,2,2,1, direct terminal). The comparison counts below use
200 scalar team-free applications of one compatible right-hand side, with
instructions counted inside the application entry only. One separate
per-instruction attribution run used 50 applications. Arms: reference CMG `03a26041` (`apply_compatible_into`, the
post-M3a Auto call), candidate `efb53394` with a never-true query (Auto today)
and without a query. Each application runs 19 edge-list products over 321,561
edges in every arm. The observed first-output hash was identical across the
arms; that hash does not establish exactness of every application in a run.

| Arm | With query | Without query |
|---|---:|---:|
| reference `03a26041` | 2,187,517,600 | — |
| candidate `efb53394` | 3,328,178,200 (+52.1%) | 2,801,377,200 (+28.1%) |
| repair `ccd7177f` | 2,300,440,800 (+5.2%) | 2,262,586,400 (+3.4%) |

The initial budget was three initial profiles plus three reserved verification
profiles. It was explicitly expanded during changed-code diagnosis: after two
verification profiles, up to four further profiles were declared, followed by
one 50-application attribution profile and two final verification profiles.
The actual total was three initial plus nine verification profiles: twelve
profiles overall. These expansions concern the cloud diagnostic only; they
neither qualify the changed artifact on a host nor reset the consumed timing
budget of an unchanged candidate. The remaining metered
difference is restriction (+40 million), the terminal solve's required per-row
counting (+30 million of +40 million), centering and prolongation (+13 million
each). Whether the repair removes the Linux excess is for root's fixed panels on
the changed artifact.

Repair, CMG `ccd7177fd6491421bab46de27bdb4d77eb2ee75c` (one maintained
implementation per kernel; counts, polls, lane tiles, operation order,
hierarchies, factors, repeats and bits unchanged): `row_lanes` hands kernels
whole counted segments of rows with every lane (one lane tile per row only for
batches wider than 1024 lanes) and its segment bodies are inlined, so the
constant scalar width reaches each loop under a polling meter; the edge-list
product reslices its input to the output's length; row operations capture the
width by value. fereg `ab8d69c` pins it (manifest and both lockfiles).

Validation reported by Claude (cloud; compact outcomes retained here):

- CMG: all 27 matrix steps exit 0 — fmt; tests default 131, `parallel` 194
  and all features 251 in debug and release (including the many-leaf,
  wide-lane, prepared tail, owned-copy and cancellation witnesses); frozen bits
  in eight feature/profile configurations; Clippy `-D warnings` for seven
  feature sets; rustdoc; benchmark crate Clippy and fmt.
- fereg: fmt, Clippy and the complexity ratchet (no production change) pass;
  1,365 fereg-core lib tests pass (3 ignored), including the 42 frozen W1/W4
  adapter digests, route cancellation witnesses and fatal-classification
  witnesses.
- Actual-plugin replay of the repaired candidate (`2c751c49...2cd7`) against
  the post-M3a plugin: 73 of 73 exact; 46 derived explicit OLS/PPML runs
  (including both PPML bias modes at one and four workers) exact with
  identical setup traces; 75 injected two-FE runs exact with equal routes.
- Standard `./scripts/run_tests.sh`: 315 Python tests pass, then the
  installed-plugin binding check fails (exit 1), as on every M3b handoff.
  `--source-only` (labelled source-only; installed-package admission not
  performed) passes 322 Python tests, 1,954 Rust test executions in 85 groups
  (14 ignored), the allocator audits, the fuzz check, the portable checks and
  the ratchet.

Limits: instruction counts on one synthetic fixture are not the Linux wall-time
regression and do not establish nonregression; the cloud host makes no timing
claim; root owns the fixed Linux/Mac panels, the analytic h16 install, native
Break and fixture checks on the changed artifact. Superseded cloud candidate
plugins (`5f9fd1ed`, `f7b40a9e`, `835ae095`) were deleted after their hashes
were recorded here and in the results file.

### Root review of the scalar repair: three timing cells remain inconclusive

At the root review boundary, M3b remained **unaccepted**. The repaired high-degree Linux cells pass their
nonregression bounds, and source, scientific and transaction checks pass.
However, two Mac cells and one Linux cell do not establish nonregression.
No source defect or external interference explains those three results.
M3c had not been released; Claude was stopped and its monitor was deleted.
Johannes's subsequent acceptance is recorded after this review.

The reviewed source is fereg `671239d47e2d93c0ce11304c10af62551787f4c0`
and CMG `ccd7177fd6491421bab46de27bdb4d77eb2ee75c`, pinned in the manifest
and both lockfiles. This review compared against accepted M3a `e9efda55`
(measured production `d33a1575`, CMG `03a26041`). The source archive has
SHA-256 `48fec7848aa9a177c08b5d24ded4671914ca6b24f13d2b55e857a476e606577b`.
The actual candidate plugins are:

- Mac: `509ef88a0cabedee7ed9b18d45e9c3984c803bfd76f01c6e1ebd11bcccd78db8`.
- Linux: `438c3a4f3e77e2a30373e4c8a3b979aa35eabf00e33a68c5d5dd2890945c16ba`.

Both carry revision label `671239d47e2d` and use Rust 1.85.1 locked release.
Independent high-effort and bounded Astra source reviews inspected all ten
changed lane-kernel callers, successful arithmetic/order, dimensions, actual
work polling, resources and joins. Neither found an actionable defect.

Root validation:

- Source-only integration passed 322 Python tests and 1,940 Rust test
  executions in 81 groups, with 14 ignored. Core passed 1,365 with three
  ignored. Formatting, both Clippy configurations, fuzz, allocator, portable,
  numeric/mock guards and the complexity ratchet passed.
- CMG release/all-features passed 251 executions in 38 groups, including
  many-leaf reductions/centering, allocation/publication tails, owned result
  copying, wide CSR/LDL/cycle lanes, joins and frozen independent witnesses.
  The unaffected feature/profile matrix is reused.
- Actual Mac and Linux binaries passed audit, ABI, load and panic checks.
  The first Mac build failed its binary audit because its path remap covered
  the repository but not dependency sources. No scientific or timing call ran
  on that binary. The corrected full-user remap passed; the failed identity,
  exit and log hash remain recorded.
- All six mapped Stata fixtures, public PPML Break's five markers and
  transaction recovery passed. Builder Break passed twice each at W2/W4,
  covering the first actual task-zero/ordinal-zero poll and recovery, not
  arbitrary deep phases. Separate inner-work tests cover the repaired loops.
- Mac replay was exact in all 73 cases. Sixteen explicit CMG PPML calls
  covered both bias modes, W1/W4, both arms and two repeats with exact science,
  solver signatures and independent checks. Root analytic h16 construction,
  installation/certification and the bounded analytic Schur witness passed.
- The Linux 42-cell audit completed 168 calls, all 42 exact, maximum paired
  difference zero, with the same six h16 installations per arm. Preparation
  job 8203604 and array 8203605 tasks 1–3 have zero failure/exit accounting;
  build job 8203592 also passed. Cross-worker science/discrete/hierarchy
  agreement does not imply every output bit or iteration count is identical.

The standard `./scripts/run_tests.sh` still **failed** after 315 Python tests
at the installed-package binding check (exit 1, 20.437854 s). Release packages
remain intentionally untouched. Source/private-native validation does not
turn that standard entrypoint green.

Both timing schedules were frozen before measurements on this changed
artifact. Mac plan file SHA-256 is
`9346b99f7bc07c2a20bb841bd167da4bcaa390541f6fb6a61fcaa8cda65456f6`;
Linux plan file SHA-256 is
`a2c644ab31369121f851e24a8c8e9181a11a801d346b76414d96527817a5fa18`.
Mac completed 312 calls over six mirrored blocks per cell on one active
desktop: ten cells pass and two are inconclusive. Linux jobs 8203630–8203635
completed 720 calls on six distinct isolated 28-core hosts; report job 8203636
revalidated retained science and captures. All seven jobs have structurally
complete zero failure/exit accounting. Eleven Linux cells pass and one is
inconclusive. All 1,032 calls passed scientific/repeat checks with exact
paired outputs/workflow. No observations were removed or repeated.

| Platform and cell | Median candidate/reference | Per-cell ratio interval | Upper excess over allowance | Verdict |
| --- | ---: | ---: | ---: | --- |
| Mac weak-low W8 | 1.016839 | [0.980604, 1.062151] | +0.589958 ms | inconclusive |
| Mac wide-rhs W8 | 1.005671 | [0.984744, 1.051833] | +0.090181 ms | inconclusive |
| Linux repeated-cells W4 | 1.031880 | [1.014145, 1.050799] | +0.056126 ms | inconclusive |

These intervals cross the allowance; none establishes a regression, and
none establishes nonregression. Independent reviews reproduced all 24
verdicts and checked ordered calls, log/timer hashes, artifact bindings,
repeat/scientific digests and observation coverage. The Mac review checked
49 process samples, maximum gap 5.2284 s. Linux review checked reservations,
censuses, process coverage, all 720 perf CSV hashes/counts and 100% event
running fractions. Neither review found evidence permitting exclusions.

Mac wide-rhs W8 and Linux repeated-cells W4 never install CMG; their two
certified actions use 15/14 and 14/14 iterations respectively. Mac weak-low
W8 installs a one-level direct factor of dimension 499 with scalar CMG
application. Its phase medians show small distributed differences, without
a concrete remaining CMG bottleneck. Another source change solely to obtain
a new timing budget would not be a diagnosed repair.

The original high-degree Linux regression has a useful repaired diagnostic
signal: additional whole-harness instructions fall from about 313 million
to 3.5–4.1 million (about 1.82% to 0.02%), and aggregate Schur recurrence
overhead falls from about 20 ms to 0.27–0.30 ms. W4 and W8 upper excesses
are now −1.171371 ms and −0.161966 ms. This is instrumented complete-solve
evidence, not an isolated-kernel or uninstrumented release-speed claim.
Six hosts do not establish independence; Mac uses one desktop; intervals
are per cell, not simultaneous coverage of twelve comparisons.

The results JSON's `performance_repair.root_validation` retains all 1,032
complete-call timing observations, interval/unit summaries, instruction counts,
actual identities, accounting, checks, failures and independent review
limits. The scoring timer is `seconds`, parsed from `NATIVE_SECONDS`;
the separate plugin `native_s` component timer is retained as a diagnostic.
Final review corrected an initial summary that retained only the component
timer; no measurements, intervals or verdicts changed. All 24 bounds were
then reproduced from the complete-call observations in the committed summary.
Earlier rejected handoffs and failed panels remain recorded.
Temporary raw captures are resume inputs, not retained committed evidence.
Johannes's retry authorization covers diagnosed repairs but does not reset
this unchanged candidate's budget or waive an unmet prerequisite. Advancing
requires his decision on an explicit exception for the three named cells
or a prospectively fixed protocol change; no further measurements are
authorized by this report.

### Johannes's M3b acceptance and M3c authorization

On 2026-10-20, after the three inconclusive cells and their limits were
presented, Johannes instructed: “Yes, please accept M3b and continue onwards.”
M3b is therefore accepted with those three uncertainties retained. None of
their verdicts changes to a pass, no observations are excluded, and this
decision does not reopen the unchanged candidate's measurement budget.
The failed standard installed-package binding check also remains a failure.

The accepted implementation is fereg
`671239d47e2d93c0ce11304c10af62551787f4c0` with CMG
`ccd7177fd6491421bab46de27bdb4d77eb2ee75c`; the complete root review and
observations are committed at `49a3aefa7ec8e513bd3ce150c59462fddab11faa`.
The actual accepted Mac and Linux artifact hashes are those in the root
review above. This source/artifact pair, with its stated limits, is the
last accepted comparison for the next milestone. M3a evidence and all
historical failures remain available; acceptance does not relabel them.

M3c is authorized within plan revision 3: parallel construction census,
emission, contraction and forest work on the existing host-owned team.
It must preserve the consolidated pipeline, arithmetic/order and scientific,
resource, cancellation and transaction guarantees. Worker-independent
representation/reductions, new RHS batching and separately declared
arithmetic remain M3d scope. Claude must report and stop after M3c for
independent source review and applicable validation before M3d is released.
This exception does not waive future prerequisites or authorize protocol,
resource or scientific changes.

## CMG successor milestone 3a: construction-work cap removed

**M3a is accepted for M3b: review, native/transaction checks, the 42-cell
scientific audit and fixed Linux/Mac diagnostics pass.** Johannes's approval
covered two bounded harness repairs before one corrected Linux panel. Historical
failures remain recorded; public two-FE activation stays disabled.

Johannes decided on 2026-10-19 to remove the optional CMG construction-work cap
altogether. CMG is pinned to `03a260411450dd2c1caaefffa3514ac78c0996d7`
(`c23666e8` plus the review repair below), which removes the control/prefix
interface, the factor-affordability preflight and all work charges; controlled
builders take a sticky cancellation query polled every 65,536 counted units and
before every allocation. fereg removes the
`32 W0` allowance, the `4 W0` handoff reserve, the startup-fit test, the
host-context charges and its work ledger; its bounded builder uses the same
cancellation cadence. The single attempt at iteration 16, cutoff 5, one tree,
seed 0, the `A <= 4 W_ref` small-direct criterion, memory admission, restart,
certificate and fallback are unchanged. The overlap experiment, its tokens,
exclusive team admission, replay panel and oracle generator are retired. The
construction-Break fixture now arms `routing_builder_break` on the synchronous
h4/k1 builder; readiness is recorded at its first poll after executed work.

Declared scope: on injected two-FE and general-pair routing arms, builds and
direct attempts previously refused by the cap may now install, and terminal
factors follow the hierarchy options and memory only. Installation, iterations,
hierarchy reports, solver signatures and OLS numerical outputs may change within
section 20 tolerances. Discrete model decisions, public uninjected routes,
explicit `cmg`/`cmgschur` and PPML stay exact. Removing owner fields lowers the
admitted owner bytes of categorical owners by a few dozen bytes.

On a four-core Linux cloud container, `./scripts/run_tests.sh` stopped at the
expected installed-package binding check. `--source-only` passes 315 Python
tests, 1,941 Rust test executions, the fuzz check, seven portable checks and the
complexity ratchet. CMG passes its default and all-feature suites, Clippy for
four feature sets and the benchmark crate. A Linux actual-plugin replay against
`7ccab5a9` passes all 73 cases with zero exact differences. Injected two-FE runs
on the 25 automatic-OLS replay cases are also exact, but none reached a CMG
build. Production lines fall by 1,922 and dispatch candidates by 18.
At that cloud handoff, six mapped Stata fixtures, Break runs at two and four
workers, the 42-cell admission diff, Mac builds and timing were outstanding.
Later host evidence is recorded below. Identities are in
[`cmg-m3a-20261019.json`](../bench/results/cmg-m3a-20261019.json).

Review repair (Codex, first M3a review). The first candidate lost cancellation
during scalar workspace preparation: the bounded builder polled once and then
called CMG's ordinary `workspace_allocation_bytes` and `try_workspace`, whose
level, component and centering banks are initialized without a poll. CMG
`03a26041` restores `workspace_allocation_bytes_controlled` and
`try_workspace_controlled` as cancellation-only queries sharing one
implementation with the ordinary ones. Without a meter the ordinary
reservations, contexts and fills are unchanged; with one, every allocation is
preceded by a poll, level/plan loops and bank initialization (128 values per
counted operation) follow the 65,536-unit cadence, and both queries poll before
returning. Layout, contents, byte counts, error classes and arithmetic are
identical. fereg's `finish` runs both through `Work::cmg`, so cancellation is a
fatal `Interrupted` attempt; memory admission is unchanged. No work ledger, cap,
predictive limit or factor preflight returns.

CMG tests cancel at every poll of component, centering and complete scalar
workspaces and of the layout query, observe polls inside each bank's
initialization, rebuild an equal workspace with identical cycle bits, and run
the allocator tracer over `try_workspace_controlled`; CMG passes 117/227 tests
(default/all features, debug and release), Clippy for default, `parallel`,
`experimental-components` and all features, and the benchmark crate in a
standalone copy. fereg's isolated cancellation test requests host cancellation
from inside a 1.8-million-value workspace preparation at early, middle and late
polls, observes the `Interrupted` mapping at exactly that poll, then rebuilds
the workspace and a bounded candidate with the reference cycle bits. A new
fixed witness shows the `routing_incremental_two` preset resolving to the
uncapped portfolio, building at iteration 16, installing CMG with admitted
storage (no budget and an explicit budget, one and four workers) and certifying
against the paired-graph analytic projection and FE contrasts. `--source-only`
passes 315 Python tests, 1,942 Rust test executions in 80 groups (14 ignored),
the fuzz check, seven portable checks and the ratchet; the standard entrypoint
again stops at the installed-plugin binding check. Linux actual-plugin replay of
the repaired plugin against the first M3a plugin passes 73 of 73 cases with zero
exact differences; the 75 injected two-FE runs are exact with equal routes and
repeats, and again none reaches a CMG build, so the source witness, not that
corpus, is the installation evidence. Production lines fall by three. The
42-cell admission diff, native Stata/Break fixtures and Mac checks were still
outstanding at that repair handoff.

Host evidence reported by Codex at fereg `573a6216` / CMG `03a26041`, not
controlled performance evidence: local source-only checks passed. A private
universal Mac plugin (sha256 `11ab35e2...0a0b`) passed audit, ABI/load and
panic containment; its first attempts (missing cargo `PATH`, then a
personal-path audit failure) are retained. All six mapped Stata 19 MP
fixtures passed. Builder Break at two and four workers recorded exactly two
readiness records each, with rollback, reset and follow-up fits, and the
public-scope PPML Break fixture passed. Native Mac replay passed 73 of 73
cases with zero exact, scientific and discrete differences against the
retained pre-M3a/M2 plugin (`7337e642...b0`); `d0cee50a..7ccab5a9` changes no
runtime or artifact input. SCC build job 8192370 passed (four slots, 123 s,
2.870 GB maxvmem; Linux artifact `2e0c4e85...b35b`, ABI/load/panic/numeric
guards pass). The standard installed-plugin binding mismatch is preserved.
These results predate the trace-only change below and need reuse
reassessment.

Evidence support (second review). The bounded Auto route reported only
installation, refusal and checkpoint; `FEREG_CMG_HIERARCHY` and
`cmgschur_setup` belong to the explicit `cmg` and `cmgschur` paths, so Auto
terminal choices were unobservable. Under the existing `FEREG_SOLVER_TRACE`
opt-in, an installed bounded build now appends stored metadata to its own
`auto_schur_route` record: `terminal`, `levels`, `hierarchy_vertices`,
`hierarchy_nonzeros`, `repeats` (`;`-separated, fine to coarse),
`terminal_factor`, `terminal_factor_dimension`, `terminal_factor_nonzeros`,
`application`, `parallel_operators` and `batch_columns`. A refused or failed
attempt carries none. The fields come from existing CMG accessors that return
stored counts; no graph component, factor or workspace payload is traversed,
and arithmetic, admission, routing, resource handling, cancellation and public
results are unchanged. With tracing off the only cost is the existing
environment check. `bench/routing_science.py` returns the parsed
`route_hierarchy`, rejects an installed record with missing or inconsistent
metadata (factor present exactly for a `Direct` terminal, one entry per level,
factor dimension within the terminal level, scalar application without plan
operators), and rejects metadata on a refused record.
`scripts/route_trace_snapshot.sh` exports a private `git archive` snapshot
of either arm. For pre-M3a fereg `7ccab5a9` / CMG `d11b2ac4` it applies
`bench/diagnostics/pre-m3a-route-hierarchy-trace.patch`, which adds the same
`trace_hierarchy` method byte for byte after the record's legacy
`setup_work`/`setup_limit` fields. It labels each binary
`<revision>-route-trace-diagnostic`; such binaries are diagnostic builds and
are never published. On the paired-graph witness the two arms trace `Direct`
with `64;26` vertices and a 24-dimensional factor (pre-M3a, which coarsened
under the factor preflight) against `64` vertices and a 62-dimensional
direct factor (M3a); the zero-budget refusal traces `installed=0` without
metadata in both.

Checks: fereg unit tests compare the appended fields with the CMG accessors,
and an isolated test captures the real trace file. Its installed
`portfolio_unseeded` record carries consistent metadata, and its zero-budget
record reads exactly `installed=0|refusal=Some(MemoryBudget)` with no
metadata. Parser tests distinguish refused, not-built and legacy records from
an installed record with missing or invalid metadata. `--source-only` passes
315 Python tests, 1,945 Rust test executions in 81 groups (14 ignored), the
fuzz check, seven portable checks and the ratchet, with declared growth of 24
production lines (6 in `borrowed_map.rs`). The bench suite passes 191 tests
(one skipped), and the standard entrypoint again stops at the binding check.
The patched pre-M3a snapshot passes `cargo fmt --check` and `clippy
-D warnings`. Its first focused run (`fe::borrowed_map::routing_tests`,
`fe::cmg_schur`, `fe::routing`) reported `FAILED. 83 passed; 1 failed`. That
command kept only the summary line, so the failing test's identity and output
were not captured and cannot be recovered. Five subsequent repetitions, plus
one capture run, each passed 84 of 84. Those passes do not explain or clear
the first failure, which stays unresolved pending review. No plugin was rebuilt
or replayed for this trace-only change, and no timing was measured.

Codex review of fereg `26cb97d4` / CMG `03a26041`: an independent
high-effort review found no arithmetic, admission, cancellation or production
correctness change in the observation. Local checks pass all 21 parser tests
and the three focused Rust tests (stored metadata, isolated real trace and
the uncapped build/install/certification witness). The standard entrypoint
again passes 315 Python tests and stops at the known installed-plugin binding
check. The private baseline snapshot applies successfully, and its hierarchy
formatter is byte-identical to the candidate's. The patch was regenerated
without context lines to remove whitespace-check errors in blank context;
applying it produces exactly the same three source files as before.

Fresh diagnostic captures must require an explicit `installed` flag and
`refusal` field before using `route_hierarchy`: the maintained parser also
accepts legacy records lacking installation attestation, so `None` alone is
not evidence of a refused build. Retain the comparison helper's completeness
checks. Trace formatting occurs after the construction timing bucket closes;
its cost belongs to the instrumented complete call, not that setup bucket.
Previous native evidence remains attributed to its original binary and is not
relabelled as evidence for a newly built diagnostic artifact.

The lost pre-M3a test failure remains unexplained. No further baseline test
repetition or controlled timing was run during that review. M3a acceptance
and M3b were blocked pending Johannes's decision on a single fully logged
paired check of untouched versus diagnostically patched pre-M3a source.
The historical failure remains recorded after his approval below.

Approved replacement check (Johannes, 2026-10-19), run in the same Linux cloud
container (4 vCPU, Rust 1.85.1). Two private `git archive` snapshots were
made: untouched fereg `7ccab5a9` (CMG pin `d11b2ac4` in the manifest and lock)
and the same source with the zero-context diagnostic patch from `b1df3c31`
(sha256 `8d9bc3af…290d`), which yields the same three files as the
`26cb97d4` patch. Each arm ran
`cargo test -p fereg-core --lib --locked -- fe::borrowed_map::routing_tests
fe::cmg_schur fe::routing` exactly once, with default test threading
(`RUST_TEST_THREADS` unset), no `FEREG_SOLVER_TRACE*`, the container's
`RUST_BACKTRACE=1` and default features. The arms ran sequentially after both
were built, and nothing else ran. Both listed the same 84 tests before
execution. Complete stdout and stderr went straight to retained files. The
untouched run passed 84 of 84 (cargo exit 0, 11.99 s) and the patched run
passed 84 of 84 (exit 0, 11.22 s); neither log contains `FAILED` or
`panicked`. This replaces nothing: the earlier patched-baseline run that
reported 83 passed and 1 failed stays unexplained and recorded. Hashes are in
[`cmg-m3a-20261019.json`](../bench/results/cmg-m3a-20261019.json).

Codex host follow-up on `d33a1575` / CMG `03a26041` uses separately labelled
private diagnostic builds of that revision and pre-M3a `7ccab5a9` / CMG
`d11b2ac4`. Both contain byte-identical stored-hierarchy trace formatting.
Paired universal Mac and Linux builds pass binary audit, actual ABI/load and
panic checks; Linux numeric guards pass. The prior Stata, Break and public/
explicit replay checks retain their original artifact identities and are
reused because review found only the opt-in trace addition since those runs,
with no numerical, admission, cancellation or transaction change.

The retained 42 qualification inputs ran twice per artifact at one and four
workers: 168 native calls, three SCC shards (8195799), all `failed=0` and
`exit_status=0`. Independent input-bound references, original-design
certification, exact fixed-worker repeats, cross-worker science and discrete
decisions pass. Thirty-six cells are exact. The six changed cells are the
one/four-worker variants of high-degree-weak, weight-dynamic and weak, all
within the previously declared numerical/iteration scope; their largest saved
output difference is `2.067568338759429e-12`. Both artifacts install CMG in
the same six cells. High-degree-weak changes from five levels and a
72-dimensional terminal factor to four levels and dimension 628;
weight-dynamic changes from three levels/dimension 16 to one level/dimension
499; weak changes from three levels/dimension 72 to one level/dimension 499.
The compact result records every cell's hierarchy, repeats and iterations.
These observations are science/admission evidence, not controlled timings.

The fixed Mac D.3 diagnostic completes all 312 calls (12 cells, six mirrored
blocks per cell, one warmup per arm), without retries, exclusions or scientific
failures. All 12 per-cell upper excess bounds satisfy `max(5%, 1 ms)`.
High-degree-weak candidate/reference median ratios are 0.954/0.924/0.915 at
one/four/eight workers; weak-low ratios are 0.429/0.593/0.603. The other six
cells retain exact science and workflow, with median ratios from 0.992 to
1.017. Independent high-effort review reproduces the frozen identities,
312 log/trace/repeat checks, 24 compact snapshots, complete observation spans,
72 mirrored scores and 24 intervals. Ordinary desktop activity remains in the
measurements. Uncertainty uses six blocks on one Mac, not six hosts, and the
instrumented diagnostic does not qualify public activation.

The Linux D.3 attempt reserves six distinct complete 28-core hosts through
`omp28` (jobs 8195862--8195867). Every task stops after its first warmup,
before any timed block: the adapter passes the frozen `oracle` filename into
scientific metadata, where the legacy dense checker expects a numeric object.
The input-bound `routing_oracle` check runs before this type error. The six
warmups and all failure records are preserved. A narrow helper now removes
only descriptor-path keys from runtime metadata; the bound reference and its
scientific checks remain. Five focused tests, including an oracle-dispatch
regression assertion, pass, and independent review finds no bypass or artifact,
tolerance, schedule or budget change. Saved-output validation job 8195882
passes all six retained outputs, including their independent numerical checks,
without executing a native solve; original bindings and before/after raw-output
hashes match. Its accounting is `failed=0`, `exit_status=0`, 13 seconds on four
slots. The six original jobs retain `exit_status=1` despite scheduler `failed=0`.
The original 720-call Linux budget permitted no retries, so work stopped for
Johannes's decision. He approved the next five bounded repairs. Before the new
panel, a second reporting repair addressed JSON key ordering: identical
changed-field paths can be traversed in a different order after serialization.
The comparison canonicalizes only that order and rejects duplicates; six focused
tests pass, including negative cases for missing paths and changed values or
scope. Independent review reproduces all 12 saved Mac comparisons, with only the
six scoped cases needing normalization. No new native call was needed for this
repair. The two repairs leave three authorizations available; they do not permit
sampling after scientific failure or unfavorable or inconclusive timing.

The corrected Linux panel (8196135--8196140) completes all 720 calls on the same
six isolated hosts, with the same artifacts, inputs, schedule and per-attempt
budget. Every task has `failed=0`, `exit_status=0`, 157--159 seconds wall time
and maximum virtual memory of 1.258--1.296 GB. Every original-design certificate,
input-bound oracle, fixed-worker repeat, cross-worker/host discrete decision and
hierarchy comparison passes. Report job 8196144 also passes (`failed=0`,
`exit_status=0`), rechecking all 720 log/perf records and retained snapshots.
The failed attempt plus corrected panel used 726 native calls in total.

All 12 Linux per-cell nonregression upper bounds satisfy `max(5%, 1 ms)`.
High-degree-weak candidate/reference median ratios are 0.946/0.902/0.889 at
one/four/eight workers; weak-low ratios are 0.520/0.633/0.698. These are changed
hierarchy/whole-solve observations. The other six cells retain exact science and
workflow, with median ratios from 0.991 to 1.014; their unchanged-workflow
single-worker instruction ratios are 0.999998 (repeated-cells) and 1.000041
(wide-rhs), providing no material control-cost reduction claim. Independent
review reproduces all 12 intervals, with the closest upper excess 1.178 ms
inside the allowance. Each uncertainty unit is one physical host summarizing two
mirrored blocks; there is no simultaneous 12-cell coverage or independence claim.
All counters are `instructions:u` at 100% running and cover the user-space
harness and inherited threads, including parsing/tracing. Both platform panels
are instrumented diagnostics, not uninstrumented release or public-activation
qualification.

These results close the M3a prerequisites in the approved plan. M3b may proceed
from production `d33a1575` / CMG `03a26041` plus the subsequent evidence-only
commits. The prior installed-plugin binding failure, lost baseline-test failure
and initial Linux harness failure remain recorded. Existing valid native evidence
retains its original artifact identity; no failed attempt was relabelled as a pass.


## CMG successor milestone 2: initial graph construction

**M2 is complete: source/native checks and the fixed cost diagnostic pass.**
Work stops here for Johannes. This does not qualify two-FE activation.
CMG is pinned to
`d11b2ac46de73b6e3d5e37db84f816e3a8cccb6d`. The upstream mapping/merge path and
fereg's independent edge emission use the existing host team, a fixed 65,536-edge
threshold and dimension-based chunks. Canonical aggregation, compensated suffix
sums, duplicate reduction and diagonal accumulation retain their order.
Optional suffix scratch is eight bytes per aggregated cell and row; output
slots hold temporary dispatch metadata. Hierarchy, application and polishing
remain unchanged. The [architecture](ARCHITECTURE.md#cmg-host-team-and-stage-admission)
describes the admission and cleanup contract.

The final focused fereg suite passes 34 tests. Upstream focused checks, all-feature
Clippy and default-feature checks pass; narrow-feature Clippy exposed an existing
profiling-only unused-function warning, retained without deleting used code.
Full source integration passes 319 Python tests, 1,959 Rust test executions and
seven portable checks. The first integration attempt stopped on the stale fuzz
lockfile; updating only its CMG pin resolved that setup prerequisite. An earlier
focused test run exposed shared-team lease contention between tests; process
isolation fixed the test without changing production arithmetic or serializing
the entire suite. Complexity grows by 282 production lines and four dispatch
candidates, with no new oversized function or file.

The private universal Mac build passes audit, ABI/load and panic containment.
All six mapped Stata fixtures pass, including the near-within witness; all four
fixed Break runs at two and four workers pass, with two cancellations per run,
rollback, reset and terminal cleanup. Those native Break runs stop at the first
64-unit bookkeeping gate; they do not establish interruption inside a parallel
graph kernel. Focused host-stage tests cover kernel joins, cancellation and panic.
Independent source and raw native-evidence review passes with that limitation.
Compact identities, checks and failed-attempt summaries are retained in
[`cmg-m2-20261019.json`](../bench/results/cmg-m2-20261019.json).

Johannes authorized implementation of M2 after the repaired M1 reference
`9244364520df5cc44c9a9b4da66c3dba844ff090`. The target is the actual synchronous
`incremental_two` direct-or-h16/k1 path: the incumbent pauses at iteration 16
and construction may borrow the full admitted host team. The separate overlap
experiment retains its one-lane construction ceiling. Routing, degree cutoff 5,
one sampled tree, seed zero, work cap and restart reserve stay fixed.

Declared scope before implementation: parallel validation/mapping, bounded
sorting and independent edge arithmetic in the initial graph. Logical partitions
are independent of worker count. Canonical cell aggregation, compensated suffix
sums, duplicate-edge reduction and diagonal accumulation retain their order and
arithmetic. Hierarchy construction, application and polishing are outside this
milestone's edits. Successful graphs, scientific results and solver signatures
must remain exact when admission agrees. New stage planning and scratch change
accounting near resource boundaries; refusal must retain charges, join all work
and publish no partial state. Insufficient optional scratch selects the serial
path before dispatch, without a retry of partially executed work.

Focused witnesses compare raw sampled edges, canonical graphs and diagonals,
hierarchies and cycle outputs at one, two and four workers, with ties, duplicate
endpoints, multiple components and sampled high-degree rows. Independent Schur
and clique witnesses, fixed-worker repeats, low-memory/refusal boundaries,
actual lane ceilings and joined cancellation/error/panic checks remain gates.

The initial diagnostic budget is four synthetic families generated once with
seed 2026101902: high-degree weak (10,000 workers, 1,000 firms, 60 periods),
repeated cells (5,000/500/20), wide RHS (5,000/500/20, 16 regressors), and weak
(5,000/500/20) in low-memory mode. On one isolated SCC allocation, each runs the
unchanged repaired reference at one, four and eight requested workers, with one
warmup and two recorded complete solves: at most 36 native calls. Up to two
additional CPU profiles target high-degree weak at one and eight workers.
Every native call has a 120-second limit; the complete diagnostic has a
1,800-second budget. These are cost diagnostics, not qualification or a speed
claim. Profiling failure is retained and does not authorize additional sampling.
A fixed comparison budget was declared after production edits and focused
checks, before collecting candidate timings. M2 ends with a report for Johannes;
hierarchy work, further qualification and two-FE activation require his direction.

The first reference diagnostic, SCC job 8179449 on `scc-zj4`, stopped after
one successful native warmup because its reader passed the binary-sidecar
`columns: []` marker to the strict metadata validator. The application failure
is retained. Independent inspection verified return code zero, both completion
certificates, the 600,000 by 12 binary output shape and its exact fingerprint.
The bounded reader repair uses the maintained parser, consumes that warmup
without repeating it, and permits only the original remaining 35 calls and
up to two profiles on the same physical host. It debits 27 seconds from the
original budget, leaving 1,773 seconds. No input or artifact was regenerated.
The remaining schedule completed in job 8179463 on the same `scc-zj4`, with
`failed=0`, `exit_status=0` and 51 accounted seconds. All 36 original calls and
both profiles completed in 76.05 seconds including the prior debit. The first
job remains failed. The two 99 Hz profiles have 164 and 215 CPU samples and
zero lost samples, but include substantial harness text parsing. They locate
construction sorts without measuring a precise native-stage cost share.

After source and mapped native checks, the fixed M2 comparison budget was the
same four input families at one, four and eight requested workers, against
the repaired reference. Six distinct isolated 28-core SCC hosts each run one
warmup per arm and two mirrored blocks per cell: 120 calls per host, 720 in
total. A seed of 2026101903 chooses the first arm in each balanced block.
Each host has 3,600 seconds and each native call 120 seconds. The focused Mac
comparison uses one warmup per arm and six mirrored blocks per cell: 312
baseline calls, with at most one externally interfered whole-cell replacement
on four cells (416 total). Its whole budget is 3,600 seconds. The existing
practical Mac observer retains ordinary desktop noise and requires two clear
intervals within 120 seconds before a replacement; timing values never select
retries. Phase/solver tracing is enabled for both arms throughout.

This is a construction-cost comparison, not a new routing qualification.
Scientific or solver-signature differences stop the comparison for diagnosis,
including admission differences; they are not silently excluded. SCC summaries
use physical hosts as the independent units, with each host's mirrored blocks
combined first. Mac uncertainty uses its fixed mirrored blocks conditionally;
neither design proves independence or supports a simultaneous-coverage claim.
The comparison retains setup, application, polishing and complete-solve costs,
memory/admission observations and all failed calls. No automatic extension is
authorized by a slow or inconclusive result.

The comparison completed all 720 Linux calls in jobs 8179530–8179535 on
`scc-tj3`, `scc-tj4`, `scc-tn3`, `scc-tn4`, `scc-tq3` and `scc-tq4`.
Every job has `failed=0`, `exit_status=0`, 28 reserved cores and valid whole-host
observation evidence. Application time is 124.20–125.89 seconds per host.
The single metadata report, job 8179555, also passed. Its deliberately pending
accounting field is resolved by the separately verified six scheduler records.
The Mac completed all 312 calls in 167.74 seconds with no retries. Ordinary
desktop activity remained in its 47 process samples; the maximum sample gap
was 5.231 seconds, with no classified competing-compute hit. The root process
and matching harness commands were absent at the terminal census. This is not
an exhaustive descendant PID/birth ledger.

Independent raw reviews reconstructed 720 Linux and 312 Mac call records,
144 Linux and 24 Mac first-call output snapshots, completion certificates,
resource decisions, exact between-artifact science/signatures, fixed-worker
repeats and cross-worker numerical agreement. All 12 per-cell complete-call
excess-time upper bounds are below zero on each platform for max(5%, 1 ms).
These are conditional per-cell bounds, without a simultaneous-coverage claim.
Successful timed full sidecars were deleted by the fixed producer; reviewers
could reread the retained first requested output arrays and all recorded
fingerprints, not replay every unavailable full array. No oracle solve or native
measurement was repeated during review.

The high-degree weak case shows where M2 helps:

| Platform | Workers | Initial setup, reference → M2 | Paired complete-call change |
| --- | ---: | ---: | ---: |
| Linux | 1 | 129.740 → 125.355 ms | −1.11% |
| Linux | 4 | 125.890 → 106.293 ms | −4.60% |
| Linux | 8 | 125.085 → 102.555 ms | −5.95% |
| Mac | 1 | 50.827 → 58.622 ms | +2.22% |
| Mac | 4 | 48.435 → 44.272 ms | −2.02% |
| Mac | 8 | 48.310 → 43.423 ms | −1.64% |

Setup here is the initial absorption's `RSOLVER` setup field, excluding final
residual/polish setup. Each timed arm is geometrically averaged within a block;
Linux combines its two blocks within host before taking the median across
six hosts, while Mac uses six block units. Complete-call changes are paired
unit median ratios. Warmups are excluded. The separate raw Mac review also
reports per-call medians of summed projection and polish setup; those answer
a different question and are not substituted in this table.

The Linux gain at four/eight workers is concentrated in initial setup;
initial solve remains about 73/47 ms and polishing barely changes. High-degree
charged setup work falls from 792,495,096 to 775,849,280 at all worker counts;
these bookkeeping units are not CPU instruction counts. The Mac one-worker
setup penalty is real in this sample, although complete-call cost stays within
the allowance. The weak-low eight-worker Linux median is 1.23% slower, also
within allowance. Repeated-cell and wide-RHS cases have no checkpoint admission
trace, so their small timing changes are controls rather than evidence of an
M2 construction benefit. All-cell stage, memory and work summaries are in the
compact result.

Setup still costs about 103 ms on Linux and 43 ms on Mac at eight workers.
The traced Schur recurrence bucket, which includes repeated CMG application,
is about 12/16 ms respectively. It is not an isolated CMG-application timer.
Canonical census/finalization and controlled hierarchy construction remain
serial, but these logs do not separate graph and hierarchy cost. The proposed
next milestone would first measure that split, then consider bounded,
deterministic coarse contraction with the existing host-team contract and
canonical reductions. It is a proposal, not work started by this report.

Implementation is committed in fereg `d0cee50a2c575564b512544841b822556ce92c26`
and upstream CMG `d11b2ac46de73b6e3d5e37db84f816e3a8cccb6d`, both pushed.
The source-bound handoff retains the exact compiled production identity,
artifact hashes, build settings, compact checks and review hashes. Capture
names identify reviewed temporary evidence, not promised permanent raw files.
Two-FE remains disabled, many-FE stays enabled, and the old failed two-FE
qualification remains failed. No hierarchy edits, new qualification, routing
retune, main merge or release-package update follows automatically.

## CMG successor milestone 1: execution and admission foundation

**The bounded follow-up repairs both validation blockers: source integration,
all six mapped Stata fixtures and four fixed native Break checks now pass.**
Independent source and evidence review is recorded below. The strict-CMG
repair changes arithmetic and cost; its performance is unassessed. Work stops
before M2 for Johannes's review, without claiming performance qualification.

Johannes authorized this milestone of the staged successor after the two-FE
qualification above. It establishes a shared execution interface and canonical
admission, without optimizing graph construction, hierarchy construction or
application kernels. Work stops after this milestone for his next decision.
No diagnostic campaign, further qualification, two-FE activation or release
package update belongs to this milestone.

The host adapter borrows only fereg's actual installed Rayon registry with a
live action admission. It retains the pool owner for the synchronous operation,
limits an overlap borrower to one lane, rejects an unadmitted fereg worker and
uses the existing serial path outside fereg's team. The scoped upstream view
cannot escape into a hierarchy or workspace. Standalone CMG-owned execution
remains available. A controlled stage precharges planning, every logical chunk
and its dispatch overhead through the coordinator's ordinary mutable controller;
workers receive disjoint, previously admitted storage and bounded kernels.
There is no shared atomic work quota. All dispatched work joins before stage
return or unwind; errors reconcile in logical order and cancellation dominates.

The current numerical builders remain serial. Five upstream entrypoints each
charge 64 new work units, including serial fallback, before borrowing the host
context. The existing total budget and incumbent handoff reserve do not grow.
Factor preflight receives the actual post-charge remainder. Consequently this
is explicitly an admission and callback/work-trace change: near a work boundary
it can refuse construction or choose coarsening instead of a direct factor,
changing the hierarchy and solver signature. It does not claim universal
historical bit or admission equivalence. The arithmetic kernels, scientific
tolerances and model/sample/rank/omission semantics are unchanged. See the
[execution contract](ARCHITECTURE.md#cmg-host-team-and-stage-admission).

Focused tests cover the actual host registry and admission lease, one/four-worker
ceilings, overlap isolation, pre-dispatch refusal and exact canonical traces,
and reuse of a hierarchy/workspace across separate installations. A terminal
factor boundary witness checks equality and two smaller remaining allowances
at one and four workers against the original two-node Laplacian. Its first test
asserted a particular null-space gauge and failed: the direct LDL factor fixes
a root gauge while the diagonal terminal is centered. The corrected independent
witness checks the original operator equation at the same `1e-12` tolerance;
both routes pass. This was a test correction, not a solver or tolerance change.

Independent review also found that the newly public owned-executor coordinator
could be misused to launch a detached Rayon task holding the last executor
owner. Joining its own worker on final destruction would deadlock. The fix
detects that path, signals termination and detaches handles; ordinary external
retirement still joins every worker and its thread-local destructors. Detached
callback work remains outside the interface contract, with asynchronous
retirement explicitly disclosed. A bounded subprocess regression exercises
last-owner destruction and eventual worker retirement at one and four workers.

The reviewed upstream change is CMG `8b3c50507165d834199c03520f9cacc933f97778`
on `codex/host-execution-foundation`, based on the previously admitted
`3422b0408023769f08f33391b90a613745601e7d`; unrelated newer upstream work is not
included. The fereg manifest and both workspace/fuzz locks bind that exact
revision. Independent high-effort architecture review passes after the two
findings above. Upstream checks pass six lifecycle tests, nine host/stage tests,
four controlled-builder tests, four factor-preflight tests and the scoped-view
compile-fail doctest, plus all-feature/all-target Clippy, no-default-feature
compilation, formatting and diff checks. The check-capture SHA-256 is
`7892547e8bfc9bd93923bc9fcc051a07f28ba0616078ad616a2082c9992660c7`.

The fereg production identity is
`a26fb6f7853009ae6656f9249ac31df767c48a4c6c213ccd7c63a5508fead31c`
across 192 runtime modules and 58 artifact inputs. The complexity inventory
records 33 added core production lines for the scoped adapter, entry accounting
and post-charge remainder; the other complexity metrics do not increase.
The source integration and private native checks below bind this identity.
No performance improvement is claimed by this foundation.

The default `./scripts/run_tests.sh` invocation passed its 312 Python tests,
then failed the installed-package runtime binding check because this branch's
source differs from the preserved release artifact. That failed invocation
remains failed; the documented explicit `--source-only` development mode is
used separately and cannot establish installed-package admission. The first
source-only invocation then stopped on two `drop_non_drop` Clippy errors in the
new tests. Removing the unnecessary test-only `drop` calls fixed those errors
without changing production identity. The failed default and source-only
capture hashes are respectively
`a133b170a28b39ea7bd16bb4b57c83b0198e8c9fd50b8830ca6b924b4e1e8b20`
and `2e1eadbd83cfd431be525700324fd051c07934909acee86e6f1195091592ee02`.

The final `RUST_TEST_THREADS=1 ./scripts/run_tests.sh --source-only` passes in
205.74 seconds on Rust 1.85.1: 312 Python tests, 1,951 Rust test executions,
Clippy, fuzz compilation, isolated allocator/FFI audits, seven portable tests,
the complexity ratchet, mock SPI and numeric input/output guards. The Linux
mock-plugin audit is skipped on Mac; no Linux or installed-package admission is
claimed. Its capture hash is
`f03bc5d7c0af3e3965e8360c18791b1bcb3c63d71973a0aa235195f8c424ca09`.

The first private universal Mac build compiled both architectures but failed
the personal-path audit because its panic locations contained source/Cargo
paths. That artifact was not admitted or used for Stata tests. The same source
was rebuilt with Rust path remapping, retaining the failed build capture
identity `e373fc3c17bda889c3e718b11f567e1c06bd7cd117a672ea72a27839b8a9017e`
and rejected binary identity
`a4479ba5c0193661f470b85df05470a137333c7f7eca7bf17594aa59a834cb1f`.

The remapped build passes in 148.41 seconds using Rust 1.85.1, the locked
release profile (optimization 3, fat LTO, one codegen unit, unwind), and the
maintained universal Mac build/audit/load entrypoint. Both x86-64 and arm64
slices pass binary inspection; actual ABI/load, worker and panic-containment
checks run on this arm64 Mac. Build identity `m1-src-a26fb6f78530` denotes the
production identity above, not an earlier Git commit. The private plugin is
`96352ecfea434212761ecf178a0f744084604155424b17651c683257d3792d52`;
the actual loader harness is
`5285ce62b666119c6271cad54e555ebb2a930d102575e5eb29d0cb118440703d`.
The successful build/audit capture is
`9853eb05f17032f776640ed11d9d9b487561edf48513cacee755543acf52eec6`.
This is private development validation, not installed-package admission.

Five of six mapped Stata 19 MP fixtures pass on that exact plugin:
`ols_cmgschur`, `ols_cmgschur_sampled`, `ols_cmgschur_nonconvergence`,
`ols_cmg_raw_residual_oracle` and `estimate_ols_auto_cmg_route`.
`ols_near_within_cmg_oracle` fails with `r(459)` and
`borrowed CG direction denominator is nonfinite or nonpositive`. This remains
a scientific failure, not an interference exclusion. The fixture, CG recurrence,
explicit CMG construction/polishing and Auto dispatcher are unchanged from the
exact parent `59c8e8d1056a56aab12730c8fc4e8c27e923c81d`.

A private build of that parent, using the same toolchain, release settings and
path remapping, also passes universal binary/ABI checks and reproduces the
same failed fixture. Its binary hash is
`b15c33189a3b335a24bfeb6c65aacb870a149ca47dd3a7b71a21fab919a29996`.
One further diagnostic per artifact adds only per-call display labels to a
temporary copy of the fixture, preserving its data, options and assertions.
Both fail on the same first offending call: ordinary fit, analytic weights,
`memory(low)`, `threads(1)`, explicit `solver(cmg)`, tolerance `1e-12`.
The preceding unit-weight fit/recovery and fast analytic-weight calls pass.
The unchanged strict CG polishing checks can reject a nonpositive direction
product; this diagnosis does not justify changing them or weakening the oracle.
The committed fixture itself is the reproducer. No solver or tolerance repair
was attempted, and no full battery was restarted for diagnosis.

The two-worker actual builder Break check passes two cancellations, rollback,
lease/reset checks and follow-up fits. The four-worker invocation reached the
Stata success/reset sentinel but failed the runner's requirement for exactly
two positive-work trace records. The runner deleted that failed raw trace,
so its record count and cause cannot be independently reconstructed. One
bounded diagnostic invocation, adding only raw-trace capture before the same
validator, passes with two records (`issued=373696`, `charged=64`, `ordinal=1`).
This later observation does not erase the first failure or establish a tooling
repair. No assertion, runtime source or observer criterion was relaxed.

The summaries above are the permanent record; capture identities are:

| Evidence | SHA-256 |
| --- | --- |
| Six-fixture M1 summary, five pass and one failure | `e075a767b5f43782b14fe0d39eb41a9914f536f11f1ab0b80a507c232423a0b7` |
| Exact-parent build/audit/ABI | `3b30dbea1e447b157492cba041646bb1c187f0a686a983042448929c4623b4c1` |
| Exact-parent failed-fixture summary | `0c4370fb6c3a450047fdea7d12648918c459eb424770a47d8b6b4ef1879f45d2` |
| M1 labelled failure capture | `6d755bbe97f6a7e56adbe24faab8e23fe1982167a9e5bb6176c6f01962746fa0` |
| Parent labelled failure capture | `c866e6d5b115ddd4a5ecb5745491c6af09784ee2dff63835b3bd28010804e7ed` |
| Two-worker builder Break pass | `5b2e892e0db129baf3d831afc22e0856fac74bf3996f29b4a22aaada31406e72` |
| Four-worker builder trace-check failure | `353c7169dc4dae9683efcfe8c28bc463dd135ab5beb192fbb0bcd547db1a779d` |
| Four-worker diagnostic capture | `6d1da18c12ad0011caad041b885a7c31330b6b66fc98e7b0d41f99363965ffd2` |
| Diagnostic raw trace | `87cf0765f3dabb3fae177bc7a4f5875e59119fa36c7ae44f17f0a776e12713c1` |

This checkpoint is not a passing M1 admission or performance qualification.
Many-FE stays enabled, two-FE stays disabled, and installed packages stay
unchanged. Dependent work stops for Johannes's direction on the scientific and
trace-validation blockers. Current reproducer artifacts may remain locally
while blocked; ignored files are not permanent evidence.
Independent evidence review verifies the pin, plugin/capture identities,
source-test count and matching parent/candidate failure, and agrees with this
blocked checkpoint disposition rather than milestone acceptance.

### Bounded M1 follow-up: trace evidence repair

Johannes authorized a bounded follow-up to repair the two validation blockers,
then stop again before M2. The historical four-worker trace failure remains
unexplained because its raw trace was deleted. Source review established a
separate concrete defect: concurrent append handles could interleave a formatted
trace record and its newline, or short-write fragments. A merged launch/ready
record produces the exact event-count failure, but this does not prove that
the historical run followed that interleaving. Publishing readiness before the
trace is not sufficient to explain a missing terminal record: the worker emits
the trace synchronously and joins before the Stata terminal sentinel.

The sink now serializes each complete file record, including short writes and
the newline, and reports append errors to stderr. The maintained Break runner
rejects trace I/O errors, merged/malformed records, duplicate fields, extra
events and nonpositive work. It retains raw trace bytes (distinguishing empty,
missing and unreadable files), PTY bytes, the original validation error, exact
plugin/fixture/runner identities and invocation metadata. Child reaping precedes
terminal capture; final draining is bounded, cleanup errors mark the result
failed, and capture failures cannot replace the original validation error or
skip cleanup. Existing evidence prefixes cannot be overwritten.

Independent review corrected cleanup-status ordering, an unbounded final PTY
drain, startup cleanup gaps and a marker/evidence variable collision before any
new native invocation. Eleven focused Python tests pass, including complete
mocked marker attempts with successful and failed exact-count validation.
A deterministic Rust regression forces short writes and a paused first record
while a second producer enters; every fragment must hold the common lock.
It passes without sleeps or probabilistic stress. The trace change adds 14
core production lines; it changes diagnostic I/O, not numerical arithmetic or
solver policy. Independent source review approves this scope. The fixed native
verification set and final combined integration were pending at the separate
trace commit `ee65e67b`; their completed results are recorded below.
The Python and Rust check-capture hashes are respectively
`8843ba1e0235707221f8113c4a2063965ab025e86d2d9c669b9625b88ec7e257`
and `dd628f20648559c38f830b6628aebd8973e636e88e4724face2e2791fbdf1fa6`.

The actual two-FE builder fixture's first 64-unit gate is `fereg.cmg.prepare`
bookkeeping, followed by its first weighted-row gate. Its marker does not prove
execution of a parallel numerical kernel, nor of the later host-context gate.
The separate scoped-stage tests retain the concurrent-work, cancellation,
join and canonical-error obligations.

### Bounded M1 follow-up: strict-CMG scientific repair

The unchanged 48-observation near-within fixture isolates a defect in strict
CMG polishing, not in the new executor. For a symmetric weighted sweep `S`,
the CG action `I-S` is positive semidefinite: its range is the FE span and its
null space is the within space. Rowwise residual and direction updates can
accumulate roundoff in that null space. The recurrence norm then counts error
that the direction's curvature cannot control. In this fixture the independent
weighted dummy/SVD action has six positive eigenvalues between 0.999427 and 1;
the identified failure is not an ill-conditioned positive subspace.

The reduced analytic-residual witness reaches a direction with 99.999756% of
its norm in the null space at the third iteration. The actual prepared-fit
debug reproducer also exposes the defect: after an exhausted CMG iteration and
strict fallback, it returns success after 72 CG iterations with a residual
error above 193 million, despite small reported recurrence and FE certificate
values. This is distinct from the preserved release-build curvature failure;
both fail the independent residual truth. The fixture's analytic coefficients
are `(0.7, -0.4)` and its residual is the constructed Hadamard noise `4uv`.
Its requested `1e-12` fit tolerance invokes the existing `1e-14` final residual
correction. Temporary diagnostic instrumentation was removed.

The declared repair represents strict-CMG residual, direction and operator
products through FE coefficients and reconstructs their observation values
with compensated summation after cancellation. It addresses the observed
null-space contamination without relaxing the Hestenes, recurrence,
original-design certificate, iteration-limit or unsafe-curvature checks.
It is not a proof of exact floating-point range preservation for every design.
Three internal coefficient banks are now required without returned FE
coefficients, and four with them. Checked joint-memory admission accounts for
those banks; tight budgets can therefore refuse a request previously admitted.
Reconstruction uses the existing fixed row Break checkpoints. Ordinary CG,
saved-model recovery and PPML keep their prior arithmetic. Strict OLS CMG
polishing/fallback/final-residual outputs and iteration signatures are affected;
model, sample, rank, omission and separation semantics are unchanged.

Independent mathematical and implementation review approved this scope before
native validation. The new core regression checks the exact 48-row witness,
returned coefficient reconstruction, RHS widths 1/4/17/65, fixed-worker repeats
and one/four-worker agreement. A separate prepared-fit regression exercises the
actual low-memory analytic-weight path and independent coefficient/residual
truth. Existing weak-cycle, extreme-scale, unsafe-curvature and iteration-limit
checks remain. One focused CMG run passed 103 tests and failed the old workspace
expectation; its corrected test checks the newly required banks at the exact
budget and rejects one byte less. The failed invocation remains failed.

Final source integration passes in 203.34 seconds: 319 Python tests, 1,954 Rust
test executions, Clippy, fuzz compilation, isolated allocator/FFI audits,
portable checks, the complexity ratchet, mock SPI and numeric guards. The Linux
mock-plugin audit is skipped on Mac. This explicitly source-only invocation
does not admit installed packages. The scientific repair adds 42 core
production lines beyond the separate 14-line trace repair; current totals are
106,533 core and 15,284 plugin production lines. The additional dispatch is the
strict reconstruction check. No runtime code is moved into ratchet exclusions.

Performance is not assessed by these correctness checks:
strict polishing now performs additional FE-product reconstruction and retains
additional coefficient storage. Prior routing timings do not establish its
nonregression. Any subsequent performance acceptance needs a bounded paired
comparison on declared strict-CMG polishing/fallback workloads, including
low-memory and multiple-RHS cases, with the existing max(5%, 1 ms) allowance.
No timing campaign, later milestone or two-FE activation is part of this repair.

### Bounded M1 follow-up: final native evidence and handoff

The final source identity is
`353cc6381360099000922c83c5ea9f6c52b84ec0d92748071f6748c9dffb2d2a`.
It hashes compact sorted JSON containing `check_complexity.production_streams`
under `runtime` and the SHA-256 mapping from `artifact_bytes` under `artifacts`:
192 runtime modules and 58 artifact inputs. A flat 900-file source snapshot
binds the build and test inputs. Postflight confirms identical current and
snapshot runtime/artifact inputs; only this evidence page changed in the
checkout after snapshotting, and only the plugin/checksum build outputs changed
in the snapshot. Root release files remain unchanged. CMG remains pinned to
`8b3c50507165d834199c03520f9cacc933f97778`.

One private universal Mac build passes in 141.79 seconds on Rust 1.85.1,
locked release optimization 3, fat LTO, one codegen unit and unwind, with Rust
path remapping. Identity `m1-fix-src-353cc6381360` denotes the source identity
above. Both x86-64 and arm64 slices pass binary audit; actual ABI/load, worker
and injected-panic containment checks pass on this arm64 host. The plugin is
`2e4e07576bf9e421e1a2758937460e2ca3eb45961484285d9a95059952c910c9`;
the loader harness is
`5285ce62b666119c6271cad54e555ebb2a930d102575e5eb29d0cb118440703d`.
This does not establish execution on an x86-64 Mac, Linux or Windows, or
installed-package admission.

All six unchanged Stata 19 MP fixtures pass in 8.06 seconds, each with process
and Stata exit zero: `ols_cmgschur`, `ols_cmgschur_sampled`,
`ols_cmgschur_nonconvergence`, `ols_cmg_raw_residual_oracle`,
`ols_near_within_cmg_oracle` and `estimate_ols_auto_cmg_route`. In particular,
the original near-within fixture completes all unit/analytic/frequency/
probability-weight, fast/low-memory, one/four-worker, Auto/CMG and saved-FE
checks, including independent dummy-regression VCE and exact-scale recovery
assertions. The earlier parent and candidate scientific failures remain
failures; they are not reclassified or rerun here.
Independent terminal review recomputes the source/plugin bindings and confirms
the six raw success sentinels, all 48 near-within fits, unchanged fixture bytes,
source-integration counts and the declared scientific/resource scope.

The fixed post-repair Break set passes four of four invocations: two each at
workers two and four. No optional pre-repair diagnostic was used, and no
invocation was retried. Each PTY records two native cancellation markers,
interruption, rollback, exact ordinary-fit reset and successful follow-up
overlap fits. Each trace contains exactly two complete work-ready records with
task 0, issued 373696, charged 64 and ordinal 1. All 112 retained trace lines
are intact; there are no trace I/O, capture or cleanup errors. The interactive
processes are deliberately terminated after the success sentinel, explaining
their recorded return code -15; all four exact owned PIDs are subsequently
absent. Independent raw review confirms these results, without inferring the
cause of the old missing trace or concurrent numerical work at this gate.

The summaries above are the permanent evidence; these hashes identify the
completed captures and the flat source binding, not durable ignored files:

| Evidence | SHA-256 |
| --- | --- |
| Independent action/null-space diagnosis | `c5fd526f56688a6f27ce84b88546fdac2419f5833291fb1ae504d0bf3a42e95c` |
| Actual prepared-fit diagnostic | `8afae1503807ab9aaa54e2ca643e784ef75cf9c7f61524fd66aacdf9f8ebdcfd` |
| Focused checks, including failed old budget expectation and correction | `4954869efba69dc67f275c030b6f0859879a9342ad2cc0fdabda7e16e455289c` |
| Source integration | `b02415cc00d08cfbc5d66bfe68361c52c991b3c73a784349636f4852f38cc691` |
| Flat source snapshot | `0f300d2170794b12abfa38b2d83493ceaa61c00389b4799540aebbf8ad542d6d` |
| Native build/audit/ABI | `056c5e665fd8b768fa5d05258cbbc3c29355c3e7e523968f1303d1781da81038` |
| Six-fixture Stata summary | `cc547c83a252c527e37edcb0c807f9b2af44cba913f2931a17d62b487f0cb01c` |
| Two-worker Break metadata, invocation 1 | `45852d35e88cc97d2612803e0822134295f6f2be15e251e9ac6e64b86401e49a` |
| Two-worker Break metadata, invocation 2 | `e60d4d169937283986214487a90ebfce8c3e75fe20a863a8d99684cde8dda906` |
| Four-worker Break metadata, invocation 1 | `cb1e657de4e00288f08b32c7c65ff21238fc53b5a9378bdd336d8110310438c7` |
| Four-worker Break metadata, invocation 2 | `0c25e3d6754be68458198dbf676f075c2e935aebeb4f48d21b4a1e592a741194` |
| Combined terminal source/capture verification | `baef3900ff82092cf457cf4043f18f4a14341c38f16c567e35b1897e69e2cceb` |

Many-FE remains enabled and two-FE disabled. The bounded correctness and trace
repair is complete; M1 performance acceptance remains unassessed, and M2 is
not started. No release artifact, qualification protocol or tolerance changed.

## Compact validation suite, 2026-10-18

Johannes authorized replacing the mandatory A/B/C harness with one fixed-candidate
qualification round, cleaning up obsolete files, then stopping. The maintained
entrypoint is `bench/qualify.py` (`plan`, `run`, `report`) with parallel SCC array
support and focused Mac host observation. It keeps all 42 routing cells,
independent numerical witnesses, exact unaffected outputs and repeats, typed
route/resource checks, original/incumbent protection and hidden/public equality.
Supported explicit solvers and PPML remain covered by the mapped feature suite.
Historical campaigns and their failures remain history, not executable prerequisites.
Unresolved failures require discussion before abandonment or advancement.

The replacement passes 148 benchmark-tool tests and independent high-effort
architecture review. `./scripts/run_tests.sh --source-only` passes, including
312 Python source tests, Rust/Clippy, allocator, portable and bridge checks.
Production identity is unchanged against `281c0a6c`: 192 runtime modules and
58 artifact inputs. Installed-package admission and the Linux mock-plugin audit
are not established by this Mac source-only check; prior unchanged-runtime
scientific/transaction evidence remains valid.

A bounded real-binary tooling smoke passed 60 calls across no-FE and four-FE
fixtures at one/four workers in 14.63 seconds. It compared original/incumbent,
hidden many-FE and provisional public many-FE artifacts, checked independent
numerical truth, exact repeats and hidden/public equivalence, and successfully
aggregated the compact outputs. The first launch stopped before native calls
because the sandbox denied process observation; resuming with that observation
available passed. This is a tooling result, not a new performance qualification.
The full 42-cell configuration also passes preparation without native calls.

Obsolete coordinator code, protocol-only tests, copied source trees, archives,
logs and repeated captures were removed. Local cleanup removed 20,798,961,766
bytes and retained an 889,130,544-byte capsule for the unfinished many-FE workstream:
three verified Mac artifacts, the current 21 inputs, independent references and
compact prior science. The new task's temporary checks are removed after recording
these results. No SCC campaign, candidate qualification, default activation or
two-FE work was launched. Both public defaults remain disabled; the existing
many-FE candidate and valid completed science are retained. Work stops here.

Check-capture identities: source integration `f45dd7e3f19f92a1177b8d220ad79dcc6af039ae61838550719c386b84ae8c50`;
native tooling-smoke summary `0b9e682b363138c3429f79265c7638d300d995ad676837626aab12b6a7b2cf16`. These identify the deleted captures;
the summaries above are the retained evidence.

### Resumed many-FE qualification preparation

Johannes subsequently authorized execution with this compact suite. The existing
candidate and 21 current inputs are reused; two-FE remains on hold. Preflight
found that execution/checking code was not bound by the manifest and that the
run budget did not cover all preparation and postprocessing. The runner now binds
17 maintained modules, checks file aliases and metadata during execution, and
enforces the whole-run deadline through final output writes. Native cleanup
blocks repeated signals until its child is killed and reaped. Passive observation
retains ordinary CPU, RSS and activity summaries without changing noise policy.

All 161 benchmark-tool tests pass, including deadline interruption, owned dummy
child/descendant cleanup with an active observer thread, source drift and alias
retargeting. Independent high-effort source review passes. The task-local
environment initially lacked report/test dependencies; after installing them,
the final complete suite passed in 9.631 seconds. Shell syntax and `git diff --check`
pass. The test capture hash is
`47296e567bc8ed78243172ec375a85b6f4693e0ae646cf93134d37a42fa2a41b`.
Native production and artifact inputs are unchanged, so the valid source,
scientific and transaction evidence above is reused.

Before any qualification observations, the fixed design is 24 mirrored blocks
on Mac and two blocks on each of 12 distinct SCC hosts, with three workload
shards per host. Both retain all 42 cells, one warmup per distinct execution arm,
120 seconds per native call, the max(5%, 1 ms) allowance and one-sided alpha
1/40 bounds. The whole-run limits are 14,400 seconds on Mac and 3,600 seconds
per SCC shard. Original and incumbent share identical execution calls; the
hidden/public comparison remains protected. This schedules 6,174 Mac and 7,560
Linux calls. SCC hosts are fixed prospectively by the first twelve eligible,
idle hosts in a lexicographic census, with queue selection unrestricted;
physical hosts remain the Linux uncertainty units. There is no automatic
replacement or increase in the sample budget.

The private Linux candidate was built from `a3f190596628` with Rust 1.85.1,
GCC 12.2.0 and the locked release profile. Its ELF audit, GLIBC 2.28 ceiling,
actual ABI/load/panic containment and both numeric input/output guards pass.
The binary SHA-256 is
`7fae1f99a4a7697540abc451a6203900ca82e2f4fb61d8ce850739178ec4954d`.
Setup jobs `8167455` and `8167462` failed before compilation because of the
wrapper's working directory and archive ownership/mode comparison respectively.
Job `8167540` completed the native checks in its 127-second allocation, then
exited 1 on a syntax error in the summary writer. These jobs remain failed.
The summary was recovered from the actual logs and verified binaries without
rebuilding or repeating native calls; its hash is
`4b3411f0f9209018b94fff74501c2799ea14fa8cc1f24d5e3f102f42bfccdd5d`.
The compute-node compact-suite smoke then passed on `scc-wo4`: job `8167554`,
task 1, reserved all 28 cores, completed 60 native calls across four cells in
59.785 seconds, and passed independent science, exact repeats, hidden/public
equivalence and host isolation. Scheduler failure and exit status were both
zero; the compact report has no errors and status `smoke_complete`. Its summary
hash is `9be40acfe4c5dc7a2d06b527318a6d8e8a01dc5fd15b28f9d453c9db95fa4b73`.
This is a tooling result, not performance qualification.

The full plans bind the same 78 input/reference files and 17 reviewed execution
modules, with separately verified Mac and Linux binaries. Plan hashes are
`f055a108c0dc50898b65f5c2d92c2f31f15bc95c43fb93f2f422e8078ecb8b7b`
(Mac) and `11d8abd07fa351730b8f38b5586882ce65e8c4a047f4a8fde79cdd7cb9749c81`
(Linux). The prospective twelve-host assignment hash is
`e0793a1bb771d25d2db980efb8558a0a547d1cc498954f7ad9b25739e10165ac`.
No qualification result follows from planning or smoke success.

### Compact qualification outcome, 2026-10-18

The approved round ran once on each platform. Mac stopped operationally after
20.903 seconds, within its fixed budget, when the observer classified the
long-lived Codex app-server as competing computation. The recorded trigger was
0.050000000001091394 CPU seconds over 5.000162208 wall seconds, with process state
`S`. This meets the frozen executable-name/CPU-delta rule, but the retained
evidence does not establish deliberate competing work or prove that its activity
was harmless. The independent review confirmed the live process identity and
the classifier's mechanical result. The rule cannot reliably distinguish this
background app-server activity from substantive agent work.

Mac retained 83 successful calls on `dense-t1`: three warmups, 13 complete
six-call blocks, and two calls in a partial block. All recorded scientific
fingerprints agree. No workload cell completed, no four-worker or affected
many-FE cell was reached, and this is not a Mac performance result. The failure
occurred before the next native launch; completed calls were reaped and observer
shutdown completed. Individual native PID/birth identities and full process
snapshots were not retained, limiting independent descendant and activity audits.
The maintained report returns `needs_discussion` and accepts zero calls from
the blocked shard; that report count does not erase the 83 actual calls.
The partially timed cell cannot be resumed automatically. No call was replayed,
no observations were removed, and the observer policy was not changed afterward.

SCC arrays `8167584`–`8167595` completed all 36 submitted tasks on their twelve
distinct assigned hosts. Every task has scheduler failure and exit status zero,
the declared 28-slot reservation, and structurally complete accounting. All
applications report completion with 7,560 calls, no recorded failures, and
passing budget and host-observation status. Application runtimes range from
134.529 to 180.076 seconds, within the 3,600-second shard limit; scheduler wall
times range from 135 to 181 seconds. All 108 collected application and raw
host-observation records match their remote hashes. The maintained Linux report
ran once and passes all 126 protected comparisons, with no report errors. Every
cell has twelve physical-host units, each aggregating its two mirrored blocks;
the bounds remain conditional on independent unit scores with a common median
and do not establish simultaneous coverage. The nearest upper excess bound is
still 0.888 ms below its allowance. This is Linux evidence, not a Mac speed claim.
Independent recomputation from actual call times reproduces all 126 passes.
The operational review validates 72 raw reservation snapshots and 144 scheduler
censuses; 1,153 recorded process samples contain no classifier hits. Maximum
process and scheduler sampling gaps are 6.199 and 61.163 seconds respectively,
within the declared bounds. This remains sampled evidence: full process rows
were not retained, so classification cannot be independently replayed from every
raw process identity, and unsampled activity is not ruled out.

Independent Linux scientific review also passes: all 1,512 retained first-call
heads and output arrays reproduce their complete scientific fingerprints, with
exact repeats across hosts for each case/worker/arm group. It performs 1,152
checks against frozen categorical references, 288 imported-reference checks,
72 inactive-column algebraic checks without new solves, and 1,449 cross-worker
and cross-host comparisons. The largest observed cross-worker/host output
difference is 4.44e-16. All 240 eligible raw resource traces pass. The 6,048 timed
calls retain matching fingerprints, but their full arrays and logs were deleted
by the unchanged runner; the review does not independently replay unavailable
timed outputs. This completes the Linux review without a new native call,
new oracle solve or repeated report invocation.

The campaign monitor was deleted after terminal accounting was available for
all tasks. Both public defaults remain disabled and two-FE remains on hold.
The current artifacts, inputs and observations remain available for the
unfinished workstream; no replacement round or increased sample was launched.
The proposed Mac repair is prospective: distinguish the long-lived app-server
from substantive local tool processes, continue recording its activity, and
retain enough process identity and call-boundary evidence to attribute a trigger.
It has not been implemented or applied to reinterpret this failed attempt.
Johannes's direction is required before deciding how to handle the partially
timed Mac cell or authorize further measurements.

The [compact result](../bench/results/incremental-many-20261018-compact.json)
retains all 126 Linux bounds and their twelve host-level excess scores, actual
artifact/source identities, scheduler totals and the Mac failure summary.
The maintained Linux summary hash is
`eae7e603bb51a60b67cf875f8e23f8ed9939d2ad548fa5311890633b412f8db9`;
the independent Mac review hash is
`dcbfbbb9c2dcda8e882262c7b1ee2cb25e4f985a1dd7f50a109ee0aa9c607b9c`.
The independent Linux review hash is
`3fdd58a9d65c9f2f72de3b6f98aa416b4fba9ea2955247bf12a29161e2fad95f`.
Only documentation and this compact result changed after collection. JSON
consistency, local documentation references and `git diff --check` pass;
unchanged-runtime test evidence above is reused.

### Mac observer repair and untouched continuation

Johannes then directed that Mac qualification work without a completely quiet
desktop. The revised Mac observer recognizes the OS-confirmed `codex app-server`
role as ordinary activity and retains its identity, parsed role, CPU/RSS metrics
and argument hash. It does not exempt `codex exec`, active tool children, builds,
tests or estimators. A live preflight identified macOS's capitalized `Python`
executable name; exact Python-name matching now handles that case while keeping
module/repository-script classification narrow. Unknown desktop activity remains
recorded noise; SCC reservation and competing-compute thresholds are unchanged. The earlier failed
attempt is not reclassified. Native calls now record their monotonic boundaries
so subsequent activity evidence can be placed relative to the calls.

Independent protocol review supports one prospective continuation using only
untouched slots: `dense-t1` keeps original indices 14–23 and the other 41 cells
keep all 24 original indices. All 14 started slots from the failed segment,
including its partial slot, stay outside acceptance; their observations remain
in the failure evidence. The original seed, balanced orders, candidate, inputs,
references, science and max(5%, 1 ms) rule are unchanged. With ten blocks the
alpha 1/40 upper bound is the ninth ordered score (binomial tail 11/1024),
so the first cell has less precision. There is one final assessment, no interim
acceptance or automatic extension. Independence/common-median assumptions remain
conditional; randomization and mirroring do not prove away session effects.

The continuation requires 5,964 timed calls and 126 fresh-session warmups.
Together with the 83 historical calls, that is 6,173 calls against the original
6,174-call cap. The three repeated first-cell warmups are explicit conditioning
for the resumed segment, not additional timing samples. Deducting the old
20.903 seconds and rounding down leaves a 14,379-second execution budget;
the 120-second native bound remains unchanged. The single flat amendment binds
the immediate old plan/result and refreshed checker sources, rejects consumed
slots and recursive continuations, and preserves the old files. It adds no
native artifacts or generated inputs. The valid completed Linux report and
independent review remain the Linux evidence. Both defaults remain disabled
and two-FE remains held.

All 171 benchmark-tool tests pass in 9.475 seconds, including the recorded
app-server threshold regression, active tool-child detection, immutable failed
evidence, consumed-slot rejection, original call/time caps, unfavorable-old-timing
invariance, source/input drift and continuation report tampering. The test capture
hash is `5aa24f69ead39a43b3a3acc5ff01b4fedeb8fd6be2a593f1a30e970e7ff4aef7`;
the result above is the retained test summary. Native runtime and artifact inputs
are unchanged, so their valid prior scientific/transaction evidence is reused.

The final prospective plan SHA-256 is
`6462878236ca45381b7e9e3452f03d5dec31b6d01c7f1b721360f62acc6a94b6`.
Before any native launch, the unused preflight plan
`aab1d066975efb6f686877de99e98f3b01590a42b894bdc937f4720a2ae38946`
was rebound solely to the capitalized-Python checker fix. This changed no
inputs, artifacts, sampling slots or budgets. The original failed Mac plan,
results and the completed Linux evidence remain unchanged.
Independent final source/plan review passes and rechecks all 82 input/artifact
files (888,103,391 bytes), all 17 current execution modules, the exact 994 untouched
blocks and cumulative budgets. Its hash is
`91e0ab2af6b80454ada7baaad5c3963cd433d623b3f6bf29e3406c624168a86e`.
Lead plan checks and new documentation-link checks also pass.

### Mac continuation outcome, 2026-10-18

The single continuation exited with status 1 after 1,631.673 seconds, within its
14,379-second budget, when the frozen observer detected a `node_repl` process.
It completed 26 cells and reached `sixteen-fe-t1`, retaining 81 warmups and 3,774
timed calls: 3,855 completed calls in total, with 629 complete mirrored blocks.
The next block, index 19 in that partial cell, was opened but contains no calls.
Together with the earlier 83 calls, the two failed segments consumed 3,938 calls
and 1,652.576 seconds. The maintained continuation report ran once and returns
`needs_discussion`, with no accepted comparisons or calls. That acceptance count
does not erase the actual observations. The monitor was deleted before terminal
validation. Both public defaults remain disabled, Linux's completed qualification
remains unchanged, and two-FE stays held. No second continuation was launched.

The revised app-server recognition worked: those services remained recorded
ordinary activity. The new trigger was PID 27393, born at 09:38:37 on October 18,
with parent PID 37146. Its CPU clock rose from 0.03 to 0.10 seconds across a
5.004377-second sample interval; both endpoint states were sleeping. The later
identity check found the same process running the app's `cua_node/bin/node_repl`
executable, still sleeping at 0.10 seconds of total CPU. The retained command hash
is `471edcb4491f55a20b811896ecea829f23d1ff9a05b1dc70b8ccbedf119b4c4b`.
This meets the frozen 0.05-second rule for a known tool executable. It establishes
sampled activity of that runtime, but does not identify its workload, distinguish
startup from tool execution, or quantify its effect on the measurements.

There is one classifier hit in 328 retained process samples, with a maximum
sample-start gap of 5.165 seconds. The hit's interval overlaps the outer subprocess
intervals of all six calls in `sixteen-fe-t1` block 18; exact overlap with the
internal native stopwatch is not recorded. The observer recorded its error during the final call.
That call completed and was reaped before the next block's pre-call host check
raised. Observer shutdown completed without a budget overrun. A current census
found no qualification runner or native harness. Individual native PID/birth
identities were not retained, so this is not an exhaustive historical descendant
audit. Lead terminal validation rehashed all 82 input/artifact files
(888,103,391 bytes) and 17 execution modules, checked the immediate prior-plan
and failed-result bindings, retained reference hashes, declared call order,
nonoverlapping call boundaries and every recorded repeat fingerprint. No source,
input, artifact, threshold or observation was changed to reinterpret the failure.

Independent failure review passes and reproduces the bindings and operational
diagnosis. It checks all 81 retained first-call output snapshots, 57 comparisons
against stored routing-oracle outputs, 18 stored local-reference comparisons,
39 cross-worker pairs, 14 eligible resource traces and the three surviving full
column fingerprints. All 3,774 timed repeats retain their first-call fingerprints.
Only thirteen families reached both worker counts; fifteen cells were unstarted.
The inactive-column oracle was not rerun: its six retained certificates and
cross-worker outputs were checked. Deleted timed arrays cannot be independently
replayed. These checks support the observed scientific prefix, not complete Mac
qualification. The independent review SHA-256 is
`42065aa20dacbbd9cbc8e52eeefa8e943ee28547e95a8404b4fa65e284f48e22`;
the terminal result SHA-256 is
`beb24768f054db752bd8f7f32861ac20a03c09ae40c591ba422c21880863b9f1`,
and the maintained summary SHA-256 is
`4a85b8043004c14e829b0f79be8209d1c3d7a4775479dd5eff2ade5f653dc610`.

The bounded diagnosis is that the remaining process-name/low-CPU rule still
cannot distinguish incidental tool-runtime activity from substantive competing
work, yet its single hit aborts the whole run. A future protocol would need
positive workload evidence and a prospectively reviewed rule for affected whole
blocks; exempting every `node_repl` process would hide real tool work. This does
not itself authorize a policy change, retrospective exclusion, another continuation
or new sampling. The later direction recorded below supersedes that stop condition.
The [compact result](../bench/results/incremental-many-20261018-compact.json)
retains this failure summary alongside the earlier Mac failure and valid Linux
result. Current raw captures remain needed for the unresolved workstream; their
hashes identify the reviewed inputs rather than promising permanent retention.
This terminal update changes only documentation and compact result metadata.
JSON consistency, unchanged historical-result checks, added documentation
references and `git diff --check` pass; valid executable/runtime checks are reused.

### Practical Mac recovery, 2026-10-18

Johannes subsequently directed practical Mac qualification with retries of specific
tests affected by interference and identified Linux as the more important test.
The completed Linux qualification remains the primary performance evidence and is
reused unchanged. Mac keeps its own scientific and performance comparisons; no
Linux result establishes Mac performance. Both failed Mac segments remain failed.

The recovery retains the 26 completed cells whose full measurement spans have
clear, sealed evidence under the previous stricter observer. These supply 610
mirrored blocks, including the ten remaining original blocks for `dense-t1`.
The entire partial `sixteen-fe-t1` attempt is excluded from acceptance. That cell
and the fifteen unstarted cells receive their original 24-block schedule, fixed
warmups, artifacts, inputs, seed and scientific tolerances. The flat plan binds
the immediate failed plan, results, observations, independent scientific review
and retained snapshots, with separate prior and current execution-source hashes.
It does not reconstruct older confirmation or continuation chains.

The new Mac screen records smaller known-tool activity as noise and flags at
least one CPU-second within a valid interval of at most 7.5 seconds, or a known
compute process runnable at 20% CPU or more. Ordinary desktop activity remains
recorded noise. Each cell receives a synchronous tail observation; a flag
overlapping its whole measurement span discards that entire attempt. One retry
per affected cell is permitted after two clear sampled intervals within 120
seconds, with at most four retried cells across the run. Persistent interference,
an affected replacement or exhausted caps stops the run while retaining completed
cells. Scientific disagreement, native failure, observation faults and input drift
cannot be treated as interference retries. Retry science must agree exactly at
the same worker count, including across discarded and accepted attempts.

The sixteen fresh cells require 2,352 native calls before any retry. The new-call
ceiling is 2,940, and the cumulative ceiling is 6,878 including all 3,938 historical
calls. The remaining 12,747-second deadline preserves the original cumulative
four-hour budget and includes quiet waits, preparation, checks, shutdown and
final writes. Acceptance is assessed once, with the existing max(5%, 1 ms)
allowance and conditional one-sided .025 bounds. A regression or inconclusive
bound never requests more sampling. The first externally eligible attempt is
used, regardless of which attempt was faster.

This is a practical sampled interference screen. It can miss brief work, and
smaller activity may still affect timings. Tail sampling changes CPU-delta
intervals and may conservatively attribute post-solve activity to the cell;
macOS CPU percentages can reflect earlier activity. Recorded classifier hits
and activity summaries do not permit independent reclassification of every
process. Block independence and a common median remain assumptions, without a
simultaneous-coverage claim. Retained first-call arrays and repeat fingerprints
support review; deleted timed arrays cannot be independently replayed.

The maintained recovery path passes 182 benchmark-tool tests in 10.725 seconds,
including local retry success, persistent interference, quiet-wait evidence,
science-failure precedence, exact science across retries, drift and report
tampering guards. Independent source review and exact plan review pass. Plan
review and lead validation independently verify the 242 directly bound files
(1,287,049,146 bytes): 82 original input/artifact files, 156 retained snapshot
files and four immediate evidence references, plus 17 execution modules.
Production identity is unchanged from `12395be2`: all 192 runtime modules and
58 artifact inputs match, so valid runtime/native/transaction evidence is reused.
No new native measurement occurred during this preparation.

The recovery plan SHA-256 is
`83473bb969671fc4390585566fd94d14ce3b7dcd8f3bc267c7f7ad5f94d8f621`.
Independent source review is
`dda8703d392e61da4351498b8a80787b6e3a1e1cd4aeff8758b2b5e3564f0530`,
and independent plan review is
`f174075b67d29d05e59cb305655694b8908a2cdf70de98275a166b8dcfac340b`.
The [compact result](../bench/results/incremental-many-20261018-compact.json)
retains the selected cells, original indices, policies, budgets, source vectors,
test hash and review hashes. Its historical Linux and Mac outcome fields remain
unchanged. The following terminal result completes this reviewed recovery.

### Mac recovery result and many-FE activation

The recovery completed in 828.192790 seconds within its 12,747-second budget.
All sixteen fresh cells passed with 2,352 native calls, no retry, no discarded
attempt and no scientific or operational failure. Together with the 26 retained
cells, the applicable evidence contains 994 mirrored blocks and 6,090 calls.
All historical calls remain accounted for: cumulative actual use is 6,290 calls
and 2,480.769208 seconds, below the declared caps. The failed first segment and
partial sixteen-FE attempt remain excluded from acceptance.

The maintained report was run once and passes all 126 protected Mac comparisons.
The nearest upper allowance excess is -0.000798512 seconds (about 0.80 ms inside
the permitted margin). Dense-t1 retains ten blocks and upper order-statistic rank
nine; other cells have 24 blocks and rank eighteen. Ordinary desktop activity was
recorded across 183 process samples, with a maximum gap of 5.224443 seconds and no
observer errors or substantial competing-compute hits. Terminal process inspection
found no owned runner or native harness. This is a sampled check, not proof of an
otherwise idle machine or exhaustive historical descendant accounting.

Independent review reconstructed every paired bound and matched the report
exactly. It checked 126 retained first-call heads/arrays, 96 stored independent
routing-oracle comparisons, 24 local-reference comparisons, 63 cross-worker
pairs, 20 resource traces and all 6,090 accepted call fingerprints, solver rows
and execution orders. It rehashed the 242 flat bound files, 17 execution modules
and 96 fresh snapshot files. The unchanged Linux report and independent review
remain passed: 36 tasks on twelve distinct assigned isolated hosts, 7,560 calls
and 126 protected comparisons. Linux remains the primary performance evidence;
no cross-platform timing or bitwise claim is made.

The compact result retains every accepted qualification time and execution order,
paired summaries, source identities and review counts. It records the actual raw
result hash `068ca9dd422e4b0dd3d2a3b51e5f1ff51898ac2772694a4576f1148a425952a9`,
maintained report hash
`c5e3dbaf08ec2432bfed8b30918131013452064293b165279f0c74ceda08e621`
and independent review hash
`e3b8065c291d4eeb34ebf4704f5660ee2560a675980eea6a3c25dcd3d57ed447`.
The qualification timer is the harness `NATIVE_SECONDS` value stored as
`call.seconds`; `native_s` is retained separately as diagnostic timing. The
compact export preserves the exact timer used by the qualification report.
The first-call review does not recreate deleted timed arrays. Inactive-column
witnesses and the historical generator limitations remain as previously recorded.
Order-statistic coverage remains conditional on independent scores with a common
median, without simultaneous coverage across comparisons.

Activation changes only `DEFAULT_MANY` to true; `DEFAULT_TWO` remains false.
All 192 runtime production modules and 58 artifact inputs match the qualified
private public-many revision `a3f19059662836645c2ac37db9aca456d220f904`.
Thus the actual native build/ABI/privacy, 96-call science, seven mapped Stata
fixtures and four Break checks recorded under provisional activation remain
applicable. The four activation-aware Rust/Stata fixtures are restored from the
corrected, reviewed revision `8941355a`, retaining every numerical tolerance and
the certified MAP fallback. MAP-specific oracle fixtures explicitly select
diagonal preconditioning; public Auto has separate exact hidden-policy and QR
checks. No new candidate or performance sampling is introduced.

The default integration command was attempted and failed its installed-package
runtime binding: the release plugin remains unchanged while this branch selects
the qualified many-FE behavior. That failure is retained. Source integration and
the final source-bound handoff are recorded separately. The first source run
exposed three historical MAP-route assertions before restoration of the four
reviewed consumers; its failure is also retained. The final serial
`./scripts/run_tests.sh --source-only` passes in 141.61 seconds: 312 Python tests,
1,946 Rust test executions including isolated child/audit processes, seven
portable Python checks, Clippy, FFI/allocator, complexity, bridge and numeric
guards. Its log SHA-256 is
`5f8b74406bca01dc41865d7a0d053eff2608c060b7e8f666fa556a58b02a4e0b`.
The Linux linker audit is skipped on Mac, and this work does not admit or publish
the installed release package.

Final independent activation review passes, including exact restoration of the
four test consumers, final source-suite evidence and reconstruction of all 126
Mac bounds solely from the compact record's qualification times. Review SHA-256:
`8ee441268beca010f61ef45f4ae1c5835152922289c314cd19dbc5f2334dce0a`.
Many-FE is enabled only on `codex/auto-routing`; two-FE remains disabled and held.
The committed compact results and hashes are the retained evidence. Temporary
task artifacts and raw captures are removed after committing these summaries.

### Separate two-FE qualification preparation, 2026-10-18

After accepting many-FE at `d5b06cf369be`, Johannes authorized two-FE work.
The fixed candidate changes only `DEFAULT_TWO` from false to true in private
builds; accepted many-FE behavior stays enabled. The checkout still has two-FE
disabled. Qualification protects both original `f35f3a4d` Auto and accepted
many-FE Auto, plus hidden/public equivalence, across 42 cells. Twenty-six cells
exercise two-FE scope, ten protect many-FE fallback and six protect no-FE,
one-FE and slopes behavior. The retained Linux incumbent at `a3f190596628`
has independently verified production-token and artifact-input equality to
`d5b06cf369be` (192 modules and 58 inputs).

The [compact two-FE evidence](../bench/results/incremental-two-20261018-compact.json)
contains the frozen plans, artifact/input/source identities, placement and
independent review summaries. Twenty-one newly generated inputs use seed
2026101821; their body hashes are disjoint from all 63 historical two-FE panel
identities. Sixteen independent categorical certificates, four imported
references and the inactive-column analytic witness remain scientific gates.
The generators are the previously audited rebuilds, not restored historical
binaries. Old panel bodies and old harnesses were not restored or rerun.

The fresh Mac plan applies bounded cell recovery prospectively from its first
call. Recorded substantial competing compute can invalidate an entire cell
attempt, independently of timing scores. At most one retry per cell and four
retried cells overall are allowed, after two clear sampled intervals within
120 seconds. Scientific/native failures, timeouts and observer faults retain
their failure semantics. There is no report-triggered resampling or budget
reset. All attempts and discarded observations remain accounted for.

Both platforms use four distinct binary/request arms, timing seed 2026101822,
one warmup per arm, max(5%, 1 ms) protection and one-sided alpha 1/40 bounds.
Mac schedules 24 mirrored blocks, 8,232 baseline calls and at most 9,016 calls
including interference retries within 14,400 seconds. Linux schedules two
blocks on each of 12 distinct assigned physical hosts, split into three shards:
36 tasks and 10,080 calls, with a 3,600-second application and 4,200-second
scheduler limit per task. Every native call retains its 120-second limit.
Each platform has 126 protected comparisons. These are conditional
order-statistic bounds at the block/physical-host unit, not simultaneous
coverage or proof that sampled host observations detect all interference.

All 189 benchmark-tool tests pass in 11.264 seconds; focused recovery tests
include four distinct arms and complete started/accepted/discarded call
accounting. Independent source and exact Mac/Linux plan reviews pass. Each
plan's 82 bound files and 17 execution modules were independently rehashed;
the transferred Linux files also pass byte/stat/alias closure checks. Runtime
production and artifact inputs in the checkout are unchanged, so valid runtime
and transaction evidence is reused. Shell syntax and `git diff --check` pass.

Private Mac original/incumbent/candidate builds pass universal build, artifact
audit and ABI/load checks. Two setup attempts were cancelled and retained:
one inherited the wrong working directory, and one reused a mixed Cargo cache.
All three accepted binaries were then rebuilt with separate caches. Initial
tool tests lacked psutil, matplotlib and reportlab in the new task environment;
the final complete suite passes after installing those dependencies. Linux
build job `8168542` passes on `scc-ei3` (failed=0/exit=0, 148 seconds), including
glibc 2.28 audit, ABI/panic and numeric guards. Maintained-runner smoke job
`8168558.1` passes 80 calls on four cells at `scc-zj4`, with clean 28-core
allocation evidence and scheduler accounting (failed=0/exit=0, 26 seconds).
The smoke is tooling evidence, not performance qualification.

Full Linux qualification was submitted once as arrays `8168720`–`8168731`,
36 tasks on the twelve frozen hosts. The submission wrapper first stopped on
the existing smoke log directory before issuing any `qsub` command. Its failure
was retained, absence of qualification jobs/output verified, and only that
directory check corrected. This did not repeat any measured work.

At the preparation milestone, actual full qualification results, allocation
isolation and independent raw scientific/timing/operational review remained
required before two-FE activation. The completed outcome follows.
Successful timed arrays/logs are deleted by the unchanged producer; retained
first-call arrays and all repeat fingerprints support review, not exhaustive
replay of unavailable arrays. No release package or main-branch change is made.

### Two-FE qualification result: incumbent retained

The fixed candidate completed all planned work without qualification retries,
discarded attempts, native failures or scientific failures. Mac completed all
8,232 calls in 3,103.217 seconds, within its 14,400-second budget. Linux completed
10,080 calls in 36 tasks on the twelve exact assigned physical hosts. Every task
has complete scheduler accounting with failed=0 and exit=0, its application
success signal and complete result; application times range from 177.060 to
241.853 seconds, below the 3,600-second limit. Recorded reservations, scheduler
censuses and process observations support the prescribed host isolation.

The maintained report ran once per platform. Mac passes all 126 comparisons.
Linux passes 124 and fails these two comparisons of the public candidate on
`high-degree-weak-t4`:

| Protected reference | Median candidate/reference ratio | Lower excess bound | Median excess | Upper excess bound |
| --- | ---: | ---: | ---: | ---: |
| Accepted many-FE Auto | 1.080424 | 6.936 ms | 12.137 ms | 13.509 ms |
| Original Auto | 1.082442 | 10.391 ms | 11.778 ms | 14.016 ms |

Excess subtracts the existing max(5%, 1 ms) allowance from paired complete-solve
time. Both lower bounds are positive, so these are regressions under the frozen
rule, not inconclusive comparisons. Linux uses one median score per physical
host; Mac uses mirrored blocks. One-sided .025 order-statistic coverage remains
conditional on independent scores with a common median and is not simultaneous
coverage across comparisons. Mac success does not override the Linux failure.
No observations were removed or added after viewing the reports.

Independent actual-evidence reviews reconstruct all 252 platform comparisons
and agree exactly with both maintained summaries. The Mac review checks 168
retained first-call snapshots, 64 exact and 62 scoped artifact comparisons,
84 cross-worker comparisons and all repeat fingerprints. The Linux review
checks 2,016 first-call heads/arrays and reconstructed full-column fingerprints:
1,536 stored categorical witnesses, 384 imported-reference comparisons and
96 inactive analytic checks; 768 exact, 744 scoped, 1,008 cross-worker and
1,848 cross-host comparisons; and 984 resource traces. Its 72 reservations,
168 scheduler censuses and 1,502 process samples pass. All source/input hashes,
file metadata and aliases remain bound. Scientific acceptance passes; the
two Linux performance regressions determine the failed qualification.

Linux audit development initially demanded bitwise equality of derived log/exp
scores across platforms. Seven of 3,024 stored block/reference scores differ
from Mac reconstruction by at most 5.90e-17 seconds. The audit records these
differences and checks an eight-ulp bound at the operation scale; qualification
is independently reconstructed from the unchanged raw timers and unchanged
5%/1 ms allowance. No scientific tolerance, observation or report changed.

The Mac terminal process check found no qualification runner/native harness.
On SCC, successful call records imply direct-child reaping through the maintained
`wait4` path; observer command children are reaped and sampling threads joined,
and every scheduler task completed normally. An optional post-run direct-node
PID census was unavailable because SSH had no trusted node keys. Automatic
approval review rejected adding unverified first-use keys; no trust store changed.
There is no historical per-call PID ledger or independent exhaustive descendant
census, and these records do not claim one. Sampled observations also cannot
rule out unsampled work or reclassify every omitted desktop process.

The [compact two-FE evidence](../bench/results/incremental-two-20261018-compact.json)
retains every qualification timer and execution order, all comparison bounds,
source/input/artifact identities, accounting summaries and review hashes.
Qualification uses `call.seconds` (harness `NATIVE_SECONDS`), not the separate
diagnostic `native_s` field. Temporary full arrays and observer/log captures
are removed after committed summaries; recorded hashes identify reviewed
captures, not permanently available raw files.

All 192 production modules and 58 artifact inputs remain equal to the launch
checkout. `DEFAULT_MANY` remains true and `DEFAULT_TWO` remains false. No
activation, full-battery restart, performance-triggered retry, retune, merge to
main or release-package update was performed. The existing valid source-suite
evidence is reused because executable runtime and artifact inputs are unchanged.

Seven mapped Stata fixtures and two OLS Break checks pass on the exact Mac
candidate, including cancellation during real optional-builder work. An extra
legacy route fixture failed its already-documented obsolete Auto-CMG code-seven
assertion, also known to fail on the preserved parent. Its maintained replacement
retains the geometries and explicit CMG tests, checks six small cases against
independent Stata dummy regressions (coefficient, variance, sample, DoF and
residuals), and labels large-component Auto/CMG agreement as secondary evidence.
The final saved-FE regression is unchanged. Independent review passes, and the
migrated fixture passes once on each of the exact candidate and accepted-incumbent
plugins. This test-only maintenance does not change or repeat qualification.

Capture identities: Mac summary
`c49b9e4d194fb58a98d18cf176176d684f8cf7fc8af867aa16acc29b2c1fe31b`,
Mac independent review
`7316ffa4e09bfd1b0e960c3121d4fb0c2e2fef1bf50779efa638dc979ae486ff`,
Linux summary
`ec6ec63d95a0854c3740a73c4a50202c2cc3a9544e204ccefa8974499fe3be08`,
Linux independent review
`4d0913c71cd73ec42145e88a16dfec69095f7db274bfcb99d580e57d9399f897`,
and independent test-migration review
`a5b53f9dcdfaa77b218d91de940446fc295dcf656ae6db6fc43cf50d28e7fc06`.

### Bounded CMG/RHS/overlap diagnosis and successor proposal

Read-only examination of the completed Linux observations confirms positive
four-worker excess against both protected references on all twelve hosts;
all twelve have negative excess at one worker. Candidate/hidden primary and
final iterations are 22/8, versus 51/47 for original/incumbent, invariant across
hosts and workers. Candidate/hidden install the existing synchronous h16/k1
route. Primary setup charges 794,664,056 of 977,928,192 work units, with zero
final setup charge. The recorded candidate primary-setup host medians are
133.88 ms at one worker and 129.93 ms at four, versus incumbent 6.58/2.00 ms.
These diagnostics support a near-serial construction cost that becomes harder
to amortize as foreground solves parallelize. They do not isolate census,
canonicalization, hierarchy, work-gate, application or overlap costs, and the
phase fields must not be summed as independently owned exclusive spans.

The current work ledger remains deterministic: for observations n, left/right
levels p/q, first submitted RHS width r and physical stride s,
`W0 = 10nr + 10n + (4p+64q)r + (p+3q)s + 8p + 12q + 64r`, with 32W0 total
and 4W0 restart reserve. Compatible Low chunks share the attempt and ledger;
later width does not replenish it. A different final-residual accuracy key may
reuse a hierarchy with its own allowance. Submitted width, physical capacity
and the number of still-active columns are distinct. The existing overlap
experiment additionally copies inputs, leases an executor worker and pays
synchronization/tail costs; its cache/controller/terminal-screen differences
prevent treating it as a pure scheduling comparison. Neither candidate routing
nor these budgets were changed in this diagnosis.

The completed Mac wide-RHS example finishes in 14/13 iterations before h16,
so it provides no evidence of CMG batch-application payback. Historical stage
profiles and the archived small batch trial likewise cannot identify the current
Linux regression cause. The retained diagnosis hash is
`e08410df64a04d5de59cdb5269b681db2cc538a25a93dbd8c4df9abe3b6b3209`;
its compact host summaries and source-bound observations accompany the results.

Proposed next work, requiring Johannes's direction and an independently reviewed
observation-only patch: use the existing analytic paired graph with unit and
unequal weights; cold width one, cold width sixteen, and width sixteen after a
one-column prime; capacity sixteen; unchanged synchronous and h16/k1 overlap
arms; one/four workers; two exact repeats. That is 48 target plus 16 priming
projections, bounded at 120 seconds per action and 1,800 seconds overall with
no adaptive replacement. Record active RHSs at the checkpoint, construction
versus reuse, charged resources, disjoint cost spans, worker lease and tail wait.
Keep analytic science, exact repeats, cross-worker decisions and interruption
contracts. This is a finite diagnostic proposal, not execution authorization,
a predictive routing policy, a larger setup budget or a new promotion gate.

## Incremental Auto implementation checks, 2026-10-17

Before the suite replacement above, both public incremental defaults were disabled. The many-FE
candidate passed discovery and B selection, but its C witness preparation timed
out before native science. The provisional activation was rolled back; see the
failed-confirmation record below. Two-FE remains independently eligible for its
own later qualification, but Johannes's subsequent priority correction requires
finishing the existing many-FE finalist first. The documented harness timeout
has been repaired and independently validated on exposed evidence. A separately
reviewed declaration and fresh 42-cell panel now pass input validation. Fresh
admission, independent witness preparation, native science, scientific
postflight and independent scientific review pass. Its timing freeze then hit
the fixed 1,800-second bound before producing a manifest. This second operational
failure blocks collection and activation; the same finalist was retained pending
Johannes's direction, with no automatic retry or advance to two-FE. His subsequent
compact-suite decision and completed tooling checks are recorded above.

Branch `codex/auto-routing` implements the prospective
[incremental qualification](SPEC.md#incremental-auto-qualification-2026-10-17).
Runtime commit `811769d38adfdd5213270af801ed8b30fe2efc31` adds hidden
many-FE and two-FE selectors, initially with both public defaults disabled. The new
schema protects original and accepted Auto; explicit-solver observations are
diagnostic. Historical campaign outcomes retain their original meaning.

The full `./scripts/run_tests.sh --source-only` passes, including 312 Python
source tests, Rust tests, Clippy, allocator/complexity checks and bridge guards.
It does not admit the installed package. The 294-test routing suite initially
passed 293 tests; its remaining assertion expected 13 frozen tool files instead
of the current 16. After replacing the count with exact dependency names, all
84 producer tests pass. The other 210 routing tests and 19 case/tool tests pass.
Independent high-effort review found no remaining material source/protocol defect.

Private universal Mac baseline/candidate builds pass binary audits and actual
ABI/load/panic-containment tests. The baseline's first unremapped build failed
path-privacy audit; rebuilding with the documented path remaps passes. Ninety-six
actual candidate calls cover no-FE, one-FE, slopes, two-FE Low/saved effects,
three-FE and four-FE models at one/four workers: independent scientific witnesses,
exact fixed-worker repeats and hidden-selector equivalence all pass. Twelve
additional original-artifact calls exactly reproduce uninjected candidate Auto.
Three Stata/MP 19 fixtures pass OLS/PPML transaction rollback and mixed-solver
science; OLS, optional-builder and PPML Break tests pass separately.

| Retained identity | SHA-256 |
| --- | --- |
| Original private Mac plugin (`f35f3a4d`) | `66ce3976dba2cb6b26ec8307040aff298e0525b63387e9c46e8f4ed9132fffb2` |
| Candidate private Mac plugin (`811769d3`) | `6dcb932890bcd4ad4033b151667bb0ee06eb25a25c418cab53fa87ec981bef95` |
| Source-suite capture | `683c8097cf0715a9018532833d818d8ec2ce36cabdb3706730af975552296825` |
| Native selector scientific summary | `922898a96e10a652f445f5e89d7c6644810e665c37d41681e0458b36a4f1568a` |
| Exact original-Auto summary | `bc613c0a2d5e668504b10a2441fe62214f2944c38111dc807589a12d23e07925` |
| Combined implementation-check summary | `790a314b3ea1c9aeb18ef79d70199200033b5b8b2be722947b978d38832fcc23` |

These checks establish implementation correctness within the stated scope, not
an Auto speed improvement. The following SCC discovery result admits separate
Mac B12/C24 qualification; neither public default has been accepted.

## Incremental many-FE discovery, 2026-10-17

The hidden many-FE candidate passes the prospective SCC discovery screen across
all 42 cells, with zero violations against original and accepted Auto under
the per-cell `max(5%, 1 ms)` allowance. Both references use the original
`f35f3a4d` artifact in this first stage, with separate observations. The
equal-weight geometric mean candidate/incumbent ratio over the ten affected
cells is **0.869634**. These are discovery point estimates, not Mac qualification
or confidence bounds treating within-allocation blocks as independent hosts.

| Affected family | Candidate/incumbent, 1 worker | Candidate/incumbent, 4 workers |
| --- | ---: | ---: |
| Small third FE | 0.8785 | 0.8235 |
| Four FE | 0.8692 | 0.8350 |
| Eight FE | 0.8817 | 0.8546 |
| Sixteen FE | 0.9005 | 0.8741 |
| Near-confounded third FE | 0.9205 | 0.8630 |

Explicit solvers identify remaining opportunities: on the small-third and
four-FE cases at four workers, MAP is about 18–19% faster than the candidate.
Those observations neither veto nor rank Auto. All protected arms pass science;
optional frontier arms retain 36 timeouts and 18 iteration-budget exhaustions.
The campaign made 8,118 actual native calls: 8,064 successful and 54 failed.
The 54 unavailable rows copied into timing records refer to those same failures,
not additional attempts. No producer was retried, replaced or reassigned.

Setup job `8161659` and native smoke job `8161732` pass application checks and
SGE accounting (`failed=0`, `exit_status=0`). The Linux builds pass binary and
actual ABI/load/panic checks; the smoke covers 96 native calls. Producers
`8161766`–`8161771` each reserve 28 slots on distinct preassigned hosts
`scc-xa1`–`scc-xa4`, `scc-xb1` and `scc-xb2`. All six complete with both accounting
codes zero, consuming 21,176 allocation seconds within the frozen 43,200-second
total budget. Each cell has three independent host allocations. Finalizer
`8162365` passes all six maintained allocation validators and the full aggregate.

Independent review reproduces the exact aggregate from the frozen evidence and
checks 580 bound files, 126 allocation-local scientific proofs, 2,646 required
solver-signature records and 96 exact outside-family comparisons. The sampled
isolation contract passes with 4,226 process snapshots, 360 scheduler query
groups and maximum process-sample gap 5.00123 seconds. Slots alone are not the
isolation evidence. Successful numerical arrays and transient native logs were
deleted by the frozen producer: review validates the retained independent-oracle
assertions and proofs, rather than recomputing numerical witnesses from deleted
arrays. Sampling does not establish continuous visibility between observations.

The first audit reader (`8162383`) failed after the maintained aggregate replay
passed because its summary expected event kind `sample` instead of `snapshot`.
A separate script changes only that comparison; twelve actual snapshots across
all six allocations pass its focused check. Corrected audit `8162407` completes
with both accounting codes zero. The original failure remains recorded; this
repair ran no solver calls and changed no frozen source or timing observation.

| Retained identity | SHA-256 |
| --- | --- |
| Original private Linux plugin | `70c0f8ebf49b857ded10e3393584c7f608b5b08a0ca0425cda63ba032e6150e3` |
| Candidate private Linux plugin | `be003cc22208919b93c89851347ac2eac2806ae4f2a75cf2cbdc3ebd72e9d364` |
| Linux build summary | `52950d4f858a590c0f20b7ebc156ecb1896f4e1995b7eb1b0dc4738ff939d0db` |
| Linux native smoke summary | `f8702f84ed4de4f67f219542b28af424dabf708b18f95ed8b0e56689307462cd` |
| Frozen plan, logical identity | `40d0ef51e3de15565460ca5586ea8016de7d8fcf2eacb970dcd24d3c5e92a8d5` |
| Full aggregate, logical identity | `6bbf29e69cbe40214da8c999b4709d39d92516c6b6096186a5fa448663762fcb` |
| Independent audit | `84822b507594f001c90f047f50b7ca25c48947a0784cb0906caf5b6f31116979` |
| Independent raw-evidence review | `9920cedb96cb643d14cb6bee9f8eae90815922729850af58f72397799158bcfc` |
| Discovery handoff review | `0f4400b392bef4d247a57377bea88ae607ae7f68ece65f6a4e9574b4e83fb3a2` |

The shortlist contains incumbent and incremental-many. The separately reviewed
historical A/A pilot supports the fixed B12/C24 design with its existing
independence/common-median limitation; its observations are not pooled into the
new panels. This result admitted the Mac selection below; exact hidden/public A/B
equivalence and untouched Mac confirmation remain required. Two-FE qualification
remains a separate stage.

## Mac many-FE selection and provisional activation, 2026-10-17

The fixed Mac B12 panel selects `incremental-many`. All 84 challenger comparisons
against original and incumbent Auto pass the per-cell `max(5%, 1 ms)` criterion;
the incumbent's 42 comparisons also pass. The affected-family geometric mean
candidate/incumbent ratio is **0.878078**, about 12.2% lower runtime. This is
developmental selection, with conditional marginal bounds, not confirmation or
simultaneous/selected-winner coverage.

Fresh B science first passed all 126 protected outcomes in 252 native calls.
The full prerequisite panel made 786 calls: 768 successful, six optional-solver
timeouts and twelve optional iteration-budget failures. Eighteen linked
`not_run` rows are not additional attempts. Independent review checked all
42 proofs and 321 file bindings, 384 exact repeat/signature pairs, 64 exact
outside-family comparisons, 136 resource records and complete CMG/IT worker
groups. The phase-aware oracle wrapper reuses the unchanged bounded mathematics;
its first A-only CLI rejection is retained, without a native or mathematical
rerun. Successful native arrays were deleted by the producer; the review checks
retained assertions, fingerprints and witness certificates, not deleted arrays.

Timing completed all 3,150 planned calls: 126 warmups and 504 six-call mirrored
blocks. The frozen host policy excludes six shared blocks for observed `codex`
process activity, leaving 498 eligible blocks with 10–12 blocks per comparison.
Every observation remains retained; there were no replacements, retries or
continuations. The observer recorded 800 snapshots with maximum gap 5.16738
seconds. The raw record does not uniquely attribute activity to individual
lifecycle polls. Independent arithmetic reproduces all 126 comparison bounds
and the selection. Shared comparison edges are not independent blocks; sampled
isolation and common-median/independence assumptions remain limitations. The
maximum observed absolute lag-one excess correlation is approximately 0.643.

| Retained identity | SHA-256 |
| --- | --- |
| B scientific records | `01feb743eb05781b015ba945403cab7d01b150bb25c0ac65e7c41838c55f33d4` |
| Independent B science review | `caeee49cdc109b182b0c4b438b1cc6f4080b5164e72429bacd469fe657206aca` |
| Frozen B timing manifest | `98b3f351789b7b952b669f12aba04277a5163848fd46cbfa85be15b4981fbde3` |
| B timing records | `eb092491904576b044e3b7611f7b8aa7ca2235e3ce64a79c85633ba344debdfd` |
| B host receipt | `483e10cbc2082158f05dca81b7bf2c69e7445d6d8966e0b40f1fc31289119928` |
| B decision, logical identity | `c196126cacb12be19df507ed152f62cb5667f357269de00306c7f22eca6d5845` |
| Independent B timing review | `f863c43155705609b23d43f23501f0d99ef9d6019e4219600f3da8b88daa1567` |

Only the many-FE invocation flag is provisionally enabled on `codex/auto-routing`
for a private public-invocation artifact. Two-FE remains disabled; kernels,
eligibility and build inputs are unchanged. Fresh source/native/transaction
checks and exact hidden/public A/B equivalence must pass before C access.
The untouched C24 panel remains necessary for acceptance; failure or inconclusive
confirmation restores the prior default. Main and release packages are unchanged.

The first source run stopped at a worker-thread-local routing marker (`4`
instead of `16`). The isolated test passed. Independent source review identifies
shared Rayon workers as a plausible instrumentation interference mechanism;
the actual interfering schedule was not captured. Serial execution passed the
core tests, then exposed three obsolete oracle assertions requiring Auto's old
MAP route. Historical geometry-led MAP witnesses now explicitly request diagonal
preconditioning, outside the new Auto/Auto scope, while public Auto is checked
against the selected hidden policy with independent QR, exact science/signatures,
covariance and DoF at three, five and sixteen effects. Twelve focused oracle tests
pass. The corresponding Stata witnesses preserve their numerical assertions.
Strict unseeded routing retains original-input MAP fallback after an uncertified
LSMR attempt; it does not promise fallback-free execution. All previous failures
remain recorded. The only changed production
token is the many-FE default flag; all 58 artifact inputs remain identical.
The final `RUST_TEST_THREADS=1 ./scripts/run_tests.sh --source-only` passes:
Python, Clippy, full Rust workspace, FFI and isolated allocator audits, portable
checks, complexity and mock-bridge guards. Native build and licensed Stata checks
remain separate prerequisites; installed release-package admission was not run.
The passing source log SHA-256 is
`8607aead9702dae51d3d6c3b49b103f2cb69e853455f2c7ac54abb8447f1e258`.

SHA-256 identities: initial failed source log
`59ca4c6ea840ff97b6c3d70165385ec6dd97600869b2da26d050e0d11c970906`;
serial old-route failure
`82344e7653d51a1dfcb559adc52b1593e53bbc2e48f625c3fc42a1c536a344ae`;
focused migrated oracle log
`befd213629cf92089b8dd1227e3872f6cf3fcdabbf9836a9acecafbaab493403`;
independent test migration review
`bc6fe6201e01d4929a29dc51971c8dc7bb53055eb31b28cccb5eb58195c4f0df`.

The private universal build from `a3f19059` passes the binary privacy/export/
dependency audit and actual ABI/load/panic-containment checks on Apple Silicon.
Its plugin SHA-256 is
`5b2a34707e4e71e8d88c71152414940bd59a823e246e58be979a78fd10fcb95e`,
build log `820ec2609c6de3de3b9745aca5269341d6943a0f5dc74dfe378541a42a57a1fc`,
and source archive
`bb35c7be8d69780e21553d8da58bdabd0923ed204aba071f3405c6853edf60a2`.
Building both Darwin slices is not an Intel-host Stata qualification.

All 96 actual-plugin native smoke calls pass independent science, exact repeats
and selector comparisons. Six of the seven mapped Stata fixtures passed on the
first run. The mixed-report migration incorrectly required LSMR without fallback;
one bounded diagnostic confirmed the fixture's original certified MAP fallback
with zero pilot iterations. Restoring its original precise MAP/fallback and final
LSMR assertions, retaining every numerical check and adding zero-pilot and
certificate checks, passes the complete fixture. The initial failure and diagnostic
remain recorded; no runtime or plugin change was needed. The six already-passing
fixtures were not repeated. All four real Ctrl-C checks pass: OLS, the positive-work
optional builder, PPML, and a three-FE adaptation of the existing OLS fixture.
The adaptation adds a nondegenerate third effect (dummy rank 18 to 19; including
the regressor gives rank 20 in 24 observations) while retaining both native and
post-plugin checkpoints and every rollback assertion. This does not establish
interruption inside LSMR iterations; unchanged solver cancellation tests cover
that layer. Exact hidden/public A/B equivalence remains pending.

| Actual public-artifact checks | SHA-256 |
| --- | --- |
| Native 96-call science summary | `24c9c04801178148c5535deb5a7f398c4235d44986541f60ed261c6b2340a0b4` |
| Initial Stata summary, six pass and one retained assertion failure | `ea0770e1952ff02500949c21b87a4228cf06de2121ecc0036e768100d62cd35a` |
| Corrected complete mixed-report fixture | `ae2e03fdd17a7a96380b8ead5d588a3677c683318d31f2e134f5c258a8824d8b` |
| Four Break outcomes | `2662f368df67770940a39e0e01f6eeb3cf630fa03ff4df701cf35bc619060b2c` |
| Corrected independent migration review | `0ac933bd88b3053bc4524b72a2e7e7a6f33d466090728cf44a4dd78bd2f423b8` |

The fixture-only correction reuses the passing full source and native build
evidence: all 192 production modules and 58 artifact inputs equal `a3f19059`.
The changed Stata fixture passed its focused execution without changing a tolerance.

### Activation review and exact A equivalence

Independent activation review re-read the 96 retained native outputs and checked
their independent witnesses, 48 exact repeat/signature pairs, 36 outside-family
comparisons, 24 worker groups and 44 resource actions. Typed source/correctness
admission passes. The full maintained activation validator reproduces the frozen
B decision before admitting the original A/B equivalence contexts. The failed
initial aggregate remains failed; the reviewed passing aggregate binds the six
original Stata passes, corrected mixed fixture and four Break passes. Break
transcripts were deleted by the unchanged runner; compact sentinels remain.

The A equivalence phase completed its single planned attempt: 42 cells, 84
artifact/cell outcomes and 168 successful native calls. Maintained qualification
and independent review pass. All four hidden/public and repeat scientific and
solver fingerprints match within every cell, including ten eligible many-FE
and 32 outside-family cells. Review independently reconstructs 84 retained repeat
solver signatures and 42 cross-worker solver-discrete comparisons. Twenty traced
prerequisites provide 40 resource actions within their limits; repeats inherit
that resource evidence. There were no failures, retries or replacements, and no
owned processes remained after completion. This is scientific equivalence, not
new timing evidence. B equivalence and untouched C24 remain required.

Before this attempt, source review identified repeated full B reanalysis inside
the oracle child's 125-second process limit. A separately reviewed parent
admission adapter runs the unchanged validators once, records the files they
actually read, and verifies those bytes around each bounded oracle child. It
preserves the 120-second mathematical alarm, native budgets and original tools.
Six focused tests pass. A's handoff contains 3,713 entries for 3,711 unique files,
including all 3,150 B call logs and observer events/observations. The exact
admitted configuration is retained separately from its later mutable pointer.
All 16 independent oracle certificates and child-success logs pass review.
The original producer deleted successful native arrays and trace logs, so this
review cannot freshly recompute full output-to-oracle numerical comparisons or
model-discrete worker agreement from those deleted outputs. It checks retained
proofs and certificates, with narrower independent solver-signature checks.

| Additional retained identity | SHA-256 |
| --- | --- |
| Independent public activation review | `49e00e005e358274ae26b850f8305ad96b883855265e9f15d3a14bc166717c24` |
| Partial activation packet | `bac2690efc503b9941f99228996e44e94fc807b56967387798cb9b8c9f8fdaf3` |
| Reviewed parent-admission adapter and supervisor | `abd9145bde0504f9597eae4da3d6af8ed8cd0cd5c463bd1207a7b542a7a929e8` |
| A equivalence scientific records | `5763ff257ba5231642ff6a87e40ac93aaf7492d6378a182f2beb182caa30bf33` |
| A admission handoff | `22bcb3f91400ff23eed92fe2ba5082ddc040dcc55c767c4f2441c2f18a016a25` |
| A terminal outcome | `16ff9ea3325e890a8ef1e4cd4a7351314dc31e71ee5b407f58841e083d45363b` |
| Independent A equivalence review | `f84dceed282103a1ed10ac06f9d6f44429158a190a5838e7df2161ad61a5f8b3` |

### Completed A/B activation equivalence

B equivalence also completed its single attempt: 42 cells, 84 artifact/cell
outcomes and 168 successful calls, with exact hidden/public and repeat science
and solver fingerprints. Independent review verifies the 16 oracle certificates,
84 retained repeat signatures, 42 cross-worker solver-discrete groups and 40
resource actions. No retries, replacements or cleanup signals were needed.
As in A, the unchanged producer deleted successful native arrays and trace logs;
the review checks retained certificates/proofs and narrower discrete signatures,
without claiming to reconstruct the deleted numerical outputs. Repeat calls
inherit prerequisite resource evidence.

The unchanged maintained validator subsequently reconstructed the complete A/B
report in 753.65 seconds: 84 cells and 336 successful actual calls. Independent
review rebuilt every factual row and checked all 40 public prerequisite trace
actions. The typed equivalence report passes the shared activation guard. Adding
only that report to the activation packet exactly preserves the partial packet
used by both producers. This completes scientific activation equivalence; the
public many-FE flag remains provisional pending untouched C24 confirmation.

| Additional identity | SHA-256 |
| --- | --- |
| B equivalence scientific records | `5887aa45a4fe11ca582d3ba60c64402cc5b92ff50fb72e67e11dab2ca42a7e31` |
| B oracle admission handoff | `55abda6aad703d94c8a9e52e3771f3afe05f7983b7a74049c5f2412756bc3bd8` |
| Independent B equivalence review | `a1d123874988c3198328db0475a9ce9a681a2f16193c08f543958e34825fb114` |
| Maintained combined factual rows | `681b5e31c474f2a584a27617013b7d7525dab56491915708f3825287661be9f3` |
| Typed equivalence report | `37308925166a2c9a6d70ab635fd1d9bc5bf0dc4e7cf6b5b6414a90f34480c585` |
| Independent activation-guard check | `8cf8a5eeaf4a1996f54e5d9b0908c222529a3232ecde7dd9dc9f7bd63bcd49ab` |
| Complete activation packet | `3119bb2b3832462e2ceac7132892952d0f65f0b9a4e0d2217c8da3f8441c469b` |

### Prospective confirmation metadata repair

Before C access, source review found that the original confirmation workflow
would repeat full B analysis 852 times during timing freeze, 630 during
collection admission and 1,482 during the final decision. These are source-level
call counts, not measurements of C or its runtime. A separately reviewed
task-local wrapper reuses successful context/activation metadata for identical
arguments within one process. The four original metadata functions still run
on each new argument tuple; qualification, mathematical witnesses, individual
native checks, host classification and C analysis remain unchanged.

The wrapper returns independent copies, propagates nested read dependencies,
checks file identities on reuse and hashes full dependencies before/after each
operation and immediately before timing begins. Drift poisons the operation.
The unchanged passive observer wraps the actual collector; successful inner
records cannot bypass a failed metadata postflight or observer completion.
The frozen input declarations, artifacts, 120-second call cap, 28,800-second C
timing budget, 24-block design and performance/scientific thresholds are unchanged.

Fifteen focused tests pass. One final metadata-only B smoke completed in 176.23
seconds: one original activation validation and two exact reused results,
caller-mutation isolation, three full byte checks and 3,711 bound files including
the external Python executable. No native calls or C body reads were made.
The initial launcher then stopped before admission because its file-reference
helper received a string instead of a `Path`; the corrected launcher and actual
helper regression test passed renewed independent review. The failed preflight
and earlier review remain recorded. The unchanged memoizer/driver reuse the
valid B smoke; this does not establish C correctness, timing or acceptance.

| Metadata-repair identity | SHA-256 |
| --- | --- |
| Exact metadata wrapper | `74719e1f96bdb6adaff4f2bc09ebcfe584508cb7edb175223f6d00cd8404e95e` |
| Confirmation driver | `5514a67866e40f3bdd85386b2b798c4dae9c086ad9c19f2cd5eea061782f01f4` |
| Corrected owned supervisor | `652f82e73f8310d5049f1fca3dd3ad060bb1cd4dbce938677971705e031a2164` |
| Final B metadata smoke | `5cc7748b11a4c91ca0411916a286a350ea704cd0b5130695a8ef3632c8d604a4` |
| Independent revised source review | `f0f3317e8ad9017677657c3242b727f26b78f679a37623c0faeb26058ea2ceed` |

### Many-FE confirmation stopped before native science, 2026-10-18

C admission passed independent review, including 3,719 actual-read file bindings
and the unchanged 42-cell, 24-block design. The subsequent single witness attempt
hit its fixed 2,100-second outer limit after 2,100.637 seconds. The interrupt
occurred in dependency hashing while validating a prepared wrapper. Four
independent oracle references passed their certificates, and 16 of 42 ready
wrappers had been written. Full witness preparation did not finish. No C native
science call or timing call ran; this is an operational qualification failure,
not an observed solver or numerical failure.

Independent review rehashed 3,935 retained files, checked all 28 partial output
files and verified the four available oracle certificates. The supervisor sent
one SIGINT to its owned witness process; cleanup completed with no survivors.
A separate process-birth census confirmed those processes were gone. Partial
outputs and the original failure remain unchanged. C inputs were accessed, so
this attempt cannot be replaced, continued or treated as an untouched holdout.
There is no C scientific or performance pass and no accepted many-FE default.

An ordinary rollback restores `DEFAULT_MANY=false` and the incumbent Rust/Stata
test expectations; `DEFAULT_TWO` remains false. The additional hidden 16-FE QR
witness remains. Production identity matches preactivation `7b164ca8` across
192 runtime modules and 58 artifact inputs. The serial source-only integration
suite passes in 200.61 seconds, including Rust, Python, Clippy, allocation,
complexity and bridge checks. Prior native/ABI, exact original-Auto, mapped Stata
and Break evidence for the identical preactivation production inputs is reused.
No release artifact changed. The earlier B improvement remains a developmental
observation; incomplete C confirmation does not establish a speed regression.

| Failure/rollback identity | SHA-256 |
| --- | --- |
| C admission independent review | `885a4cdcd727b2d504a63c2fc51f2f61cc4afb6b4d7c9c4af4d6bc5ecf180c90` |
| Failed C supervisor outcome | `101cbe6347587065c76c2777b73c96f92792833012124e2fd6e8a2a274f8c2bc` |
| Independent raw failure review | `90d598aa444d01caf121a1143fdd98291f44b8501433084766805a59cce407c7` |
| Current owned-process census and failure validation | `41fcce9f77cffbdc918b2b7e99228144dbfb524aaf9eeaf8011965bd60f46d92` |
| Rollback source-suite capture | `becab2ad92e9d6408df84a915a7208be68e42469821fdff209992c78c57da6d5` |

The existing terminal exporter handles completed timing decisions, but not a
failure before timing starts. The separately reviewed
`routing_failure` extension (subsequently retired with the old harness) validated this
retained-only outcome without changing frozen selector files or manufacturing a
timing decision. It checks the actual failed start/outcome, complete owned-process
census, partial outputs, absent science/timing, admitted metadata and full file
closure, original source/build evidence and independent export review. Sixteen
focused tests pass, including rejection of changed external dependencies,
incomplete cleanup, unexpected native/timing output and attempted acceptance.

Independent export review verifies the original Mac plugin/build log and Linux
build log, successful setup accounting (`8161659`) and prior compute-node byte
audit. It also recomputes all original/current production and artifact-input
inventories. The retained incumbent remains `f35f3a4d`; the branch's disabled
hidden-candidate additions have their separate preactivation identity. Actual
terminal generation, local raw replay and portable validation pass in 12.17
seconds without B reanalysis or native calls. This closed the first many-FE attempt
as retained under the then-current handoff. Johannes subsequently required
completion of the many-FE finalist before two-FE advances, as described below.
A prospective two-FE
driver must explicitly bind/install the failure extension for local admission;
portable validation alone is only an export-attestation check.

| Retained terminal identity | SHA-256 |
| --- | --- |
| Failure-extension source review | `54da45252c3f93d8a82ef72881e3a00becbed344c3d6bbd0e736aa3478d87087` |
| Independent source/rollback inventory audit | `35b39a0185119afc52289faba6cc4fd3a59c1b0cdf1c7e73792693c76b3951b9` |
| Independent retained-incumbent export | `a81c78929d468a612176ed34b1fb79b63e047fc527c8559a94269474d61f1ce3` |
| Many-FE terminal, logical identity | `a3a635318c29c8709ae53083ef9257d0dae370787095233006dd68cd9ccb5009` |

### Measured validation-overhead repair, 2026-10-18

The failed C attempt remains failed, with its accessed inputs and all partial
outputs preserved. Johannes authorized repairing the measured harness cost and
independently validating it on exposed evidence before generating a fresh
confirmation of the same finalist. The private public plugin, scientific scope,
Auto protections, C24 design, confidence standard and native budgets remain
unchanged. The branch's public default stays disabled until confirmation passes.
A further operational failure requires diagnosis and direction; it does not
automatically start another confirmation or advance two-FE.

The two-FE setup job `8164867` completed before the stop request took effect:
56 seconds, four slots on `scc-gd4`, scheduler `failed=0`/`exit_status=0`, with
setup and native-smoke outputs retained. No discovery producer was submitted,
and two-FE B/C bodies remain untouched. Its qualification is deferred until the
many-FE outcome, applicable checks, documentation and source are complete.

Bounded profiling on old evidence identifies repeated hashing as a material
cost. One original partial-activation validation made 163,290 digest calls over
3,702 distinct files, reading 341.735 GB in 198.42 seconds; 150.69 seconds were
inside hashing. These numbers include profiler instrumentation. One old ready
wrapper alone read 5.608 GB in 779 digest calls, taking 2.84 seconds.

The new operation-local validator reuses exact metadata results only at four
named entrypoints and verified immutable dependency hashes. It preserves original
semantic validators, captures regular-file reads and aliases, checks identity on
reuse, and fully rehashes bytes at execution boundaries and completion. Eighteen
focused tests pass. Independent source review approved the utility, versioned
confirmation extension and supervised driver. The final routing integration
suite passes all 372 tests in 188.54 seconds, including actual bounded dummy
process success and timeout/descendant cleanup. Thirteen preparation tests pass;
review caught and closed interruption-during-cleanup and output-path escape
issues before any preparation execution. Full exposed-harness validation remains
required before fresh generation. These tooling changes leave all 192 runtime
production modules and 58 artifact inputs unchanged.

That initial repaired profile passed in 37.49 seconds, including a second exact
metadata hit and full before/after byte checks. It hashed 18.314 GB across 3,710
closure files. Both returned values have the exact original canonical identity
`642f05f689174b5ef642852ddb67060b0ad1b3422d654d8b8618b7225b92a080`.
This diagnoses and reduces validation overhead; it is not a solver timing or
fresh-confirmation result. No new native or oracle calls were made.

| Repair evidence | SHA-256 |
| --- | --- |
| Original admission profile | `3cf70f1c12f251f6a5408ffb5f486d5adcae958ecf5f7aad43c841f2d270b633` |
| Original ready-wrapper hash profile | `2e1117ea21042a048321801b22291242ac0d44e76c7c56f0b7ba2b1000185eb7` |
| Repaired initial admission profile | `56fa079f0573f1665b81867ee94aeb5c79dc4110e1708ec59c5419e32f60d966` |
| Utility source review for that profile | `753100510f142522d681c3a8a1a36874e7189def0774c34516656e152c3393a0` |
| Independent repaired-profile review | `85f16656b864ea8b8606b7868072505aeb657884a26ea2529769469a7a316aeb` |
| Confirmation extension source review | `2896831a08b89692de968d04803dfc9add4244702a749321ac0651d7dda14190` |
| Supervised driver source review | `72e42e26ab7d3465ff1c96d2eac3ebb901de0a4bd7c00e733997461db148c40c` |
| Preparation source review | `dc89c567f9bee66a58535b9aaa114c5f58388a75f4a4d1e401ba50f396974e76` |
| Final routing integration summary | `a888e49a7bf7bffa5366cb1cbc4a92b0d27df5dce86c93314d829f717e92d7af` |
| Deferred two-FE handoff | `651dbc39aa48380e9de14d728dad46bb4a03863268b88de8eb04ae9382b77f5a` |
| Two-FE setup accounting | `6decdfbb081209cb6a999f74e23ac0265e899cfce235ab6e1caa55c75ebecfee` |

### Rebuilt input generators and exposed-A verification

The original native input-generator executables were absent when preparing the
repaired workflow. Bounded restoration builds did not recover their recorded
hashes. The current-source and exact `f125c0b3` archive builds produced identical
replacement binaries; a separate original-core build also failed to recover
the originals. These are rebuilt artifacts, not restored historical binaries.
The earlier uncommitted imports-generator source and whole-core build-cache
identity cannot be established from the retained evidence.

Source review traced the actual RNG, design, wire serialization, core metadata
types and independent expected-value mathematics. Those output-reachable paths
are unchanged; estimator routing is not called by the generators. One bounded
diagnostic then regenerated only exposed A using its original seed. It completed
in 15.70 seconds without native estimation. All 42 case/body/descriptor identities
and eight emitted scientific-reference files match exactly. Of 55 compared
files, 38 match byte-for-byte and 17 metadata files differ solely in their
recorded generator hash. Every raw difference remains explicit. Forty-eight
existing independent A-oracle files were verified unchanged, not regenerated.

Independent review rechecked the raw comparisons and all 847 closure files and
848 aliases. The prospective generator mapping requires this exact evidence,
preserves both historical and rebuilt identities, and rejects unexplained
scientific or metadata differences. The ordinary exact-generator requirement
remains the default. Twenty-four focused tests and all 375 routing integration
tests pass; the latter took 189.77 seconds and included actual bounded process
cleanup. This establishes tooling provenance on exposed inputs, not fresh
confirmation or a solver-performance result. Both public defaults remain off.

| Generator evidence | SHA-256 |
| --- | --- |
| Rebuilt case generator | `477fa714226a81ecd586ea957b2c9f86e05e073d6ef31cab6f4c2d19fd076250` |
| Rebuilt imports generator | `ab2fb6b8577c5769a120170997d87fb62df259bc47217018c11d2169ac432bad` |
| Bounded restoration summary | `7ae8cffdcd7f40cf7933aed3845148c2f08c8a53cdf3261e6ef32c485a914a62` |
| Exposed-A diagnostic outcome | `8400494899984cf36d1d3eb6e110021bd2f1f0e79924af2bca4515cac517e23f` |
| Independent raw audit | `78b28d4851d397397a694a94f8833a50d07b79bf523784010bb2fdd082211882` |
| Generator mapping review | `dcc9fae9c8ad94c6e7d17bdb193203aa64b8d7f19ac876da4d466c938ab67fd2` |
| Optional provenance-gate source review | `3a89ed5bbdcb52c83e9d553c8116af61767c38b55a7ca1e8cdb95f8022675d4b` |
| Final 375-test integration summary | `d10ca181ce79fd35228b9caf42700cbb8e6f55fec9484ecc8f5d0d866f9d1b61` |

The complete exposed-evidence validation subsequently passed in 506.40 seconds
under its 1,800-second outer bound. It reconstructed the original C admission
from the unchanged four inputs, reproduced the canonical partial-activation
result exactly, and verified all 42 existing B wrappers. The child performed
two full byte checks over 4,835 files, reading 23.266 GB in actual hashes with
542,528 immutable-digest reuses. This complete operation includes more work than
the earlier partial-admission profile; their elapsed times are not a paired
performance comparison. There were no native or generator calls.

Independent review rehashed all 4,837 files and checked all 4,838 aliases across
the child and supervisor records, with unchanged stat identities and no drift.
Complete and partial activation metadata retain distinct keys; the six required
validation checks pass. The owned child exited zero, no cleanup signals were
needed, and both recorded processes were absent afterward. The exact repair
proposal passed independent review before declaration or fresh generation.
The remaining confirmation still uses the same finalist and private public
artifact, C24 design, tolerances and work limits; both defaults remain disabled.

| Complete exposed validation | SHA-256 |
| --- | --- |
| Actual validation report | `2666133146b0c7e5aaa0b0c1fa4cf13e2b5a3f7cb6a4f8995dff7cdac4bb1562` |
| Successful supervisor outcome | `7cdd7da4877eb3d8662e4e29ef65e4a91d57e694e433b81e478085b00ea26815` |
| Independent raw review | `43024b4ae420ad3b07f7451a576f08478d96fc0c7586e34c528c44535a7f7491` |
| Final prospective repair review | `1ea6548583f9bb8c6227921c48740fadcc745eea17df7fb6c3151daec3f67465` |

### Fresh many-FE confirmation inputs, 2026-10-18

The once-only declaration completed in 485.05 seconds under its 900-second
bound, before any fresh generation. Independent review verified the sealed
proposal, repair approval, chronology and all 4,903 files and 4,904 aliases in
its read closure. It preserves the selected candidate, private public artifact,
24 blocks, one warmup, alpha 1/40 and all original scientific and timing bounds.
The new input and timing seeds are 2026101801 and 2026101802.

The single generation operation then completed in 516.78 seconds under the
same 900-second outer bound, including admission reconstruction. Its two owned
commands exited zero in 0.15 and 9.13 seconds, within their 120/300-second
bounds; no cleanup signals or survivors remained. Six full byte checks covered
4,905 files and 4,906 aliases. Independent review rehashed that closure and all
21 distinct case files and numeric bodies. All 42 cell descriptors and worker
groupings match the original design, with no case or body identity shared with
any prior many-FE or two-FE A/B/C panel, including recursive history. The two-FE
B/C bodies were not accessed.

These are input and provenance checks, with zero estimator or timing calls.
The rebuilt-generator limitations above remain. Fresh independent witnesses,
science, resource checks and controlled confirmation are still required; both
public defaults remain disabled.

| Fresh preparation identity | SHA-256 |
| --- | --- |
| Sealed declaration | `fbe1307c9b6721f0ae0c497376f10141407f794e7a9009086e295be66bd57d31` |
| Independent declaration review | `fa069ef030ad51a58a78aac7952ab475d27e5db01f11a40f12ed3ef02f98a76d` |
| Actual generation record | `4033ffe4df0ff2cddbeb7d14b189457b4345d478d86c394a88718df3d9e8f790` |
| Successful generation outcome | `90dc8270470c106f0a744eef7dc0c0c047a51666e7f93afedd5fadcb60913b8c` |
| Fresh panel | `3214d41b18602d1e2ee51061ca341cdaec760e0388e5239152279f41222b35fc` |
| Independent generation review | `58c3983863f1eff332838955140728832aa0402a06492174e0da572a49930602` |

### Fresh admission and independent witnesses, 2026-10-18

The single fresh admission completed in 897.78 seconds under its 1,800-second
bound. Independent review reconstructed the exact context from the original C
context and the declared fresh cells, design and version marker. Artifacts,
models, protections and the original four admission inputs remain unchanged.
All 4,591 combined dependency files and 4,592 aliases passed independent byte
and stat checks. The supervisor and child exited zero without cleanup signals
or surviving owned processes.

Witness preparation then completed in 372.02 seconds under its 2,100-second
bound. All 42 ready wrappers bind the exact context, artifacts, resource contract
and shared dependencies. Sixteen independent oracle bodies cover 32 cells;
their retained rank, numerical and resource certificates and array hashes pass.
Four imported local references cover eight more cells with exact output and
saved-variable mappings, finite values and valid shapes. The two inactive-column
cells retain the unchanged executable oracle that runs during native science.
The parent completed 33 full byte checks, each oracle worker two, and the outer
supervisor two. Independent review rehashed all 5,056 combined dependency files
and 5,057 aliases and checked every worker/parent receipt and fixed bound. No
owned processes remained and no cleanup signal was needed.

These stages made no native estimator or timing calls. The oracle review checks
retained certificates and array identities; it does not claim a second oracle
solve or native scientific agreement. Both defaults remain disabled. Native
science and its independent review must pass before the single C24 timing panel.
No scientific tolerance, confidence standard, artifact or budget changed.

| Admission and witness evidence | SHA-256 |
| --- | --- |
| Fresh derived context | `6b7cebb61255abafdfd1ec5e0810bdcf7831129d6f46507b3b22b41dce122f7a` |
| Independent admission review | `e4d1806682f82c90874c41367de46d834807576cc6c6f6c9f6ebde476beeb0c1` |
| Successful witness supervisor outcome | `c2695b94b9783a53792a38c3e3bd00761881876ac7bf96c53efb9221ad27e470` |
| Actual witness report | `1b55b8a4c70d5aac15c742f2cbc18fb4927322cdca8830ed97871a16b9246152` |
| Sealed oracle handoff | `9b2d2bb795261827086aaf7a5db65a6861435a5d9f9cf8d38aee0db60b05d5d7` |
| Lead witness validation | `18c93dcc47637536b2432796cbc01170f89e07ef2487cf966a70e8c6476bc98a` |
| Independent witness review | `0023e42723ee038935288c32f74cdf5b538c7d353ec3fb7a3d476641085c293d` |
| Independent local-reference addendum | `121a11a1e0571b074f8913e6d0d1ffd79e33b1feb55a16692eb57406bb3a8271` |

### Fresh native science and postflight, 2026-10-18

The single native-science operation completed in 2,712.10 seconds under its
15,000-second outer bound; the native phase used 2,474.64 seconds of its unchanged
14,400-second budget. All protected outcomes pass. Of 444 prerequisites, 426
pass and have successful fixed-worker repeats with identical scientific,
diagnostic, prepared-input and witness fingerprints. Eighteen optional
MAP/Irons–Tuck cases remain unavailable: twelve exhaust their iteration budgets
and six reach the fixed 120-second timeout. Their linked `not_run` repeat rows
are not additional attempts. The panel made 870 native calls: 852 successful
and 18 failed. The CMG/Irons–Tuck worker checks group those existing records and
add no calls. No failed case was retried or replaced.

The unchanged qualification, comparison-coverage and scientific/resource
postflight completed in 189.15 seconds under its 1,800-second bound. It checks all
42 wrappers and accounts for 402 comparison edges: 126 protected edges admitted
to timing, 258 diagnostic edges and 18 unavailable diagnostic edges. Lead byte,
stat and alias audits pass across 5,025 files/5,026 aliases for science and
5,067 files/5,068 aliases for postflight. Each child and supervisor completed two
full byte checks without poison. All recorded owned process identities are
absent, both operations exit zero and neither requires cleanup signals.

Independent raw review passes all 42 proof seals and their records, resource
bounds, failure artifacts and comparison coverage. All 336 protected calls
pass; 426 repeat pairs have exact retained fingerprints, 213 comparable worker
pairs have identical discrete solver decisions, all 42 public/hidden fingerprint
comparisons match, and 128 outside-family exact checks pass. The reviewer
rehashed 5,155 files and 5,156 aliases without drift and independently confirmed
that all 512 unique recorded science/postflight process identities were absent.

Successful native raw arrays and transient result/log files were deleted by
the unchanged producer. Review therefore checks retained fingerprints, solver
rows, proof seals and executed checker identities; it cannot independently
replay every array comparison or inspect unavailable RCERT payloads. The
inactive-column oracle retains its identity and passed checks in sealed proofs,
not separately retained numerical values. Neither a native call nor an oracle
solve was repeated for review. Both public defaults remain disabled, and no
timing or performance conclusion follows from these scientific checks.

| Science and postflight evidence | SHA-256 |
| --- | --- |
| Actual C scientific records | `392c6bdabea7b067217bc89df763b5bf0a75cae51933fb674cf13869bc5afc10` |
| Successful science supervisor outcome | `37c5236c79b7a5e71f54015add9ded1457a0c0200807d1a0c8a9dcfb961ad01c` |
| Science child outcome | `da75b8d7f6a663396857c6512c42a7c19f28c3fdc96746a7961eae303469a486` |
| Lead science validation | `3597f46dfad731781b541e51537cb1decc988cbb8d40302d09466aed30977a1b` |
| Successful postflight supervisor outcome | `773be8c2a78d3ba5542adc1b0a8e9b68e8363401bb941c86bd57dba2c60bff4b` |
| Postflight child outcome | `5abc338c02b67425675fecd3ccf8ccc00688cbb3bc167557c4ec3e21bc1fd9ea` |
| Actual postflight report | `3564a1b838314de5d3d5f0e25771edc0f92f1a63767aaec34e6c29177756e639` |
| Lead postflight validation | `77583e5dd97e8a23ec9b7d1475acc275de05d4b1c5e3f13b260d8efd1eab2ff7` |
| Independent raw scientific review | `000dc5467252a8943551a9f64b7907c29a7f522f8bfa049b65febfdc37824432` |
| Typed scientific review for timing admission | `e01f0ab514ebe2e5adfdaa0e72c00baee2d8832800aad00da2e216b894ffebc2` |

### Fresh timing-freeze failure, 2026-10-18

The single timing-freeze attempt exceeded its fixed 1,800-second execution bound
and terminated after 1,801.64 seconds including cleanup. The supervisor records
a timeout; the child records the resulting interrupt. One `SIGINT` stopped the
owned child, cleanup completed, and a current process census found both recorded
process identities absent. The monitor was deleted before failure validation.
No timing manifest or successful report was produced, and collection and the
decision stage were never started. Both public defaults remain disabled.

The partial packet contains 42 cells and 168 prepared requests, all matching
their scientific prerequisite hashes. A post-failure audit rehashed the observed
closure of 5,123 files and 5,124 aliases, totaling 8,860,899,717 bytes, and checked
their stat identities without drift. All 174 partial artifacts are preserved
for diagnosis. These retrospective checks do not complete the failed producer's
boundaries: its child completed zero full byte checks, its supervisor completed
only the prelaunch check, and both operations are poisoned by their exceptions.
There is no timing or performance acceptance.

The retained traceback stops in `time_blocks.freeze` while constructing a
`RoutingScience` adapter. Its context-binding loop calls the original file digest
after the cached metadata function has returned. The hash wrapper reuses bytes
only inside the four metadata entrypoints, so this downstream loop still reads
the files repeatedly. The paired design creates 126 adapters for 42 cells and
three protected comparisons. The later `all_bindings` loop also checks duplicate
bindings before deduplication, and `load_manifest` repeats both paths. These are
source-level findings; the uncached path has no retained profile establishing
its exact total bytes or share of elapsed time.

Independent review checked the outcomes, traceback and source path, frozen
configuration, all 168 prepared-input identities and the partial-output inventory.
It agrees with the diagnosis and attributes the full closure and current owned
process census to the lead audit. This is acceptance of the failure diagnosis,
not acceptance of the freeze. Adapter construction can execute the existing
inactive-column oracle if reached; the retained evidence does not establish how
many adapters completed. No native timing collection occurred.

The failure does not erase the completed scientific checks, but it prevents
confirmation of this candidate. No frozen source, configuration, tolerance,
budget or stage lock was changed; no producer was rerun. Another repair or
continuation requires Johannes's direction, including an explicit decision on
the consumed freeze stage. Two-FE remains deferred.

| Timing-freeze failure evidence | SHA-256 |
| --- | --- |
| Exact supervisor start | `7a314562b3a2a460401e3ae41e7e042e2d72553909acf653578aef7d012c388b` |
| Failed supervisor outcome | `1f554c0fcfdbf536da09b49f32347f3c4c3a8c7146a738e2a7065fede7868d45` |
| Interrupted child outcome | `7c3112c1514b6306cd4a0acf9512fb9335ac5c761eb37a2dba380224c4456117` |
| Partial timing configuration | `2bca08a6216553790bc873d469754cbd0e4d02285b26a27f792a24a199ba221f` |
| Lead failure and integrity validation | `1d34c6ce97725fff447fc68fb5de3a8b2db60173b3f832d4b2b9ecca5467073f` |
| Independent final failure review | `b14f72f5bcdb78280a35d1eaca0687494efc16862211f7a6fb817b88009dd295` |

## Main-branch GitHub installation, 2026-10-14

The root package installs the maintained Stata files and a universal macOS
plugin built from `46aa78f67b102373675b7c72196dba1fbfb7d337`. The packaging work
changes no estimator production tokens or native artifact inputs: the production
comparison passes for 192 runtime modules and 58 artifact inputs. The native
identity is retained in [`dist/fereg-native.json`](../dist/fereg-native.json).
The root package checker binds this binary to the runtime on `main` and rejects
missing install files, changed binary hashes and ancillary-file directives that
would omit required metadata.

- `scripts/run_tests.sh` passes, including 243 Python tests with 13 existing
  optional skips, Rust/Clippy/allocator checks, the complexity ratchet and native
  bridge checks. The Linux mock audit remains skipped on this Mac.
- The universal macOS build passes the binary audit and actual-plugin ABI and
  panic-containment harness. The dependency audit reports zero vulnerabilities.
- The corresponding-source archive builds with Rust 1.85.1, an empty Cargo home
  and offline networking, using only the included dependency sources.
- All four staged installation, PPML, native-file rollback and single-runtime
  fixtures pass in Stata/MP 19 on Apple Silicon.
- A fresh local root-package installation into isolated user ado directories
  installs all 31 files byte-for-byte. Selftest passes, OLS agrees with Stata
  `regress` within 1e-8, a two-group PPML witness agrees with log group means
  within 1e-9, and absorbed PPML saved means agree with prediction on `e(sample)`.
  The initial test driver incorrectly compared excluded observations too;
  restricting that comparison to the estimation sample passes without changing
  the estimator or tolerances.

The exact `github install johannes-schmieder/fereg` command and direct
`net install` from the public raw `main` URL both pass in separate fresh Stata
processes with isolated user ado directories. Each HTTP installation installs
all 31 files byte-for-byte from commit `4e7216e3`, passes native selftest and
passes the same independent OLS, PPML and saved-mean prediction witnesses.

The [beta assets](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.1)
retain the native and corresponding-source packages, dependency audit, source
closure and checksums. All 45 published assets match their local SHA-256 and
size bindings. Artifact SHA-256 values are:

| Artifact | SHA-256 |
| --- | --- |
| Universal plugin | `a75de3534e85921731ff97da1242b8fb872cc5e8782f2222da86a5f222424a7b` |
| Binary ZIP | `8faf8cca170268d55e71c9ae139ce7628d98e0c9014aa348d2bb48219ea25379` |
| Corresponding-source ZIP | `c2d7441cf757d2fc4bb64c529b9e0ba8560e7338b652595614e2d3b8eedaf4b8` |

This qualifies the documented macOS installation scope. It does not establish
Linux, Windows, native Intel-host Stata qualification or a performance claim.
GitHub Actions remain disabled.

## Controlled routing measurements, 2026-10-14

Johannes approved replacing whole-host quiet admission with the
[controlled paired protocol](development_workflow.md#controlled-timing-protocol-approved-2026-10-14).
Rules commit `da8551cc` retains ordinary OS activity, the 5%/1 ms allowance,
scientific tolerances and historical outcomes. Known competing computation may
invalidate only whole blocks under a prospectively frozen policy. The reviewed
tools in `438e1d93` separate diagnostic measurements, isolated SCC discovery,
Mac selection and untouched public-artifact confirmation.

The first fixed Mac A/A pilot completed all 300 calls and 72 blocks in 515.13
seconds: six workloads, twelve balanced ABBA/BAAB blocks each, one warmup per
label, no replacement attempts, no exclusions and no scientific failures.
Both labels used the same independently qualified candidate and input bytes.
The observer retained 142 process snapshots; monitoring coverage passed.
[Retained evidence](../bench/results/solver-routing-controlled-20261014.json)
contains every timing, scientific fingerprint, frozen policy, source/capture
hash and diagnostic summary.

| Workload | Median B/A time | Upper bound on median excess, ms |
| --- | ---: | ---: |
| OLS, no FE, 5 columns, 1 worker | 0.99992 | -0.888 |
| OLS, two FE, 5 columns, 4 workers | 0.99958 | -8.128 |
| OLS, three FE, 5 columns, 4 workers | 0.99997 | -14.291 |
| OLS, 129 columns, 4 workers | 1.00906 | -5.219 |
| PPML, two FE, 5 columns, 1 worker | 0.99878 | -19.027 |
| OLS, two FE, 20 columns, 16 workers | 0.99450 | -16.177 |

Here excess is `N - B - max(.05 * B, .001)`, using the two geometric-mean
solve times per block. All six upper bounds are below zero. These are conditional
diagnostics, not solver speedups or release qualification. The twelve-block
upper rank is 10, with binomial upper tail `79/4096 <= .025`. Serial dependence
remains uncertain: lag-one excess correlations are about .47 for two FE, .64
for three FE and -.53 for the wide case. The pilot does not prove independent
observations or common medians. Independent review supports proceeding with a
finite candidate experiment while preserving this limitation; A/A observations
will not be pooled into selection or confirmation.

The full repository suite passes, including 244 Python tests (12 optional
skips), 1,331 core tests, 195 plugin FFI tests and ten isolated allocator audits.
All 210 benchmark tests pass. An initial benchmark-suite invocation omitted
the existing task dependency path and failed to import SciPy/psutil; correcting
the environment passed without code or tolerance changes. Production identity
is unchanged against `da8551cc` for 192 runtime modules and 58 artifact inputs.
The previously accepted standard-20 timing evidence remains valid.

The SCC setup job also passed both Linux builds, export/glibc audits, actual
ABI/selftests and numeric guards, followed by 292 independent scientific calls
(73 cases, baseline and candidate, two repeats each). Fixed-worker repeats
were exact. This was an untimed four-slot setup job. At that boundary, isolated
routing discovery still required its reviewed scheduler/process evidence
collector and no routing policy had been selected. Public Auto remains unchanged.

The next tooling boundary adds shared mirrored multiarm blocks, the reviewed
SCC reservation/process collector, and a separate discovery-to-Mac B context.
Each pair shares two executions per distinct arm per complete block; B uses
twelve frozen rounds and preserves the original protected and internal-frontier
allowances. The coordinator recomputes selection from bound raw evidence and
refuses C until a separate public-activation transition is reviewed. The full
repository suite passes again, all 250 benchmark tests pass, and the final
discovery-only guard/timeout edits pass all 13 focused tests. Production identity
remains unchanged against `8bebd879` for 192 runtime modules and 58 artifact
inputs. These checks qualify tooling, not a routing winner or performance claim.

| Check output | SHA-256 |
| --- | --- |
| Full repository suite | `83472cff14c91b379086b66e6d8bf5c80c75864ac6598180d7ac61981f55db1a` |
| 250 benchmark tests | `6a8db9a2040b1aa7c8a214bd932bc4edeb7ba70653f7f426241ceb7be945303c` |
| Final 13 discovery tests | `b36c24775f681a23d81d6bb4e4fae8e20acb9589fb188376c8a54c50a03b2c74` |

SCC accepted scientific-prerequisite job `8103651` (four slots, 12 GB,
four-hour cap) and dependent discovery job `8103652` (28 slots on a requested
28-processor host, 84 GB, twelve-hour cap), using tools frozen at `09f7c134`.
The plan binds 211 files, 42 original A cells and the full baseline-plus-twelve
policy grid. Before submission, a plan-output path correction fixed a rejected
freeze without running any solver. The discovery job initially had a user hold:
release required predecessor accounting with `failed=0` and `exit_status=0`
and its complete fresh scientific proof.

The prerequisite job completed on `scc-tb4` with both accounting codes zero,
6,492 seconds wall time and reported `maxvmem=1.138G`. Validation checked
the frozen plan, artifact/input identities and all 42 resulting packets. Of
864 planned solver/case pairs, 846 passed with exact repeats: 1,692 passing
native calls. Eighteen optional MAP, Irons–Tuck and coefficient-space Irons–Tuck
comparisons on weak, high-degree weak and dynamic-weight graphs reached their
declared time or iteration limits; their repeats were not run. The original
failures remain unavailable comparisons, giving 1,710 actual native calls.
All 14 CMG and the one generic Irons–Tuck cross-worker checks passed. This is
Linux scientific evidence from a shared allocation, not timing evidence.

Only after these checks, the existing job `8103652` was released. Collection
completed three sealed rounds on `scc-zj4` with 28 reserved slots: both accounting
codes are zero, wall time is 21,607 seconds and reported `maxvmem=1.486G`.
Successful collection accounting alone does not establish node isolation or a
valid shortlist.

The dependent evidence-only wrapper `8103792` exited with status one after
58 seconds, before finalization or aggregation. Its six accounting queries,
each capped at five seconds, could not retrieve predecessor `8103651`; its
output directory remains empty. Fresh successful accounting reads took
6.9–7.2 seconds. The failed wrapper, log and terminal accounting remain retained.

An independent Astra review cleared a separate evidence-only repair: up to three
60-second accounting reads per predecessor, the original validators and
scientific tolerances, exclusive output creation and a 25-minute allocation.
The repair binds the sealed collection and frozen plan/source identities and
refuses existing finalization products. An initial submission returned no job ID
during scheduler communication timeouts. A subsequent Astra review established
that the identical permanent directory claim permits at most one finalizer;
guarded recovery `8121739` then ran and exited with `failed=0`, `exit_status=1`
after 43 seconds. It exposed 16 failed scheduler observations in the third round.
The original full-collection isolation protocol therefore failed. Its partial
accounting product, logs, measurements and failed finalization remain intact;
the old claim is never deleted or retried.

Both earlier complete rounds precede every scheduler gap. Separate reviewed
window-certificate tooling uses two conservative monotonic upper bounds: the
third round's original elapsed duration and an observed direct-child native call
in its directory. It checks scheduler coverage through both bounds, the entire
collection's process/configuration/accounting evidence, and all three rounds'
original scientific outcomes and fingerprints. This is a narrower retrospective
certificate, not a repaired full-run pass. An initial proof-only job `8126703`
rejected an incorrectly supplied producer-source hash before science; the
corrected binding is the actual frozen `09f7c134` producer `e7203750…`.
Corrected proof-only job `8126709` passed both original-source replay and a
second fresh-process replay, with terminal accounting `0/0`, 168 seconds wall time
and `maxvmem=838.719M`. Its certificate retains the two original round hashes,
checks 242 scheduler observations with a maximum gap of 60.011 seconds, and
records all 16 errors outside the certified window. Independent high-effort Astra
raw review also passes: all 4,322 process rows,
both configurations, original accounting, all three rounds' science and exact
fingerprints, and the certificate's eight raw hashes. No per-solve timing values
or rankings were inspected. Only rounds one and two receive the narrower
certificate.

The separate prospective coordinator was frozen to retain the certified first
two rounds and run only a complete replacement third round. It preserves all 13 original
producer/checker files, inputs, tolerances, warmups and call budgets. Each fresh
attempt reserves and verifies its own complete 28-core allocation. At most one
replacement is allowed after complete unchanged science and independently
recorded infrastructure failure; incomplete science and solver timeouts cannot
trigger replacement. The original third round stays excluded. A new aggregate
and independent raw-evidence review were required before a Mac shortlist could
be admitted; the completed SCC result is recorded below.

The recovery tooling at `27661fbc` passes independent high-effort Astra source,
clock-proof and architecture review. The full repository suite passes again;
295 unique benchmark tests pass across the routing and complementary suites,
including the final corrected-window regression. Production identity remains
unchanged for 192 runtime modules and 58 artifact inputs. These are tooling
checks at that tooling boundary. The window replay and independent raw review
passed before replacement measurement. SCC setup `8126735` passed all 53 focused tests
and froze the 287-file plan with terminal accounting `0/0`.

The first fresh round-three attempt, `8126744`, completed its full producer
round on `scc-zj4` but exited `0/1` after 7,309 seconds because its final
scheduler query timed out. The evidence-only job `8126751` completed `0/0`
and independently classified it as infrastructure failure, without aggregation.
Independent Astra raw review confirms all 846 passing pairs, 18 original
unavailable pairs, complete warmup/repetition/resource coverage and exact
science, prepared-input and witness fingerprints. All 55 timestamped scheduler
errors match the retained report. Its 1,457 process snapshots have complete
coverage and no known competing compute, but the terminal observer marker is
incomplete and the final configuration observations are absent. This is
replacement eligibility, not valid timing or node-isolation evidence.

Before using the single remaining replacement, one prospective five-minute
one-slot scheduler-health job, `8130615`, required three query pairs at fixed
60-second starts, retaining the original ten-second command cap. All six
responses passed identity/structure checks; terminal accounting is `0/0`, with
122 seconds wall time and zero native solver calls. This checks current service
responsiveness only and never substitutes for the frozen isolation protocol.
No further health panel was run or selected from repeated attempts.

The final predeclared replacement, job `8130631`, completed logical round
three, attempt two, with the same 28-core requirement, four-hour producer budget,
artifacts, input bytes and original tolerances. Its collection and producer
completion markers pass. Evidence-only job `8130649` revalidated both attempt
reports, the strict isolation receipt and the complete aggregate; its completion
marker also passes. Both jobs have terminal accounting `failed=0` and
`exit_status=0`:

| Job | Host / slots | Wall seconds | CPU seconds | maxvmem |
| --- | --- | ---: | ---: | ---: |
| Collection `8130631` | `scc-zj4` / 28 | 7,267 | 8,690.340 | 1.855G |
| Evidence `8130649` | `scc-tb4` / 1 | 317 | 305.373 | 1.515G |

Independent high-effort Astra raw review passes the final reservation,
configuration, process, scheduler and accounting evidence: 1,451 process rows,
122 scheduler observations, zero scheduler errors and a maximum scheduler gap
of 60.000187054 seconds. All 846 usable solver/case pairs preserve the original
science and exact fingerprints; the 18 original optional unavailable pairs
remain carried without retry. Certified original rounds one and two plus the
fresh third round provide nine paired measured slots. Independent recomputation
of all 7,614 measurement values, 13 ranking rows and 360 comparisons per row
matches the complete aggregate exactly. The original full-run protocol and
original third round remain failed/excluded, as does the first fresh attempt;
no third fresh attempt is permitted.

The SCC shortlist is `baseline`, `h32-k0`, `h16-k0`, `h32-k1`, in that order,
for Mac selection only. These are descriptive point estimates; the nine paired
slots are not asserted independent. At that SCC boundary, public Auto was
unchanged and Mac B/C had not been executed. Evidence-only handoff job `8132293`
subsequently completed
with terminal accounting `0/0` in 173 seconds, a passing application marker and
zero native calls. Its fresh frozen aggregate replay matches the entire accepted
result exactly and produces the typed Mac admission receipt. Independent
high-effort Astra review binds 84 actual local evidence files and validates
completed Mac qualification and cross-platform production/artifact provenance.
The coordinator has now admitted B12 and declared the separate untouched C24
budget before B case access. The single B witness preparation then stopped
after six local-truth wrappers: `low-saved-t1` and its four-worker counterpart
have unchanged frozen case/body bytes with three categorical effects, while the
derived declaration says two. Earlier preparation copied panel A family shape
metadata into every panel. The original B sidecar and canonical header already
specify three. No oracle attempt or native call occurred. The old declarations,
partial wrappers and failure log remain retained; strict model checks stay in
place. Independent high-effort Astra review subsequently verified an explicit
clerical amendment against 104 evidence bindings, retaining every original raw
reference. It corrects only those two B counts. The validator applies the exact
reviewed patch to the original sealed manifest and requires whole-manifest
equality, unchanged case/body bindings and original sidecar attestation. A/C
metadata, categorical flags, comparator/solver policies, tolerances and timing
designs remain exact. The amended B12 and untouched C24 declarations precede
any oracle, native or timing call in the amended cohort. All 23 focused tests,
305 benchmark tests and the full repository suite pass; independent Astra source
and test review passes. Runtime production identity remains unchanged for 192
modules and 58 artifact inputs. At that amendment boundary, fresh B witnesses
and science remained prerequisites for timing; C inputs were untouched.

Fresh Mac B science then ran against the amended declaration and exited with
status one after 2,411.6715738750063 seconds. Its retained result is
`complete_with_failed_cells`: of 486 planned solver/case pairs, 460 passed with
exact repeats, 18 optional pairs were unavailable, and eight first calls were
blocked with their eight repeats not run. Only 40 of the required 42 checked
wrappers were published. The original records, tool identities, failed first
calls and missing repeats remain unchanged.

Both `four-fe-t1` and `four-fe-t4` blocked explicit `lsmr`, `map`, `it` and
`it-coefficients` at the same cross-request checker assertion. Independent
high-effort Astra raw/source review identifies request-dependent primary
fallback/warning diagnostics: Auto reached certified MAP after an LSMR fallback
and recorded `1/1`, while the clean explicit requests recorded `0/0`. The error's
`exact_view` RSOLVER indices 8/9 are published raw columns 10/11,
`fallback_used` and warning count; raw timing columns 8/9 were already removed.
The original independent numerical comparison and oracle checks completed before
the recorded exception, as established by hash-matched execution order and
immutable failure evidence. All eight retained logs also pass independent
iteration/memory resource checks. Full original baseline results and output
columns are unavailable, so the review uses retained baseline `solver_rows` and
execution-order evidence and makes no fresh full baseline raw-replay claim.
Same-request repeat diagnostics must stay exact; scientific tolerances and
resource checks remain unchanged.

The initial narrow checker repair and separate derived completion were still
under development at the diagnosis milestone. Its proposal froze the intended
scope to exactly eight missing repeats, one per blocked arm/worker pair, with no
retry or replacement: 120 seconds per call and 1,200 seconds total, within the
original remaining budget of 11,988 seconds. At that point the calls had not run,
and no readiness or qualification pass was claimed. The original failed cohort
and that initial proposal remain historical evidence.

The implemented consumer now retains the old tool hashes, resolves only three
exact archived source identities, and accepts the completion through a separate
typed path. Ordinary qualification rejects derived-only fields. The B/C
transition changes source provenance only; native inputs, artifacts, scientific
tolerances and the B12/C24 designs remain fixed. A unique reviewed executor and
exclusive claim prevent copied-plan retries. Verification uses a fresh
stat-guarded hash memo for each pass, with unchanged content checks and no global
cache. Runtime production and native artifact inputs remain unchanged for 192
modules and 58 artifact inputs.

The first actual pure preflight failed before creating a plan or native claim:
the original record's `macOS-26.6.2-arm64-arm-64bit` platform label did not match
the old Darwin-only predicates. The narrow correction changes two source
predicates to accept proper `Darwin-` and `macOS-` string labels while retaining
Linux, Windows, misleading-label and type rejection. Earlier source review,
preflight failure, nine outdated deployment-fixture errors, an approximately
95-second concurrent/older-source check and the older 328-test pass remain
retained. Current checks pass all 40 focused tests in 7.245 seconds and all 330
benchmark tests in 190.540 seconds; the full repository suite passes after the
Python-runtime correction. Independent Astra source review bound 139 files and
cleared pure plan/preflight only, with native dispatch conditional on the final
330-test pass and execution readiness. The actual readiness receipt binds that
passing log before the claim. The successful pure plan preflight took
4.014218874974176 seconds with 402 bindings and eight requests, making no native
call; this is validation overhead, not a solver performance measurement.

The once-only executor then completed exactly eight native calls with exit
status zero in 72.04086220893078 seconds. Each repeat uses its original prepared
input, plugin, worker request and 120-second cap, and matches the retained
first-call full fingerprint, including timing-stripped solver diagnostics and
binary output-column bytes. No first call was re-executed, no optional failure
was retried, and no SCC availability outcome was imported. The typed completion
is a separate derivation: original records `e66c18a4…` remain byte-identical and
`complete_with_failed_cells`, including the eight blocked first calls and eight
not-run repeats.

Independent high-effort Astra raw admission review now passes against 434
hash-verified evidence bindings. A different comparison implementation strips
only time fields, checks every remaining raw scientific field exactly, compares
column files chunk-by-chunk byte-exactly, and independently reproduces the
original and completion fingerprints. It verifies completion certificates,
prepared/plugin/worker identities, the reviewed executor, once-only claim,
process completion/caps and full-suite-to-readiness-to-claim chronology. Actual
`routing_blocks.qualification` validation then took 19.16 seconds and admits all
42 B cells: 40 inherited proofs plus two separately derived cells, with 468
passing arm requests and the original 18 Mac optional unavailabilities. Every
unaffected row remains exactly equal to its original immutable row.

This admits B scientific prerequisites only. The missing original full baseline
raw output/columns limitation remains. Environment enforcement is established by
the pinned executed helper and successful control flow; there is no separate
independent environment snapshot. At this science-admission boundary, no B
timing, B winner, public activation or C-body access had occurred; public Auto
remained unchanged.

| Mac B science evidence | SHA-256 |
| --- | --- |
| Original failed-cohort records | `e66c18a4efd7f0143effacdf47e748c5b94ed008bd631b896ebb5ddde5b57fbe` |
| Independent checker diagnosis, 33 raw bindings | `3eadbd56f83fef32484770e6a073e3f46401a58b143c1ce7ef39cbb53e3d1fff` |
| Prospective source review, 139 bindings | `119daef737682899ec57fce5f3995c13d42fb8b742414c8fac1e6738d4486df9` |
| Frozen eight-request plan file | `e31f94f2d98aa1e89c23b5080fc969c5825df7e171fcbfde97e9d71af8582551` |
| Actual eight-call native report | `c48d0bb9910f39e03ec23b63eaf13e2abfa22dbdae8d62d10561806a9f47a52d` |
| Separate typed completion file | `9fb7541c7437409a5a446b64ce8d769064309cfee38aa557a78d338a5a82f92f` |
| Independent raw B-science admission, 434 bindings | `612f091a0d36196fb0e9bd762110bd47df52675d5962057969522c18b8614d71` |

The science-admission snapshot of the [routing explanation](solver_routing.pdf)
was 22 pages and recorded this admission and the 330-test pass. Built-in compilation
and exported compilation pass; visual inspection confirms the changed page,
and the first 21 extracted-text pages remain exactly unchanged. PDF SHA-256 is
`fc3554c987918d0e24905cbc8ed25303fae1bf28383aed0534eb1cc9e67e7fd8`;
[editable source](solver_routing.tex) SHA-256 is
`e9e21e1fc6acc59cd747cb6cf9ff1933a74251d110c1d8de3a9ef11b81868489`.
Earlier report hashes remain historical in the compact evidence. That snapshot
made no B timing, winner or C qualification claim; the updated report is bound below.

The subsequent fixed Mac B timing campaign at `16774e2e` is terminal **NO-GO
for performance qualification**. Collector and observer both exited zero. The
collector records 8,477.926190333092 seconds excluding preflight, within the
frozen 43,200-second total and 120-second call caps. All 42 warmup units and 504 blocks completed: exactly
468 warmup and 11,232 timed native calls, with no retries or extra observations.
Independent high-effort Astra raw review checks all 904 frozen bindings, retained
logs and clocks, 468 admitted cell-arm fingerprints and prepared inputs. The
original 18 optional Mac unavailabilities remain unchanged. Another monitor was
paused during measurement and restored afterward.

The blocking defect is in frozen `parse_ps`: macOS `ucomm` executable names
retain right-padding, while `known_compute` compares unnormalized names. The
original classifier therefore recognized zero known-compute categories. The
retained 2,555 parsed snapshots contain 3,298,909 process rows, bracket the run,
have no observer errors and a maximum gap of 5.176543 seconds. A separate
whitespace-normalized external audit uses the same declared categories, activity
thresholds, ownership and whole-block overlap policy. It finds 284 active
intervals affecting 102 blocks, leaving 402 blocks and four to twelve per cell.
`near-confounded-third-t1` retains only four and has no finite one-sided 0.025
median upper bound. This audit is descriptive: it issues no alternate host
receipt and cannot retroactively qualify the campaign.

All four candidate frontiers fail both independent descriptive audits. The
original faulty 504-block mask has 43 failed edges for baseline, 47 for h32-k0,
53 for h16-k0 and 53 for h32-k1. The normalized 402-block audit has respectively
51, 55, 59 and 63. Shared-block arithmetic was independently rederived for all
1,326 comparisons per mask; 23,706 numeric interval values match the frozen
pair functions to a maximum absolute difference of 3.197442310920451e-14.
These are conditional, marginal median bounds, not simultaneous or selected-winner
coverage. Balanced ordering and one session do not establish independent block
scores; failed-edge counts are correlated. No causal speed claim follows.

The original frozen analyzer then completed with exit zero: all 1,326 summaries
contain 1,130 passing and 196 failing bounds, with conditional statistical pass
and qualification pass both false. Independent Astra comparison checks every
summary field, coverage, masks and frontiers: 87,516 exact scalar and 58,348
numeric comparisons, maximum difference 5.6621374255882984e-14 (intervals:
2.2537527399890678e-14). The original raw review remains unchanged; a separate
addendum closes its previously pending official-output comparison. The analyzer's
successful host validation did not detect the classification defect and cannot
establish host eligibility; snapshot coverage itself was complete. No B finalist decision or
activation has run, no C case body has been accessed, and public Auto remains
unchanged. The complete campaign, original host receipt, raw evidence,
descriptive analysis and protocol failure remain preserved. Subsequent planning
concerns materially revised algorithms with independent mathematical witnesses
and a prospective parser fix, rather than retrying the current h policies.

A subsequent **prospective observer repair** strips executable right-padding
before classification and retains the exact final field as `executable_raw`.
Internal spaces, arguments, process identities, sampled counters, ownership,
compute categories and activity thresholds remain unchanged. A bounded,
invocation-local cache reuses only command/executable classification, never
activity or eligibility decisions. This does not replace the closed B receipt
or requalify its observations. Independent high-effort Astra review finds no
remaining P1/P2. All 22 host tests, 28 timer tests, 334 benchmark tests and
`./scripts/run_tests.sh` pass; the Linux ELF audit is platform-skipped on Mac.
Production identity remains unchanged (192 runtime modules, 58 artifact inputs).
A passive live parser preflight recognizes the expected compute categories in
1,339 rows, including 761 padded names, with no snapshot error. This checks
parsing, not host eligibility or performance. Source and check hashes are in the
[compact evidence](../bench/results/solver-routing-controlled-20261014.json).

Original `ps` stdout was not captured. Parsed process snapshots and sampled
counters remain bound, but sampling can miss short-lived or first-seen sleeping
work, missing arguments and PID reuse within start-time resolution. The frozen
producer deleted successful numeric outputs and binary columns: retained
fingerprints, column hashes, all logs and source-bound contemporary checks
support the recorded science, without independent post hoc recomputation of
each timed output. The earlier baseline-raw and environment-witness limitations
also remain. Codex app/exec-service activity dominates the normalized inventory;
the census cannot attribute it to polling, file watching or other requests, and
none is retrospectively exempted. Two separately sampled Stata processes were
first-seen/sleeping and did not trigger the frozen activity rule; no extra
exclusions were invented.

The preceding 22-page snapshot of the [routing PDF](solver_routing.pdf) recorded this terminal
NO-GO on page 22; its first 21 layout-preserving extracted-text pages were
exactly unchanged at that point. Compilation and visual review passed for that
snapshot. Its PDF SHA-256 is
`720ba60b563f3df4b96059cb0dfff997954ded41e72857da4aec980a1b41bf2f`
(625,716 bytes); [source](solver_routing.tex) SHA-256 is
`c5eec711d2f8645b461cf107bd69072e8f577bd2f1affb7d3d9ac67b7b583748`.
The preceding science-admission PDF hashes remain historical above and in the
compact evidence.

| Terminal Mac B evidence | SHA-256 |
| --- | --- |
| Frozen 904-binding manifest file | `31faa339679f7dc9103f8af742e02f37a42a84320ed3f34d8b784c47cb8c9938` |
| Manifest canonical identity | `6e200f98a9d6b45a1fef0ff62a3bad7eee0a56b4aaa7241bc6ca754859012ed0` |
| Complete 11,700-call records | `94cd0f911dee3f29448435440b40feef71e97ac70abf9b2492d947a66c7f154c` |
| Complete event stream | `f980496499400a4c9c5e487f7eb26c55eba96a8a8c82435e133de0afad41e19c` |
| Original non-qualifying host receipt | `cc7cd04c5766beee54948b84b8557251a8d3992959e9c890421424a38373ca18` |
| Independent raw review, NO-GO | `4df4aed96150b1fbf6408aa9a9651ad05596c7c19ccb87811618dcbe9ac8f3ac` |
| Original frozen analyzer output, non-qualifying | `ca62d0aadbe2d84d12d740058f39002b0b356726ac4723f94e5c9e7e1d9de5af` |
| Independent complete-output addendum | `e29eb9218453824c8814333ade3a6674433bd9b030aef3b161faa9edba781e24` |

A separate, bounded **construction diagnostic** uses an instrumented archive of
`165d37b4`, not a replacement Mac selection panel. It completed all 40 calls
(five existing B workloads, one/four workers, bounded h16-k0/explicit degree-5
CMG-Schur, first/repeat), with 120-second call and 1,200-second aggregate caps.
All calls passed their independent scientific witnesses; all 20 complete
science/solver fingerprints, output-column bytes and normalized diagnostic
repeats match. No C body was accessed and no retry ran. Mac process/thread
snapshots completed, but isolation is not established and all instrumented
elapsed observations are descriptive only. The SCC scheduler's bounded XML
preflight timed out despite healthy SSH; no new SCC job was submitted.

Independent high-effort Astra review rechecked all 450 bound dependencies, all
40 raw result/column/log hashes, 40 oracle checks and 20 cross-arm/worker
comparisons. All 208 application witnesses are finite, repeat exactly and have
zero scalar/batch discrepancies on the same immutable hierarchy. Successful
bounded and explicit constructions have identical recorded graph, component
and full-preconditioner signatures for weak and dynamic-weight cases. These
streaming FNV signatures supplement the SHA-256-bound raw outputs; they are not
cryptographic equivalence proofs.

| Primary bounded build | Cell loading/sorting/aggregation work | Total admitted work |
| --- | ---: | ---: |
| Weak | 61,682,688 | 106,787,008 |
| Dynamic weights | 79,944,832 | 127,327,112 |
| High-degree weak | 517,686,528 | 855,687,152 |

The high-degree primary build refuses a 64-unit coarse-edge-sort operation with
16 units remaining; its final action refuses a 64-unit local-sort operation
with 56 units remaining. Both retain the original allowance. Each complete
census/emission pair repeats the same canonical cell preparation. This supports
removing that duplicate pass before changing control granularity or production
batching; it does not prove that a revised build will complete. Dense and
wide-RHS h16 calls correctly perform no optional construction. The largest
application-diagnostic admission is 3,231,360 bytes under its separate
128 MiB cap; it is excluded from production memory claims.

The [instrumentation patch](../bench/results/solver-routing-cmg-diagnostic-20261015.patch)
is retained with the [compact evidence](../bench/results/solver-routing-controlled-20261014.json).
Patch SHA-256 is `0e1c969a70c53c35506a35105989c5b8516ba7eb02ab1ab96194f07f49aa4c04`;
summary SHA-256 is `4a3cb3bf8ea290d7ad543e7118b696c0e3f8d47e97518ab016401127a81c4657`;
40-call records SHA-256 is `b2ba5972117e5f63cdad23d7638593902a0131f15afefe85169ccb4d816c5f01`.
Public Auto and the closed B NO-GO remain unchanged.

A subsequent fixed **portfolio-v1 development study** completed all 92 native
calls on seven existing B inputs and one separately generated direct-route
input, at one/four workers. The changed artifact combines strict MAP4 with
seeded LSMR for three or more categorical effects, and a bounded direct-or-CMG
choice with fixed h16/k1 for two effects. It does not change public Auto. The
separate preparation used one generator and one independent oracle call,
zero native fits and 0.718233 seconds under its 180-second cap. The study used
120-second call and 2,400-second aggregate caps, with no retry, extra call or C
body access. Its 178.039089-second interval includes checking and profiling but
excludes the initial guard/import setup and separate input preparation.

Independent raw review rechecked all 564 bound dependencies, the 475-file source
inventory, all 276 result/column/log hashes, 92 independent oracle checks and
46 exact complete science/solver/output-column repeats. Sixteen scoped
cross-artifact comparisons, seven exact unaffected controls, 22 worker
scientific/discrete comparisons and all 46 traced first-call resource checks
passed. All repeat resource inheritances required exact science and output
bytes. The Low-mode and saved-output witnesses also passed.

The terminal status remains **incomplete-route-coverage**. Both workers witnessed
the independent direct certificate at zero iterations, seeded LSMR continuation
and high-degree-weak CMG installation. No early-prefix MAP acceptance or CMG
installation on weak, weighted or wide-RHS was observed. Weak reached checkpoint
16 but refused work at 147,847,160/168,968,192 primary setup units and
59,473,744/67,970,048 final units. The 32W0 ledger reserves 4W0 for restart;
the recorded attempts finish 8 and 48 units below the 28W0 builder allowance.
These aggregate traces do not locate the precise refused operation or prove
cache admission. Weighted converged in 7/6 primary/final iterations and wide-RHS
in 14/13, before checkpoint 16, with only the 1,024-unit preflight charge.
CMG installation on high-degree weak likewise does not prove cache admission.

The [compact evidence](../bench/results/solver-routing-controlled-20261014.json)
records this milestone under `portfolio_development`, including runtime identity
`5cdfeb313776b2fceaedb1a3f8ff6d0ad0dff4749688485dc33e96a16725a23d`,
plugin `2890a103f2e887a1545d379e87d41397ee2e2a3b457e6a09a8424cdce55a55f8`,
92-call records `cb9359a408243b3381da1df8cc9fdd53b5101755cfce365e1cc5d8651cdea569`
and independent review `1d4191d2fe30594772e0cd9d0db11c29d71edc0b6d27194d5e3ad935b348514e`.
Capture hashes identify reviewed temporary evidence; they do not promise a
retained raw archive. All 371 benchmark tests, 1,708 workspace tests and 196
serial FFI tests passed. Four workspace tests remain ignored; all ten serially
ignored FFI allocator audits subsequently passed in separate fresh processes.
Strict workspace and FFI Clippy checks passed. The full repository suite remains pending because the installed native bundle
is stale. No isolation, speed, qualification, activation or promotion claim
follows. Missing coverage stays explicit, permits no extra calls, and cannot
clear a dependent gate requiring that evidence. The original campaign remains
closed with its NO-GO unchanged.

A separate instrumented attribution follow-up remains **FAILED: five of six
native calls** under its fixed six-call, 120-second-call and 900-second inclusive
limits. Four records passed; the fifth, near-confounded-third record remains
failed and unrepeated, and the sixth was never dispatched. There was no retry.
Independent offline review passed all five retained independent-oracle,
exact 92-study science/non-timing-solver/output-column, action-work/route and
resource checks. Only the first four calls form two exact science and normalized
diagnostic pairs. Those source-bound counters support bounded engineering facts;
they do not complete the probe or qualify a performance subset.

Weak directly admitted a 1,680,048-byte cache and performed zero raw emission
reloads. Its completed root has 500 vertices and 8,113 edges. With the unchanged
direct threshold of 700, the grounded sparse factor has dimension 499. Both
actions refused a 128-unit terminal sparse numeric operation, with 8 and 48
units remaining: builder work was 147,846,136/147,846,144 primary and
59,472,720/59,472,768 final. Adding the 1,024-unit caller preflight reconciles
these counts with the earlier setup totals. High-degree weak admitted a
9,760,048-byte cache, also with zero raw emission reloads. Its hierarchy runs
through vertex/edge pairs 1,000/41,992, 798/28,413, 706/22,077 and
555/14,364; builder work completed at 723,206,248/855,686,144. Caller startup
and restart charges remain separate. The 32W0 allowance and 4W0 restart reserve
are unchanged; charged operation units do not measure CPU cost.

The fifth call lacks `seeded_initialize`, `seeded_action` and `seeded_finish`
telemetry because the probes instrumented `solve_prepared_input`, while the
actual `GeneralRoutingState::solve` route uses `solve_windows`, routing sessions
and `advance`. Coarse prefix, LSMR return, reconstruction and certificate phases
were observed. This is an instrumentation coverage defect; the missing durations
are unobserved, not zero. Its offline scientific agreement cannot replace the
missing diagnostic repeat or change its failed status.

The same [compact evidence](../bench/results/solver-routing-controlled-20261014.json)
records this under `portfolio_development.attribution_followup`, binding the
17,512-byte, 439-line
[diagnostic patch](../bench/results/solver-routing-portfolio-attribution-20261015.patch)
(`d0a378522121e2db51b4d413e68df336ed1b53dd9330d4dc582c77aa39351ad7`)
to base code commit `e082f3238bf13e48dd6f1146a00a39a316f727cf` and the same
production runtime identity recorded above. The copied checkout has earlier
documentation; all four original patch inputs match that code commit exactly.
The 47,607-byte records hash is
`58f9e412b8b7242286c4556547e2e356a213aa0d80a1503fa61b87338f6e3e84`;
independent failed-protocol review is
`1d59b53ffef6ec12a7a3edb96f4d78d41bffea92c9849bc181c4895832e63e4e`.
Capture identities promise no retained raw archive. No isolation, speedup,
qualification, public routing, activation or promotion claim follows.

A subsequent private-portfolio source change adds a conservative terminal-factor
screen through published [CMG revision
`3422b0408023769f08f33391b90a613745601e7d`](https://github.com/johannes-schmieder/CMG/commit/3422b0408023769f08f33391b90a613745601e7d).
The separately charged 1,024-unit checked integer bound uses the actual remaining
allowance; nonfit continues existing coarsening before any factor attempt.
Default/explicit, existing controlled/reduced/overlap paths remain unchanged.
Later factor, workspace, memory and startup guards still apply: this is neither
reserved credit nor an installation or performance guarantee. The setup cap,
restart reserve, thresholds, original operator and scientific tolerances stay
fixed; private hierarchies, iterations and rounding may change.

Independent high-effort Astra math/source review passed. Final-manifest upstream
checks pass 115 default and 220 all-feature tests, strict Clippy and formatting.
The bound witnesses cover 1,154 full factors, 9,331 ordered union sequences,
four additional chains and 48 heapsorts. Twelve local/spanning compatible-dipole
cases pass at unchanged `1e-11`. Earlier rounded-matvec failures remain recorded;
unchanged legacy finalization of the same hierarchy produces bit-exact outputs
for all 18 diagnostic applications. These finite witnesses do not establish an
all-RHS floating-point bound. Fereg passes 24 CMG, 18 routing, two independent
portfolio-oracle and two layout tests, plus strict core all-target Clippy,
formatting and the complexity ratchet. An initial 17-pass/one-fail routing result
is retained: its stale cross-policy bit/iteration/work equality contradicted the
declared changed-hierarchy scope. The reviewed test repair keeps the original
independent analytic tolerances, caps, owner reuse and cross-worker checks, and
strengthens exact same-policy repeats. No production workaround was added.

The same compact evidence records these facts and hashes under
`portfolio_development.terminal_affordability`. Current runtime identity is
`097fc6b4f67611e779d916784fe5178033efe6fffa04528e7b2dd62fc117f068`.
The full test entrypoint now passes after a one-line refresh of the separate
fuzz lockfile to the reviewed CMG pin. Its initial stale-lock failure remains
recorded. The gate includes the main 300-test Python suite plus seven additional
Python checks, both strict Clippy modes, independent Rust witnesses, FFI and ten isolated allocator audits, complexity and bridge
checks. The native identity gate remains intact. Public Auto is unchanged.

The first final-universal packaging candidate was rejected before any of its
eight planned study calls. Independent raw-byte review found 21 personal build
paths solely in Mach-O `N_OSO` debug-map records (11 x86_64, 10 arm64); the
original `strings` scan had incorrectly reported success. The original artifact,
build report and unexecuted sealed plan remain rejected. The narrow reviewed fix
uses `strip -S` before universal packaging and signing, retaining the exact
unstripped preimages and ordinary symbols. The maintained audit now scans raw
bytes; 24 binary-audit and 12 packaging tests plus shell syntax pass. The rebuilt universal artifact passes exact non-linkedit section and ordinary
symbol comparisons in both slices, raw-byte privacy, binary audit and ABI/load.
This was a packaging defect, not a scientific failure or qualification result. Compact identities are recorded in
`terminal_affordability.pre_dispatch_packaging` in the same evidence document.

The actual final universal bundle, built from clean source `3f487f214331bc9b87c379fa0a27de6347f9a8f7`,
has SHA-256 `6eeec78a552eede98af1eb1aeb534a63bf156855aa92e32fb75d7ea925f35442`
and 18,464,144 bytes. Its separately admitted fixed eight-call functional study
passes all independent oracles, four exact repeat pairs, cross-worker science
and discrete checks, and resource limits. Both weak and high-degree weak inputs
install CMG at checkpoint 16 with one/four workers. Independent high-effort Astra
review checked every raw output, log and binary column; review SHA-256 is
`c5887373d627e3c701a9845c77e1f3bf1b8341b553053bfd8f227d6c52d6d9b3`.
This observes installation on these inputs without completing native92's missing
coverage or changing failed6. It establishes no speedup or performance qualification.

The corresponding-source archive builds offline from an empty Cargo home with
all dependency paths inside the archive, then passes universal audit and ABI/load.
Its bytes are not asserted identical to the supplied bundle. The matching
[public companion recipe](macos_source_build.md) adds the required Rust path
remaps to the frozen archive's abbreviated command; an unadorned ordinary build
is not claimed passing. Archive and supplied binary bytes remain unchanged.
Current advisory count is zero. Fifty staged licensed fixtures pass. Twenty-one repository-only
fixtures initially fail at checkout-path assertions or missing unshipped test
helpers; their unchanged tests then all pass in the required repository context
against the same plugin. Both OLS and public-scope PPML Break/rollback checks
pass. An initial OLS Break CLI argument error occurred before native execution
and remains recorded. Actual local `net install` succeeds and all 31 installed
payloads match exact accepted hashes. Its admission checker stops before smoke
because Stata lowercases three metadata basenames and writes `PLUS/stata.trk`;
independent review confirms those are the only differences. The failed report is
retained. A separately sealed and independently admitted smoke-only continuation
passes selftest and independent installed OLS/PPML examples, preserving all 31
payloads and the tracker byte-exact. It uses one Stata process, no reinstall or
retry, and retains the original failed admission. [Beta.2](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.2)
is published at distribution tag `1c72e59d25789704157856f4c1df790211cd00c2`,
with build source still `3f487f21`. All 47 public assets match their accepted hashes.
Actual public versioned `net install` (30 flat payloads) and actual installed
`github install` at that full commit (31 root payloads) pass independently in
fresh processes/directories, including exact bytes, selftest and OLS/PPML checks.
Main was then pushed normally to `3f9d2267`. Both the actual unversioned
`github install` and the separate raw-main HTTPS `net install` pass in fresh
isolated Stata processes: 31 exact payloads, selftest, independent OLS/PPML
witnesses and failure-preservation checks. Independent high-effort raw review
clears all installation routes. Later prose-only result commits retain identical
package bytes. No new platform or routing-performance claim follows.
Compact records are in `terminal_affordability.final_universal_functional` and
`terminal_affordability.release_validation` in the same evidence document.

The prospective SCC observer now has an explicitly plan-bound
`qstat-pair-retry-v1` variant. At most two query pairs share one deadline bounded
by 20 seconds and the previous successful pair start plus 75 seconds; no backoff
or budget renewal is allowed. Every exit-zero partial response is semantically
validated, the first valid complete pair terminates the envelope, and all failed
attempts remain replayable. Invalid semantic evidence is terminal. Legacy plans
and outcomes remain unchanged. Independent high-effort review and 93 focused
checks pass. This absorbs brief transport failures while preserving strict
reservation/process/configuration checks; persistent scheduler stalls still
invalidate measurements, with no replacement. A bounded dispatch preflight at
2026-10-16 01:52 UTC confirms SSH access and no new campaign directory, but owner
`qstat` and `omp28` configuration queries each time out at ten seconds. No new
jobs or measurements have been submitted. Two further bounded owner queries
in login and nonlogin shells also time out; identical client resolution and
selected SGE settings do not explain the failure. SSH remains available and the
underlying scheduler-query cause is unproven. Local release and preparation work
continue independently; scheduler responsiveness is required before dispatch.

The current 27-page [routing explanation](solver_routing.pdf) includes the
private portfolio flowchart, frontier table, exact direct-work/cache bounds,
development history and terminal-factor screen on pages 23–26. Page 27 records
the completed fresh SCC discovery NO-GO, paired-ratio table and remaining-RHS
limitation. Public Auto remains unchanged. The original 22-page mathematics
is preserved, and pages 2–26 retain their exact source body and extracted text
after header/footer and whitespace normalization. An initial comparison stopped
on PDF extractor word-spacing differences; the corrected content check passes.
The lead visually checked the updated cover and final evidence leaf, reusing
prior verified unchanged layouts. Independent high-effort fact/math review and
raw evidence review pass; no new full Pro review is claimed. PDF SHA-256 is
`99ce95328c5bb0845280d127bc94d6648cdcc4b475c94bc128cc999cc0dcd4d9`;
[editable source](solver_routing.tex) SHA-256 is
`d68c529e98c14f90bbe872cecfc9d957888c961b5e0771c1cdfa2de2a6c994e9`.
The saved source compiles successfully through the built-in compiler. The older
attribution follow-up remains FAILED at five of six calls, with no sixth call or
retry. The new discovery rejects this candidate; no Mac selection, public
activation or confirmation follows.

Separately, the prospective campaign's independent preparation role completed
12 fixed fresh imported inputs/references: four each for A, B and untouched C.
All headers, output positions, independent full covariance/constant/prediction
and balanced saved-channel gauges pass; source, tool and dependency bindings
remain unchanged. All 12 bodies are unique and disjoint from 63 prior frozen
body hashes across eight metadata files. The fixed 120-second/import and
600-second/panel budgets pass without retry or seed repair, with zero native
plugin calls. C numeric content and targets remain embargoed. These are inputs
and independent references, not complete 21-family panels or native qualification.
The same compact evidence records their reviewed source and metadata identities
under `scc_portfolio_preparation.fresh_import_preparation`.
The full fresh panels now contain 63 distinct model bodies and 126 worker cells.
All 48 categorical references and complete input/freshness checks pass. The
original preparation remains failed: its first reference child could not import
SciPy, before any computation or reference product. A separately authorized and
reviewed reference-only adapter keeps the exact generated bodies, fixes the child
environment and completes the unchanged references once. Prior and continuation
elapsed time total 77.722 seconds under the original 14,400-second budget; no
body regeneration, native call or C disclosure occurred. The later binder must
retain both outcomes explicitly; the original bind is still ineligible. Compact
identities and scope are under `scc_portfolio_preparation.fresh_panel_preparation`.
Independent high-effort review verifies the complete safe metadata, command counts,
source bindings and freshness without reading C numeric files or targets.
A separately reviewed portable certificate binds the exact A data and historical
preparation records for Linux: 79 A payloads and 67 metadata/source/command files.
Its A-only local transport validation passes; B/C numeric payloads remain local
and embargoed. This historical proof does not replace live Linux dependencies,
artifact admission or fresh allocation-local science. At 02:51 UTC, a fresh
bounded check still finds SSH available but both scheduler queries unresponsive;
the new campaign directory is absent and no job has been submitted. Independent
high-effort source review now clears the SCC wrappers and explicit portable Linux
binder. Their actual build/configuration/admission and freeze prerequisites remain
pending. A cleanup-only maintained tooling repair defers an outer alarm through
bounded producer cleanup, restores the handler and marks interruption incomplete;
94 focused tests and the unchanged full entrypoint pass (307 Python checks and
1,927 Rust test executions). Runtime, installed package and prior scientific
artifacts remain unchanged. Setup and collection retain fixed entry deadlines
and cleanup reserves. At 03:04–03:05 UTC the configured scheduler TCP listener is
reachable, but one bounded owner query still returns nothing in 60 seconds. The
scheduler application/protocol cause is unproven; no job submission was attempted.

On October 16, the fresh campaign source is fixed at `cc2aae4d`, with baseline
`46aa78f6` and compiled Mac source `3f487f21`. An independently reviewed 70 MB
shallow Git archive preserves all three exact commits, clean checkout, independent
objects, runtime identities and installed-package identity. Its round-trip and
Git integrity checks pass. The archive and the existing A-only input transport
reached the new `portfolio-v1-20261015` SCC directory after a fresh absence check;
both remote SHA-256 hashes match. No B/C numerical payload was transferred.
The recorded 3 GB full-history bundle is superseded as a transport, without
changing source history or any measurement.
Create-once deployment completes in 40.16 seconds, checking every extracted
file, the exact helper bytes, live tools and numerical modules, the three
tracked Cargo configurations and 17 absent ancestor/Cargo-home configurations.
The separate clean candidate and baseline checkouts retain their exact runtime
identities. A further source-identity and 21-file pending-evidence check passes
in 30.42 seconds. These are file/setup inspections, with zero native calls.
Independent high-effort review verifies the retained remote command captures,
completion record, actual configuration and 21 evidence-file hashes. It clears
the staging milestone only; no numerical payload was opened for that review.

Independent high-effort review clears the final wrapper, six-test Linux binder,
pending admission templates and nine-test create-once deployment helper. The
templates retain null Linux artifact identities and explicitly reuse Mac evidence
only for the source contract. Actual Linux builds, final admission review,
allocation-local science and isolation remain prerequisites. The live inventory
verifies Rust/Cargo 1.85.1, GCC 12.2.0, Python 3.12.4, NumPy 1.26.4 and SciPy
1.14.0, including three Linux numerical-module hashes. At 03:44 UTC, both fresh
ten-second scheduler queries still time out with empty output. No new build or
benchmark job has been submitted, and public Auto remains unchanged.

The scheduler recovered at 09:47 UTC: the actual owner listing and full-host PE
query both passed. The readiness monitor was deleted before continuation.
Separately reviewed setup job `8137493` completed on `scc-gr4` with terminal
accounting `0/0`, 402 seconds wall time and `maxvmem=3.650G`. Both actual Linux
ELF builds pass GLIBC 2.28, export, ABI/load and numeric-guard checks; all ten
commands and 190 focused tests pass. Independent high-effort raw review verifies
the artifacts, logs, tools, source identities, shared release profile and Cargo
configuration. The inherited ABI selftest briefly requests 32 workers for two
small checksum reductions despite the four-slot reservation; setup is not
evidence of a universal four-worker cap, isolation or performance.

The actual resource and sample-1 admissions bind these Linux artifacts while
allocation-local science, observed memory, solver availability and timing remain
pending. The independently reviewed evidence-only binder completes all 42 A-cell
witness packets, with all 46 output hashes verified and zero native calls.
A subsequent display-only log-name error is retained; binding was not rerun.
The first pure seal, `8137604`, stopped at its initial resource guard before
outputs or solver calls, with accounting `0/1` after 19 seconds. One separately
reviewed probe, `8137613`, passed `0/0` and confirmed SCC serial accounting
`slots=1`, `granted_pe=NONE`, with `NSLOTS=1` and absent `PE`. A new wrapper copy
changes only the two pure-role allocation guards and its test reference; all
17 focused tests and independent review pass. The original wrapper, failed
job, builds and binding stay unchanged. Repaired seal `8137651` passes `0/0`
after 54 seconds and freezes the actual 303-file, 42-cell plan. Independent
review verifies the canonical plan, exact design and available raw hash closure.

After separate source/configuration review and 13 mocked dispatch tests, three
collectors were submitted user-held with a dependent zero-native finalizer.
Independent review verifies all accepted IDs, raw responses, unchanged source
hashes, actual requests, user holds and exact dependencies. Their user holds
were released together once. At 10:41 UTC, `8137670`, `8137671` and `8137672`
run concurrently on `scc-zj4`, `scc-wo4` and `scc-yk3`, each requesting a complete
28-core node. Finalizer `8137673` waits on those exact three IDs. This fresh
protocol has one full round per distinct node and zero measurement replacements;
no intermediate timing or ranking was inspected. Complete terminal accounting,
fresh science, strict isolation and independent raw aggregate review remain
required before any new shortlist or Mac selection.

All four jobs subsequently completed with terminal accounting `0/0`.
The three collectors used 6,727, 10,157 and 6,763 seconds on their respective
nodes, totaling 23,647 of the fixed 43,200 allocation-seconds. Finalizer
`8137673` ran for 511 seconds on `scc-tb4`, serial `NONE/1`, after the last
collector ended. The campaign monitor was deleted before inspecting results.
All original measurements, optional failures and stopping rules remain intact;
no measurement replacement or extra observation was submitted.

The complete finalizer reports **portfolio V1 NO-GO**, with a baseline-only
shortlist. Independent high-effort record review verifies all 126 scientific
proofs, 126 checked wrappers, 1,026 passing exact science pairs, stable
availability and protected discrete decisions across hosts, and all 636 ranking
comparisons. Each node has 342 available arm-cells and 18 optional unavailable
arm-cells. The three nodes dispatch 7,236 native calls: 7,182 successes and
54 original optional failures, comprising 47 timeouts and seven iteration-budget
exhaustions. Unavailable arms are not retried or assigned invented runtimes.
The 3,078 successful measured calls provide nine locally paired slots per
available arm-cell; these slots are not asserted to be independent samples.

The lead independently reproduces all 42 portfolio/baseline paired median
ratios. The following point estimates illustrate the mixed frontier; the
[compact result](../bench/results/solver-routing-controlled-20261014.json)
retains all 42, including the nine baseline violations.

| Family | 1 worker | 4 workers |
| --- | ---: | ---: |
| Weak two-FE | 0.669 | 1.010 |
| High-degree weak | 0.773 | 1.122 |
| Dynamic weights | 0.654 | 0.950 |
| Small third FE | 1.108 | 0.994 |
| Four FEs | 1.066 | 0.986 |
| Eight FEs | 1.075 | 1.073 |
| Sixteen FEs | 1.112 | 1.310 |
| Near-confounded third FE | 1.333 | 1.122 |
| Wide RHS | 1.001 | 0.999 |

Ratios below one favor the portfolio. The geometric mean over all 42 cells is
`1.0005864414545023`, combining gains and losses rather than establishing
neutrality. Eight of nine baseline violations involve at least three FEs.
The worst available internal comparison is sixteen FEs/four workers against
LSMR: paired ratio `1.4571265636472241` at allowance `1.2`, giving normalized
score `2.285632818236121`. That score is normalized excess cost, not a
2.286-fold runtime. Both predeclared rejection conditions are met. The protected
baseline allowance stays max(5%, 1 ms), the internal allowance max(20%, 1 ms).
These Linux discovery point estimates are not Mac performance qualification.
The campaign requested full-node isolation; validation requires the complete
reservation, process and scheduler evidence. Native worker comparisons here are
one and four, not 28-worker scaling results.

Independent high-effort review also clears the strict full-node isolation
evidence and complete aggregate. Separately reviewed, zero-native evidence job
`8140245` passes terminal accounting `0/0`, serial `NONE/1`, after 215 seconds
on `scc-tb4`. Its original-source replay reproduces the entire canonical
aggregate exactly, and its raw census checks 4,712 process snapshots and 397
scheduler query envelopes containing 794 successful commands. No transport retry
was used and no known competing compute was detected. Maximum process gap is
5.0033 seconds, maximum scheduler gap 60.0003 seconds, within the unchanged
7.5/75-second bounds. Both owned command groups drain cleanly, without signal,
error or cleanup escalation; no native solver call or measurement was rerun.
Successful numerical arrays and logs were deleted by the original frozen
producer. Their oracle/scoped checks are
source-bound execution attestations; they cannot be freshly recomputed from
missing arrays. The retained records, proofs, resource diagnostics, worker
decisions, failure payloads, isolation evidence and complete aggregate remain
independently checkable. The original canonical aggregate is
`a89d500049863211317fb70ce121968b3c7bde105136ec4859e76a4cc1ef1476`, file
SHA-256 `a177a0b8fdaca394df4224c2e883e32bcdf2f6066d9c00e9378e3f84afe39d40`,
at `finalizer/discovery-shortlist.json` in the fresh SCC campaign.
The final independent review is retained there as
`independent-review/finalraw-review.json`, SHA-256
`9319bcc143e8c32a97f5935f9ae3005ac8f5d9fae09f50efe75789c58be77559`.
Replay completion SHA-256 is
`a848188f24f5e65da8c24be5fe8d792dc4aee5e965277bd274b311bde98c89df`;
the independent raw census is
`a8d01fd754dff09e5d43b763cbfa79efe8a8b008c8c970ec777f5e9cc631c61a`.

The result closes this candidate's route to fresh Mac B, public activation and
confirmation C. Public Auto remains unchanged and B/C remain untouched. The
next engineering questions are the complete strict-prefix/seeded-LSMR cost and
the loss of difficult two-FE gains at four workers. The observed differences do
not isolate MAP sweep cost or establish that serial CMG alone causes the losses.
Current V1 reuses a hierarchy and scales setup allowance with the first
nontrivial batch and physical stride, but retains a fixed iteration-16 checkpoint
and scalar CMG applications. It has no remaining-RHS payback rule; Low mode
normally fixes the allowance on the outcome-only projection before regressor
chunks. Wide-RHS parity does not establish a benefit from always building
earlier. A successor needs a separately declared scope and design, preserving
this failed screen rather than retuning it into a pass.

### Prospective cost attribution after portfolio V1, 2026-10-16

The next operation diagnoses the closed candidate; it does not reopen its
performance screen or use the untouched B/C payloads. Before native dispatch,
freeze an observational source patch against `60f1e241`, the existing universal
Mac plugin `6eeec78a552eede98af1eb1aeb534a63bf156855aa92e32fb75d7ea925f35442`,
the diagnostic arm64 build and the six already revealed A input bodies:
weak, high-degree weak, dynamic weights, small third FE, near-confounded third
FE and sixteen FEs. Both artifacts use the same private V1 policy. The
instrumented build retains release optimization settings; coarse clocks and
trace output change no algebra, solver state layout, work tariff, admission,
tolerance or stopping decision. It is never installed or published.

The fixed schedule has one and four workers, with four complete calls per
cell in control/instrumented/instrumented/control order: 48 native fits maximum,
120 seconds per fit and 1,800 seconds for the complete diagnostic operation.
There are no warmups, replacement calls or opportunistic continuation. A
missing required phase, failed call or exact comparison stops the schedule and
retains the failure. Successful calls must match scientific fields, output
columns and solver signatures exactly across artifacts and fixed-worker repeats;
build identities and clock values are not exact targets. Existing independent
science for the unchanged runtime remains applicable. This is diagnostic
attribution, not new independent scientific qualification or a speed comparison.

Separate bounded-CMG graph, hierarchy and workspace construction from scalar
cycle and gather/scatter costs. For many FEs, cover the actual
`GeneralRoutingState::solve` session path, prefix setup/sweeps/certificates,
session initialization/action/finalization and the outer certificate, including
the final residual action. Verify phase coverage on the first relevant cell
before proceeding. The earlier instrumentation failure in the unused seeded
entrypoint remains failed. No production successor is selected from a missing
phase or from charged-work counts mislabeled as CPU time.

Independent source review identifies an additional confound: V1's strict
prefix/fallback certificate is tighter than the incumbent's MAP early/fallback
contract, although their main LSMR certificates agree. A future removal of the
unconditional prefix must explicitly preserve its declared certificate scope;
it cannot be described as an arithmetic-only change or silently weaken a gate.
Any new candidate requires separate source review and qualification. Public
Auto and the completed SCC NO-GO remain unchanged.

The diagnostic completed all 48 original calls after one reader-only repair.
The first control solve returned success, but the initial reader expected inline
JSON columns while the retained timing harness writes a binary sidecar. That
runner remains failed. Offline validation recovered the complete first result;
a separately reviewed continuation ran only original indices 1--47 under the
original conservative deadline. There was no repeated native call, new time
allowance, replacement observation or changed scientific tolerance.

All 36 comparisons to the first result in each of 12 cells are exact for the
scientific heads, output-column fingerprints and solver signatures. Every call
passed its actual resource caps. Independent Astra high review checked all 48
command records, 113 bindings, 904 source files and 1,572 nested phase scopes.
The [compact results](../bench/results/solver-routing-controlled-20261014.json)
retain the two instrumented observations per cell, separate main/final actions,
all failure identities and review SHA-256
`3ab40849c0a6c1bd48bdca664b7fcfcd57ecb3aece87e2b698ca1a290b160bc5`.
The [observational patch](../bench/results/portfolio-v1-attribution-20261016.patch)
is retained. Successful continuation binaries were deleted after checking;
independent replay verifies retained fingerprints, not missing numerical arrays.

| Instrumented four-worker case | Dominant elapsed phases (ms) | Native total (ms) |
| --- | --- | --- |
| High-degree weak | Graph 114.1; hierarchy 35.3; CMG cycles 16.9 | 277.6 |
| Small third FE | Prefix including setup 29.6; LSMR backend 25.8; session work 34.1 | 119.2 |
| Near-confounded third FE | Prefix including setup 20.7; backend 7.7; session work 16.8 | 61.6 |
| Sixteen FEs | Prefix including setup 168.1; backend 49.0; session work 69.5 | 374.9 |

These are descriptive medians of two instrumented observations, with ordinary
macOS activity and logging overhead. Nested phases must not be added; clocks
are elapsed time, not CPU shares. They neither qualify performance nor identify
the benefit of a code change. The high-degree graph and hierarchy costs barely
change between one and four workers, making construction the larger initial
target. For sixteen FEs, the main prefix precedes LSMR, but the final action
certifies after four MAP sweeps without LSMR. Removing every prefix therefore
has an unresolved tradeoff rather than a guaranteed saving.

### Prospective strict-unseeded causal control, 2026-10-16

Before selecting another routing policy, add one private developer control that
differs from portfolio V1 only by omitting the unconditional categorical MAP
prefix and its seed. Preserve the strict original-design acceptance and fallback
tolerances, full action limit, one setup ledger, retained LSMR preparation,
preconditioner choices, final-action handling and all two-FE/direct behavior.
No geometry pilot substitutes for the removed prefix. Solver choice, iteration
counts and affected floating-point projection/estimation/inference outputs may
change; sample, rank, omissions, factor slots and independent mathematical
targets may not. Public Auto and every existing explicit request remain exact.

After focused mathematical tests, source-only integration and independent
source/build review, a separate fixed 24-call diagnostic compares V1 and this control on the
revealed small-third, near-confounded-third and sixteen-FE A bodies, at one and
four workers, V1/control/control/V1 order. Each arm repeats exactly at fixed
workers; both must satisfy the original independent witnesses and stable model
decisions. Report main and final actions separately. Caps are 120 seconds per
call and 1,800 seconds inclusive; no warmups, replacement calls or continuation
on an inconclusive result. Native dispatch requires actual artifact/input/reader
bindings and review. This resolves whether prefix savings repay its cost; it
does not reopen the failed SCC screen, access B/C or authorize public activation.

The private control passed 18 core, two parser and four independent QR oracle
checks, including exact unaffected paths, strict certificates, saved factor slots,
fixed-worker repeats and cross-worker decisions. Independent Astra high source
review passed. Its first default integration invocation stopped after the Python
checks because the working source no longer matched the installed plugin; no
later integration checks ran in that invocation. This failure is retained.

The experiment therefore remains an exact unpublished source patch; main's six
experimental files were restored to the incumbent after preserving their bytes.
The reviewed explicit `--source-only` mode retains every scientific, Rust, FFI
and tooling check and omits only the installed-package checker, reporting that
whole gate as not performed. It cannot pass installed-package admission or
promotion. Actual experimental build/ABI checks and the declared native witnesses
remain required before the 24-call diagnostic. This prospective clarification
precedes all calls and does not turn the failed default invocation into a pass.

The source-only entrypoint passes six mocked command-flow tests and independent
Astra high review (`0cddfaf4ea103715f52e400ee912ff065446e833e37d2a53c506d51917ad1fb3`).
After restoring main's unchanged incumbent, the normal full entrypoint passes in
223.34 seconds: 255 Python tests plus seven portable checks, 1,927 Rust tests in
86 groups, 14 Rust ignores, exact mathematical witnesses, strict Clippy, allocation
and FFI audits, and the installed-package checker. The Linux linker audit is
correctly skipped on Mac. Main's 192 runtime modules and 58 artifact inputs remain
identical to `c90d03fb`. Default-pass command SHA-256 is
`a71877e378949cfc5108ef7fa7e8ea57907bc994e9fd07dfe5831b891f6ddafa`;
the earlier failed experimental invocation remains
`b3107469694567c825361048c450a2307bd6e0c35537c4e73f96b7a255d0c902`.
These are distinct outcomes; the candidate's source integration, actual build
and 24-call study are separate prerequisites.

The private candidate's source-only integration subsequently passed: 1,933 Rust
tests in 86 groups, 255 Python tests plus seven portable checks, 14 Rust ignores,
and the unchanged mathematical, Clippy, allocation, FFI and complexity gates.
Its first archived-source invocation failed a packaging unit test because the
copy lacked its own Git inventory. Restoring local Git metadata at the exact
base, without changing any of its 906 files, resolved that environment defect.
The successful suite generated a mock executable; the outer inventory assertion
recorded that extra file, which was preserved outside the compiled source tree.
Every source hash remained exact and the suite was not repeated for that assertion.
Installed-package admission remains explicitly not performed for this candidate.

The reviewed observation transform then passed six checks. All 32 actual
release-build, binary-audit, ABI/self-test and numeric-guard commands passed;
independent Astra high review verified 2,447 input bindings and 108 products.
The experimental arm64 plugin is
`eed24ef22a77c2efac778d5d0063629f197d1c5b737eba516f173947cc714941`;
integration/build review is
`e3faba8968fccc5913a69834887977c8c7057afd883f3f1245e231dc2905cea2`.
These checks admit only the declared diagnostic, not installation or promotion.

The fixed study's first solve passed its original scientific and resource
checks, but the reader rejected the post-action `routing_action` summary.
The source emits that summary after the instrumented action scope closes.
The failed runner, plan and result remain immutable. A separate reviewed reader
repair binds each summary to the just-closed action, checks its final flag and
requires exactly one summary, retaining all other positive coverage checks.
It revalidated saved call 0 and executed only the original unstarted indices
1--23, finishing at 22:12:07 UTC within the original absolute deadline
22:28:19.158333 UTC, including its 30-second cleanup reserve. No completed solve
was repeated; the budget, science and original failure are unchanged. This was
an evidence-only repair, not continuation of an inconclusive candidate result.

All 24 retained numerical outputs pass independent Astra high raw review,
including fresh re-execution of their original oracle/model checks, 12 exact
fixed-arm/worker repeat comparisons, cross-worker discrete decisions, resource
caps and positive branch coverage. The review rederives every result row and
checks all 3,272 original and 18 repair bindings. Raw-review SHA-256 is
`2636d8e93f9928468e6faea6e36647bb95bd81c2f85b22f49430ad69d909acf8`;
completed-result SHA-256 is
`b5fd6c41204c3692f1723555b3d96ec00b423afb4d98326f054e57f35ecf4510`.
The [private control patch](../bench/results/portfolio-unseeded-control-20261016.patch)
is retained for reproducibility; it has not been applied to the public runtime.

Median complete native times below use two observations per arm and cell, in
the fixed V1/control/control/V1 order. They are descriptive diagnostic results,
not controlled Mac qualification or a new discovery shortlist.

| Revealed A family | Workers | V1, ms | Strict unseeded, ms | Control/V1 |
| --- | ---: | ---: | ---: | ---: |
| Small third FE | 1 | 263.936 | 224.975 | 0.852 |
| Small third FE | 4 | 120.719 | 105.399 | 0.873 |
| Near-confounded third FE | 1 | 148.963 | 88.277 | 0.593 |
| Near-confounded third FE | 4 | 61.472 | 41.682 | 0.678 |
| Sixteen FEs | 1 | 849.427 | 675.513 | 0.795 |
| Sixteen FEs | 4 | 360.286 | 280.554 | 0.779 |

Removing the prefix increases LSMR session work in small-third and sixteen-FE
cases, but saves more prefix work in these observations. Both main and final
actions improve in all six cells. For sixteen FEs, the final action uses seven
unseeded LSMR iterations instead of four certified MAP sweeps, yet falls from
130.95 to 50.08 ms at one worker and 49.28 to 22.70 ms at four. That final action
reuses the main action's prepared backend; it does not establish the best route
for an isolated cold one-RHS request. The control's descriptive ratios of
one-worker to four-worker native medians range from 2.12 to 2.41.

The diagnostic retains all observations, with no warmups or timing exclusions.
Five-second host snapshots show sustained approximately one-core `suggestd`
activity in both execution windows and no observed competing builds or native
jobs; they cannot exclude short unsampled work. The first cell spans the reader
repair pause. Instrumentation overhead differs with the scopes each route
executes. These limits prevent a release speed claim. The result supports
strict unseeded LSMR as the next general-FE candidate to test across the full
declared domain. It does not resolve the separate two-FE CMG construction cost,
prove a universal winner, or reopen the closed V1 discovery/Mac outcomes.

### CMG graph-stage diagnosis, 2026-10-16

A separate, prospectively fixed eight-call diagnostic splits the graph timer
into census, edge emission and canonicalization. It covers revealed weak and
high-degree weak A cases at one/four workers, with two observations forming one exact repeat pair per cell,
120 seconds per call, 1,200 seconds inclusive and no retries or exclusions.
The [observational patch](../bench/results/portfolio-cmg-stages-20261016.patch)
adds one timer and read-only work/ordinal deltas per reached stage. Arithmetic,
ordering, admission, allocation and cancellation statements are unchanged.

| Revealed case | Workers | Census, ms | Emit, ms | Canonicalize, ms | Whole graph, ms | Hierarchy, ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Weak | 1 | 7.904 | 0.854 | 3.517 | 12.631 | 4.491 |
| Weak | 4 | 7.719 | 0.854 | 3.393 | 12.291 | 4.351 |
| High-degree weak | 1 | 62.883 | 16.639 | 33.722 | 113.663 | 35.382 |
| High-degree weak | 4 | 62.874 | 16.619 | 33.641 | 113.572 | 35.356 |

These are medians of two instrumented elapsed observations, not CPU shares or
qualification estimates. The three children are disjoint; the parent includes
them plus other graph work and must not be added to them. Census is the largest
substage, but includes controlled sorting, compensated merging, cache storage
and component union. Canonicalization includes edge validation, sorting,
merging and allocations; it is not a sort-only timer. Both retain substantial
serial costs at four workers. The stage counts are work/allocation boundaries,
not instructions: high-degree census traverses 5,449,572 boundaries with
277,335,840 charged units. These counts do not identify callback latency.

All eight fresh numerical arrays pass independently rerun original oracle and
model checks. Four fixed-worker pairs are exact, both families agree across
workers with stable discrete decisions, and all resource limits and work counts
pass. Comparison to the old 48-call V1 study uses retained exact column hashes
and scientific records; its deleted arrays were not recomputed. Independent
Astra high review rechecks all 3,071 bindings, eight oracle/model checks, eight
old V1 fingerprints, four exact pairs, all stage ownership and 16 action caps.
Exactly eight calls finished in 83.932 seconds without a retry.

The source transform and 32-command native build/ABI audit pass. The new clock
source reuses the reviewed predecessor's source-only scientific integration;
there was no new full suite or installed-package admission. A preparer metadata
comparison was corrected before native execution. The superseded review still
contains one stale auxiliary test-log hash; the actual reviewed source, tests,
live eight-test pass and final plan are bound separately, and the raw review
retains that discrepancy. No frozen measurement was repaired or discarded.
Eighteen passive host snapshots retain ordinary OS activity, including about
82 CPU-seconds of `suggestd` and 14 of `corespotlightd`, plus the runner's own
scientific checking. This is diagnostic evidence, not an idle-host certificate
or a Mac speed claim.

| Stage diagnostic binding | SHA-256 |
| --- | --- |
| Observational patch | `24c3d17d7291929b143dd5afcc0a492ae066bbf52b1ec4764829efc0a4c5a341` |
| Instrumented plugin | `91af5727a05a3fa6ceb7ea8748029b4e8eebdecdd2080f509b5f965e0a0c0a7a` |
| Fixed plan | `4be88d06500d99d277c7c8d4b45f1c11d7b1fa54d237ea6e54b66ad2ebef41eb` |
| Eight-call result | `9ced797cfaf4919a7f2276cc1cf72a5c372a7251fa7556cd096b4c6a000f8801` |
| Independent raw review | `c33d99f36e1b4fd999029172ffd9219188f3f09dd851f65e266b92565e959dd9` |

The next engineering question is whether optimized synchronous construction
still dispatches an always-Continue callback at each boundary. Removing only
that redundant dispatch could preserve every cancellation check, charged unit,
ordinal and controlled callback. Disassembly is required before treating it as
an optimization opportunity. These observations do not justify a worker-scaled
admission threshold or a new routing policy.

### Synchronous CMG controller diagnostic, 2026-10-16

Optimized disassembly of the preceding binary showed that each synchronous
work boundary still packed a 48-byte descriptor and invoked an external
always-Continue callback. The candidate makes that external controller optional.
Synchronous cached presets use `None`, including when cache admission declines;
controlled, reduced and overlap paths retain `Some`. Every interrupt check,
validation, budget comparison, ordinal increment, work charge and refusal stays
in its original order. The upstream accounting bridge remains. Candidate
assembly confirms that the absent-controller branch skips packing and dispatch,
then reaches the unchanged accounting instructions. This static finding alone
is not a latency estimate.

The prospective comparison used weak and high-degree weak revealed A cases at
one/four workers. Each cell followed baseline/candidate/candidate/baseline:
16 calls total, no warmups, retries, replacements or exclusions, 120 seconds per
call and 1,800 seconds inclusive. Both arms use identical V1 requests and the
same stage instrumentation. The runner reported 446.566 seconds from outer entry, before final observer teardown.
The following are descriptive medians of two observations per arm/cell.

| Revealed case | Workers | Baseline native, ms | Candidate native, ms | Candidate/baseline | Graph, baseline → candidate, ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Weak | 1 | 67.746 | 57.241 | 0.845 | 12.371 → 4.958 |
| Weak | 4 | 43.325 | 31.782 | 0.734 | 12.410 → 4.922 |
| High-degree weak | 1 | 422.487 | 333.654 | 0.790 | 113.514 → 46.359 |
| High-degree weak | 4 | 276.618 | 187.220 | 0.677 | 113.748 → 46.475 |

Both candidate observations are faster than both baseline observations in each
cell. Graph construction falls by about 59–60%. High-degree hierarchy
construction also falls from about 35.4 to 14.2 ms, while CMG cycle time remains
near 16.7 ms. These disjoint setup scopes explain most of the roughly 89 ms
native reduction on that case; graph children are not added again. Removing a
serial cost has a larger proportional effect at four workers. The descriptive
one-worker/four-worker native ratios rise from about 1.53 to 1.78 on high-degree
weak, and 1.56 to 1.80 on weak. This is not four-core linear scaling, a cold
single-RHS comparison, or an estimate of Linux performance against the public
incumbent. In particular, do not multiply these ratios by the earlier SCC ratios.

The runner reports all 16 fresh original oracle/model checks passed, with exact
scientific outputs, column hashes, solver signatures, work and stage counters
within each four-call cell. Cross-worker scientific/discrete and resource
checks also pass. Independent Astra high raw review rehashes all 12,264 binding
entries (3,478 distinct paths), reruns all 16 original oracle/model checks and
verifies four exact four-call cells, eight within-arm repeat pairs, eight
cross-artifact pairs, four family/arm worker comparisons and 32 action caps.
No discrepancy was found. Ninety passive snapshots retain ordinary desktop
activity, including 442.86 CPU-seconds of `suggestd` and 428.32 CPU-seconds of
the runner's own checking. No observed compiler workload or unowned harness
appears; this is not an idle-host claim. These results do not qualify a release
or authorize public Auto activation.

Fresh source-only integration on uninstrumented main plus the exact controller
patch passes 1,928 Rust tests, 255 Python tests and seven portable checks, with
Clippy, arithmetic, transaction, allocation and complexity checks. The focused
builder tests pass all ten tests. The production complexity inventory grows by
ten lines in this one file; no limit or exemption changes. The exact main
inventory is unchanged across the suite. The private diagnostic source then
combines that patch with the previously reviewed unseeded source and reversible
observational clocks: one changed file out of the preceding 906-file snapshot.
Its 32-command native build/ABI audit passes. This is composed validation, not
a fresh full suite on the instrumented tree or installed-package admission.
The earlier proposal to test the instrumented tree failed its prospective static
complexity check before any suite ran; that rejected approach remains recorded.

| Controller diagnostic binding | SHA-256 |
| --- | --- |
| Uninstrumented controller patch | `0b3c7fcd0de32cf3f58397aaddd6eccbb3c51a6ab93b4de3b54759ef25193cc8` |
| Candidate plugin | `fec86539f113d662c3519f75869acbb8e009da873d8d46ca3f713ac0416ad3b2` |
| Actual build and configuration review | `5c3bcf043f54338f8d5d0620bb5f33a5c01984b803fa51663b7b145b5ae55e3e` |
| Fixed plan | `9f2cff088b7d1f1af3acbc2e0212f0d93878e2daf65c5406ef031bcbe517f46c` |
| Actual plan review | `fec552b46472273a30f5675e95747a6cf2c3b98a07cdd23d2582ac72da079294` |
| Sixteen-call result | `39fb3ed4417fe8d3eb08a1dbc645650fbcc1d90b229040e73c1bba11cea9c383` |
| Independent raw review | `93c2edf18582a2369a245b597eb0bbff427925a23bddd40765c747a5ee4a020b` |

The next candidate should combine this construction change with strict unseeded
LSMR for general categorical FEs, retaining the existing two-FE constants and
public incumbent. A genuine combined-source integration and fresh full-domain
discovery are required; the separate diagnostic successes cannot substitute for
them. Remaining setup cost and RHS reuse warrant measurement, not new
worker-dependent thresholds fitted to these four cells. Old V1 discovery and
Mac B outcomes remain closed NO-GO; no new B/C has started.

The combined uninstrumented source is now integrated as an experimental path.
Independent Astra review verifies both exact patches and their interaction.
A fresh `./scripts/run_tests.sh --source-only` passes in 144.662 seconds:
1,934 Rust tests in 86 groups (zero failures, 14 ignored), 255 Python tests and
seven portable tests, with the mapped scientific, transaction, FFI, allocation,
arithmetic, strict-C and complexity checks. All 908 tracked files remain exact
across the suite. Two child-process summary prefixes are interleaved in the raw
stdout; all 86 numeric summaries are retained and counted. The normal ratchet
records +25 core and +6 plugin production lines, without changing any limit or
exemption. Source-review SHA-256 is
`dc4f5bc7a6bbca6f43daa7d48a6c6e75d3714fb7b36d7921d12e1b479e64755b`;
command SHA-256 is
`dd5279ea2d643bb1173a6d2e86ab5a0802c3e7da9a36d68b767df47400ae592a`.
This source-only suite deliberately does not admit the installed package. The
combined uninstrumented native build, fresh broad discovery and Mac B/C remain
pending; no speed qualification or public default activation follows.

### Prospective combined-candidate discovery on six SCC nodes

The next experiment compares the integrated `portfolio-unseeded` candidate with
the same qualified baseline and complete mathematical comparator frontier. It
reuses the 42 revealed A cases (21 families, each with one and four workers),
not their prior availability, science, timings or shortlist. This is a new
discovery experiment on known cases, not an untouched confirmation set. Old V1
discovery and Mac B remain closed NO-GO. Public Auto is unchanged.

Each of three logical rounds has two fixed family partitions. The first ten
families in canonical sorted order form partition 1; the remaining eleven form
partition 2. Both worker counts and every solver comparison for a family stay
within one allocation. The six allocations must use distinct physical hosts
and establish full-node isolation, not merely reserve 28 slots. Each cell still
has three host repetitions; its nine measured slots are not nine independent
replicates. Splitting families changes cross-family host covariance, so a logical
round must not be treated as one shared host in an uncertainty calculation.

| Fixed partition | Families / cells | Arm-cell slots | Native-call maximum | Inclusive allocation budget |
| --- | --- | --- | --- | --- |
| 1 | 10 / 20 | 182 | 1,274 | 7,280 seconds |
| 2 | 11 / 22 | 178 | 1,246 | 7,120 seconds |

Every available arm has two fresh prerequisite calls, a traced call, one warmup
and three measurements. The complete design retains at most 7,560 calls and
43,200 allocation-seconds, including cleanup; no extra sampling budget comes
from doubling the node count. Per-call limits remain 120 seconds. The split
depends on family identities and arm counts, never observed speed or
availability. There are no measurement replacements, reassigned partitions or
partial aggregates. All six terminal accounts, application completions,
scientific certificates, exact local repeats, cross-host discrete decisions and
isolation evidence must pass before scoring the complete frontier. The existing
baseline-violation and worst-normalized-ratio rules still stop at baseline only.

Cross-worker agreement concerns scientific decisions: sample and drop decisions,
rank/omission metadata, recovery markers, presence flags and discrete degrees of
freedom. Solver-route differences are retained separately. Worker-dependent
workspace admission can legitimately select LSMR or MAP, and Low-mode chunks
can aggregate warning and pilot metadata differently. Those execution fields
are not a universal equality requirement across worker counts. Existing stricter
CMG/IT contracts and fixed-worker, cross-host checks remain in force.

The typed policy, partition producer and validators pass independent Astra
high-effort source review. The full benchmark suite passes 413 tests in 171.867
seconds, with all 48 source/test files unchanged throughout. An earlier run
retains ten environment errors (eight missing SciPy, two missing psutil); using
the existing task dependencies resolves them. All 192 production modules and
58 artifact inputs remain identical to runtime commit `dd9d4286`, so its valid
source-only runtime suite is reused. These tooling checks do not establish
Linux performance. The design remains prospective pending the actual Linux
build, ABI checks, input binding, pure seal and dispatch.
Scheduled setup uses four slots and explicitly caps the actual native ABI
checksum worker request; library environment variables alone cannot enforce
that cap. Fresh guarded Mac B12 and untouched C24 remain conditional on a valid,
independently reviewed new shortlist.

The frozen source is now `020eab1e9559fd68f619a4ec73fb04fcb6d1136f`.
Private universal Mac builds of that source and baseline `46aa78f6` pass their
audits and four-worker host ABI checks with identical compiler and release
profiles. Independent Astra review verifies the source/dependency closure,
the baseline's two test-only harness overlays and unchanged installed artifacts.
The candidate plugin is `2c85d890`; the fresh baseline is `b49d0471`.
The baseline retains 21 debug-path occurrences under its original build script;
it is a private comparator, without current distribution admission. Neither
build has run the new scientific or timing panel.

Linux setup `8151673` built both plugins and the timing harness, passed both
ABI/numeric-guard checks and 218 focused benchmark tests, then failed its final
mock-C compilation. GCC 12.2 at `-O0` conservatively allows negative integers
in a formatted test argument and rejects a potentially truncated 16-byte
buffer. Accounting is failed=0/exit=1, 431 seconds on `scc-ei3` with four slots;
there is no completion marker. The narrow test-only repair enlarges two buffers
to 32 bytes, preserving all supported worker arguments and compiler warnings.
Its six local mock tests and actual four-worker ABI checks against both private
Mac plugins pass. The original failed setup and successful builds
remain intact. The separate continuation `8151852` then exited failed=0/exit=1
in 31 seconds before running any mock tests: its isolated fixture omitted
`replay.h` and the nested `replay_io.h` dependency. The complete fixture passed
six local tests, but Linux continuation `8151966` exited failed=0/exit=1 in
34 seconds after running all six: the shell worker-count guard accepted an
Arabic digit and reached the deliberately nonexistent compiler. A separate
read-only Bash 4.4.20 check reproduces range acceptance under `en_US.UTF-8`;
explicit ASCII digit lists reject it. This establishes a test-harness portability
defect, not a solver failure. All three failures and the successful Rust builds
and ABI checks remain retained. Locale-independent admission and a focused Linux
retest are still required; there is no setup completion marker.
The explicit-ASCII shell repair passes six local tests and independent Astra
review. The complete continuation passes 15 focused validation tests; its
admission assembler passes four. Submission stops before deployment or `qsub`
when full Git source status exceeds 90 seconds. A bounded diagnosis also times
out on tracked-only status and complete untracked-file enumeration. Source
readiness is unverified; no new job or collection has started. One read-only
30-minute readiness monitor is capped at sixteen probes and stops by
2026-10-17 09:30 UTC. Recovery still requires a fresh reviewed source check and
actual Linux success; the monitor does not waive either gate.
The first scheduled probe subsequently recovered at 01:56 UTC: exact source
identity and full porcelain status, including untracked files, passed with no
locale submission claim or output. The monitor was deleted. Independent Astra
review cleared a separately recorded submission continuation, and SCC accepted
job `8152429` at 02:01 UTC. Its scope remains the affected-locale witness and six
mock tests, reusing unchanged builds. It completed successfully on `scc-pi2`
with four slots in 138 seconds: failed=0/exit=0, the affected-locale witness
passes and all six mock tests pass. The completion marker and every expected
output are present. Independent Astra raw review clears the unchanged builds
and source-level resource/sampling contracts, verifying 234 local references
and distinguishing ten remote tool/build-manifest references checked by the
producer. Fresh allocation science remains pending. Initial pure assembly
`8152811` failed before publishing any file because its raw review referenced
a retained capture omitted from transfer. A full 244-reference audit found
exactly that missing file and no changed references. After copying its exact
bytes, separately reviewed continuation `8153140` passed failed=0/exit=0 in
124 seconds and created all four admission/config files. Independent review clears
all four files and their unchanged 246-reference evidence set. Pure input binder
`8153173` then passes failed=0/exit=0 in 125 seconds, preparing 42 A cells with
46 verified metadata files and zero solver calls. Independent binding review
clears the unchanged pure seal. Job `8153203` then passes failed=0/exit=0 in
126 seconds on a serial single-slot allocation and freezes 447 file bindings.
Independent Astra review verifies both canonical plans, all 447 bindings,
the fixed partitions and actual dispatch configuration. The held dispatcher
submits collectors `8153231`–`8153236` in round/shard order and dependent
finalizer `8153237`. Separate raw review verifies all seven job identities,
resources, arguments and dependencies, 31 claim/config records and nine
dispatch bindings. One `qrls` releases only the six collectors after fresh
user-hold checks. All six subsequently ran on distinct complete 28-core hosts
and finished failed=0/exit=0 within their declared budgets: 3,901 / 3,958 /
5,927 / 2,505 / 3,915 / 2,472 seconds in round/shard order. Finalizer `8153237`
finished failed=0/exit=0 at 04:40:08 UTC on 17 October, after all collectors,
in 459 seconds. The campaign monitor was deleted after complete terminal
accounting. No measurement was replaced or rerun.

The complete aggregate returns **baseline only**. Its candidate has one baseline
violation: high-degree weak at four workers has a paired median ratio of
1.1510, above the 1.05 allowance. Within-host median paired ratios are 1.1613,
1.1510 and 1.1280, so the regression appears on all three hosts. The worst
internal comparison is weak at one worker against sampled CMG-k2: ratio
1.4262 versus the 1.20 allowance. Normalized excess 2.1311 is
`(1.4262 - 1) / 0.20`, not a 2.131-fold slowdown. Sixteen internal-comparator
comparisons fail; these are not sixteen distinct failing cells.

There are useful descriptive gains. Across all 42 cells the geometric mean
candidate/incumbent ratio is 0.9388. Weak two-way ratios are 0.6452 / 0.8824 at
one/four workers; dynamic weights are 0.6332 / 0.8847. Small-third and four-to-
sixteen-FE cases improve by about 9--17%. Wide RHS remains near parity at
1.0072 / 1.0076. Those cells finish before the upgrade checkpoint and do not
identify CMG amortization. All observations remain included. The lead's
independent calculation reproduces every aggregate ranking field from all
3,078 measured observations. Three hosts per cell are the independent units;
the frozen discovery score's nine paired slots are not nine independent
replicates. These point estimates neither prove Mac performance nor admit B/C.
Independent Astra high raw review verifies all seven terminal accounting records,
126 checked scientific proofs, 1,026 passing exact science pairs and all 3,078
measured observations. All 54 original optional first-call failures are retained
(40 timeouts, 14 iteration-exhausted), with the same 18 unavailable arm/cell
combinations each round. Actual dispatches total 7,236: 7,182 successful and 54
failed; carried timing-side unavailability is not a second dispatch. All 4,502
process snapshots, 384 scheduler envelopes (768 commands), reservation records
and terminal observation markers pass isolation replay, with no scheduler retry
or detected competing compute. Both independent scoring rederivations reproduce
all ranking fields. The review checks the retained proofs, fingerprints and
resource records; successful output arrays and transient logs were deleted by
the original producer, so it does not claim a fresh oracle recomputation from
those arrays. This completed, reviewed discovery establishes a performance
NO-GO for the frozen candidate, not a new passing shortlist.

Source review confirms the earlier synchronous no-controller optimization is
already active in the candidate. The handoff preserves the first 16 iterations'
solution progress and restarts conjugate recurrence state. The remaining bounded
builder stages edges, uses serial controlled sorting and performs genuine
fine-grained interrupt, budget, ordinal and work checks. Explicit CMG uses a
different executor-based construction path. On high-degree weak, the candidate
charges identical setup work at one/four workers and its first setup remains
about 160 ms in the first host's traced prerequisites; final hierarchy reuse
charges zero setup work. These observations do not isolate callback cost.
The subsequent static-dispatch diagnostic below preserves arithmetic, every
boundary and failure semantics, but demonstrates no material complete-fit gain.
No family-specific or worker-count threshold is inferred from this screen.

### Bounded CMG static-dispatch diagnostic, 2026-10-17

A private candidate specializes the upstream graph-construction meter and uses
a concrete fereg callback adapter. The actual ARM64 graph path calls
`Work::gate` directly; the retained parent has an indirect callback at that
boundary. Hierarchy construction keeps its dynamic meter. Upstream CMG commit
`b4ca4ad3fd5d7d783be7feaef5cb62a81019017d` passes 118 default and 223 all-feature
tests. Parent and candidate produce the same 77,090-byte controlled-construction
witness, covering 611 boundaries plus refusals and invalid inputs.

The private fereg candidate passes its universal native audit/ABI and explicit
source-only integration suite: 1,935 Rust test executions, 312 Python tests and
seven portable checks. The first source-suite attempt exposed the stale fuzz
lockfile; that failure is retained. The corrected candidate passes the complete
suite. Installed-package admission and release qualification were not performed.

The fixed Mac diagnostic executes all 40 calls in 37.508 seconds: eight traced
prerequisites/warmups followed by one ABBA and one BAAB block per cell. Every
call retains the original independent numerical targets, exact scientific
bits and solver signatures. Setup work, route and iteration traces are exact;
all four cells install CMG at the unchanged checkpoint. Independent Astra high
review rechecks all 200 raw artifacts, all 40 oracle evaluations, 46 bindings,
eight resource traces, 80 host snapshots and every paired-summary field.

| Revealed A cell | First block ratio | Second block ratio |
| --- | ---: | ---: |
| Weak, one worker | 1.03598 | 0.99938 |
| Weak, four workers | 1.00386 | 0.99461 |
| High-degree weak, one worker | 0.97550 | 0.99478 |
| High-degree weak, four workers | 0.99066 | 1.00957 |

Ratios are candidate/parent geometric means within each balanced block. All
observations remain included, with no retry or exclusion. Endpoint snapshots
record ordinary desktop activity and no identified competing build, test or
estimation workload; they do not prove uninterrupted whole-call isolation.
Two blocks per cell do not establish statistical equivalence or nonregression.
The high-degree four-worker result is essentially neutral, so this intervention
does not resolve the closed SCC rejection. The fereg adapter and dependency
pin were removed from main; the private experiment and published upstream API
remain available. Public Auto and the incumbent artifact are unchanged.

The subsequent bounded diagnostic profiles the established parent under
automatic upgrading and explicit CMG-k1. Historical substage clocks implicate
census/local sorting and genuine work checks as well as global canonicalization;
the static experiment isolates only indirect dispatch. Full artifact, protocol,
output and review hashes are in the compact controlled-results evidence; no
release speed claim is made.

### Local CMG work-gate inlining, 2026-10-17

Two `inline(always)` annotations on the bounded builder's local work gate and
charge wrapper remove repeated out-of-line calls from the two hot sift loops
and census. Actual ARM64 assembly preserves cancellation, validation, budget,
ordinal, optional-controller and commit ordering. Arithmetic, sorting, work
tariffs, sampling, h16, memory admission and refusal boundaries are unchanged.
The upstream dynamic meter remains. Enclosing frames grow and some spills
remain; ARM64 text grows by 20,920 bytes (about 0.36%).

The fixed Mac comparison uses the same revealed weak/high-degree weak cases
and one/four workers as the preceding static-dispatch diagnostic. Eight traced
prerequisites/warmups precede 32 clean calls, one ABBA and one BAAB block per
cell. All 40 calls finish in 37.473 seconds, within the fixed 120-second call
and 1,800-second inclusive limits. No retry, replacement or exclusion occurs.

| Revealed case / workers | Block 1 ratio | Block 2 ratio | Geometric mean ratio | Parent → candidate native, ms |
| --- | ---: | ---: | ---: | ---: |
| Weak / 1 | 0.97021 | 0.96653 | 0.96837 | 58.976 → 57.110 |
| Weak / 4 | 0.94485 | 0.95704 | 0.95092 | 33.131 → 31.505 |
| High-degree weak / 1 | 0.95563 | 0.94915 | 0.95238 | 366.245 → 348.806 |
| High-degree weak / 4 | 0.93351 | 0.93607 | 0.93479 | 215.706 → 201.640 |

Ratios are candidate/parent; displayed native times are geometric means of
the four clean observations per artifact/cell. Both blocks favor the candidate
in every cell, with observed reductions of about 3–6.6%. Two blocks per cell
are descriptive, not a confidence bound, broad nonregression qualification or
release-performance claim. These Mac ratios cannot be multiplied into earlier
Linux outcomes.

Independent Astra high review rehashes all 97 bindings and 200 raw artifacts,
recomputes the original numerical witnesses on all 40 retained outputs, and
verifies exact science, repeats, solver signatures and eight traced resource
checks. Setup charges, h16/k1 installation and iteration counts stay exact;
the final action charges no new setup. All 40 owned captures finish cleanly.
Eighty endpoint snapshots retain ordinary desktop/indexing noise without
exclusion; they cannot establish continuous whole-call isolation.

Focused admission-order and isolated cancellation witnesses pass. The full
source-only suite passes 1,938 Rust executions, 312 Python tests and seven
portable checks; the private universal build passes binary audit and actual
plugin ABI/load checks. Four build-tool and 14 diagnostic-tool tests pass.
The complexity inventory adds only the two production annotation lines and
changes no limit or exemption. Installed-package admission was not performed;
the Linux mock audit remains unavailable on Mac.

Retain the two annotations within the experimental workstream and close this
diagnostic without further annotation tuning or sampling. Full workstream
qualification remains pending. Public Auto stays incumbent; no new SCC
shortlist, Mac B/C admission or reopening of any old failed campaign follows.
The compact evidence records the complete protocol, all block observations,
source/build/review hashes and these limitations.

| Inlining diagnostic binding | SHA-256 |
| --- | --- |
| Exact source patch | `df9be06d90fa3d6cdb3b20bb0d3eaf8480c30e2dc49925949fae79b8496067b4` |
| Candidate plugin | `c39553f948a9250019376e6c24a8d8cee2208f59d711c4f8fc66c5c22a646384` |
| Assembly review | `d72b3842b885bc0944c45eeba86df609822b222d2249e7000f19d132bf86b801` |
| Fixed plan | `5715b049c443838f692127a17bb3681a810d7458ea741a51c09c151c103d2dba` |
| Forty-call result | `642fc517cae5dc0ecd42e223cd82d303eb506b67bd275ae13bd342874644f21b` |
| Independent raw review | `4f37f6282428233c9973acfbb57fe6ac996ad78b168f5c373c6b171d4839bc60` |

### Integrated candidate Linux qualification, 2026-10-17

The new campaign freezes source `691cef5c` against baseline `46aa78f6`.
It keeps the simpler CMG gate-inlining implementation and strict unseeded LSMR;
the discarded batching and heap-credit prototypes are absent. Setup job
`8155921` passes with failed=0/exit=0, four slots and 416 seconds on `scc-gr4`.
Both same-profile Linux ELF artifacts pass audit, actual-plugin ABI/self-test,
and input/output guards. The 218 benchmark-tool tests and six harness mocks pass.

Independent Astra high review checks all sixteen substantive commands, twenty-five
Git probes, source/overlay identities, both binaries and the harness. Its 167
references include 159 corresponding local files rehashed independently and eight
remote tool/numerical identities retained from the producer's evidence. This is
build and source-contract admission; fresh allocation science and isolation
remain pending.

The build names the host `scc-gr4`; accounting uses `scc-gr4.scc.bu.edu`.
A separately reviewed consumer adapter accepts only that exact SCC short/FQDN
relationship and delegates every other check to the unchanged producer validator.
All 27 pure tests pass. Original setup, configuration, binaries and raw accounting
remain unchanged; no rebuild or solver retry is used.

Evidence assembly `8156079` passes in 49 seconds and pure input binding `8156092`
passes in 79 seconds, both with failed=0/exit=0. The binder admits 42 A cells and
46 metadata files without solver calls. Original seal `8156129` fails before
outputs because its launcher selects a Python symlink and the unchanged producer
requires the exact executable. Separately reviewed launchers use the already-bound
Python 3.12 binary and explicit C/UTC; 13 mock/structural tests and three shell
syntax checks pass. The original launcher and failure remain preserved. Pure seal
continuation `8156149` passes with failed=0/exit=0 in 80 seconds, serial NONE/1.

Independent Astra high review closes all 389 frozen bindings: 381 local files
rehashed with zero mismatches and eight unchanged setup-attested tool/library
identities. The actual plan is schema 3, canonical `ea8251ec`, file `cbe0bed0`.
Collectors `8156167`–`8156172` and dependent finalizer `8156173` are accepted.
Independent review checks all 31 dispatch records, nine bound inputs and seven
actual job identities, resources, arguments and dependencies. Exactly one release
of the six user holds succeeds after fresh target-hold and source checks.
Only lifecycle status was inspected while any job remained active. All seven
jobs now have complete failed=0/exit=0 accounting. Collector wall times are
5,943/2,604/3,919/2,521/3,917/2,517 seconds, totaling 21,421 against the fixed
43,200-second allocation budget; finalization takes 422 seconds. The monitor was
deleted after terminal accounting. All six original isolation validators and
the full original aggregate replay pass independently; the replay reproduces
every result value exactly. Fresh science includes 1,026 passing pairs with
exact local repeats and 54 retained optional unavailabilities across three
rounds. Cross-host availability and model discrete decisions remain stable.
Independent Astra high review checks 4,254 process snapshots and 724 successful
scheduler commands, with no observer errors or observed competing compute.
This establishes the declared sampled isolation contract, not continuous
visibility between samples.
Review rechecks all 126 retained scientific proofs, fingerprints and resource
attestations. The frozen producer deleted successful arrays and transient native
logs, so this is validation of retained proof evidence, not fresh numerical oracle
recomputation. The 7,236 distinct native dispatches remain within the 7,560 cap;
54 timing prerequisite rows reuse earlier unavailable proofs without new calls.

**The completed discovery returns baseline only.** All 42 aggregate incumbent
comparisons pass their original allowances. The geometric-mean candidate/incumbent
time ratio is 0.931181: 0.914859 at one worker and 0.947795 at four. These are
descriptive comparisons with the incumbent, not scaling factors or confidence
bounds. Selected ratios are:

| Family | One worker | Four workers |
| --- | ---: | ---: |
| Weak | 0.625740 | 0.843512 |
| High-degree weak | 0.745664 | 1.038499 |
| Small third FE | 0.885158 | 0.831239 |
| Four FE | 0.874437 | 0.839234 |
| Eight FE | 0.878793 | 0.858202 |
| Sixteen FE | 0.900295 | 0.897430 |
| Dynamic weights | 0.616790 | 0.859155 |
| Wide RHS | 0.995911 | 1.009576 |

The fixed internal-comparator screen rejects 14 comparison edges across seven
cells. The worst is weak/one-worker against explicit CMG k2: time ratio 1.373446
against allowance 1.20, hence normalized excess `(1.373446-1)/0.20=1.867231`.
This is a 37.3% time penalty, not an 86.7% penalty. Gaps also remain against MAP
for small-third/four-FE at four workers and against explicit CMG on other weak
cases. The aggregate high-degree weak/four-worker incumbent comparison passes,
but its three host-level ratios are 1.043, 1.076 and 1.014; this is not a pass
on every host. Weak/one-worker loses to CMG k2 on all three hosts.

Within each host, dividing the candidate's median one-worker time by its median
four-worker time gives 1.226–1.566 for weak, 1.435–1.623 for high-degree weak,
1.657–1.807 for wide RHS and 1.780–1.987 for four FE. Each median uses three
measurements under the fixed producer order. These are descriptive scaling
observations, with three independent hosts per cell and no confidence claim;
the nine timing slots are not nine independent replications.

The trial is closed NO-GO under the unchanged screen. No measurement replacement,
partial aggregate, new Mac B/C admission or public Auto change follows. Older
campaigns remain closed; differences between campaigns do not isolate the causal
effect of gate inlining. Exact artifact, accounting, review,
launcher and dispatch hashes are in the
[compact results](../bench/results/solver-routing-controlled-20261014.json).

The first submitted RHS width and physical stride determine the fixed work allowance and
an installed CMG hierarchy can be reused. The route does not yet explicitly
compare construction, transition and extra certification cost with expected
savings over remaining compatible columns. Future columns provide a possible
amortization horizon, not evidence of savings or additional work allowance. This
panel evaluates one/four-worker performance; the separate matched-RHS regression
establishes correctness and reuse, not wide-active-RHS payback.

### Matched independent RHS coverage, 2026-10-17

The maintained routing regression now uses sixteen independent residual columns
on the same paired, weighted, two-component graph. Fixed dyadic FE signals and
Walsh residual patterns preserve the original first column exactly; equal-weight
opposite pairs give an independent analytic projection for every column. The
formulas and five layouts were fixed before the first passing test, without
tuning after observing coverage.

At one and four workers, two exact repeats cover cold widths 1, 4 and 16,
reused width 1, and width 16 after a one-column priming solve: twenty owners and
twenty-eight projections. Every prime and target passes the original residual,
reconstruction, saved-FE contrast and certificate checks. Installation markers
and subsequent recurrence progress establish CMG application; retained hierarchy
identity and setup work remain exact on reuse. Capacity is always sixteen, while
the first submitted width determines the unchanged 32W0 allowance and 4W0 restart
reserve. Later wide requests do not renew that allowance.

Independent Astra high review passes. Fixed-worker scientific bits repeat exactly;
cross-worker science and discrete decisions agree. This is correctness and route
coverage, not evidence that all sixteen columns remain active at handoff or that
wide solves are faster. Production tokens and artifact inputs are unchanged.
Public Auto remains incumbent; no SCC shortlist or Mac B/C admission follows.
Source, patch and first-test hashes are retained in the
[compact results](../bench/results/solver-routing-controlled-20261014.json).
The integration run passes 1,939 Rust executions, 312 Python tests and seven
portable checks, including strict lint, FFI allocation, mock-SPI and numeric
guards. It uses the documented source-only entrypoint; installed-package
admission is not performed and the Linux mock audit is unavailable on this Mac.

### Local heap-step credit diagnostic, 2026-10-17

A separate private prototype grouped cancellation checks within a local heap
sift. Eligibility required synchronous census mode, the bounded terminal policy and no
external controller. After the original first-step gate, permission covered at
most 15 further steps, capped by remaining work and ordinal capacity. Each
executed step still charged 64 units and one ordinal before the unchanged
comparison/swap body. Unused permission was discarded without charging it.
The declared change was cancellation polling, bounded by 16 steps/1,024 charged
computational units, not elapsed time. Controlled callbacks, successful work,
scientific bits and non-cancelled refusal prefixes stayed exact.

Independent Astra high source review and boundary witnesses pass. The source-only
suite passes 1,942 Rust executions, 312 Python tests and seven portable checks;
the private universal build passes audit and ABI/load checks. Actual ARM64 code
bypasses the intended checks on credited steps; both local sift frames shrink
from 208 to 192 bytes, while total text grows by 208 bytes. This establishes the
mechanism, not a speed gain. The only failed focused check was a new test's
incorrect assumption that a tighter terminal-policy budget must refuse; that
policy can choose cheaper coarsening. The corrected test compares exact old/new
outcomes at each allowance. No production or tolerance repair was needed.

The separately frozen diagnostic ran exactly 40 calls: eight traced
prerequisites/warmups followed by two balanced ABBA/BAAB blocks per cell, on
revealed weak and high-degree weak A inputs at one/four workers. All calls
completed in 39.730 seconds, within the original 1,800-second budget and
120-second call limits. No retry, replacement or exclusion occurred.

| Complete native-time ratio, candidate/parent | Block 1 | Block 2 | Geometric mean |
| --- | ---: | ---: | ---: |
| Weak, 1 worker | 0.99866 | 1.03170 | 1.01505 |
| Weak, 4 workers | 0.96241 | 0.99742 | 0.97976 |
| High-degree weak, 1 worker | 0.95953 | 0.97899 | 0.96921 |
| High-degree weak, 4 workers | 1.01702 | 1.06717 | 1.04180 |

Independent raw review checks all 40 outputs directly against the original
oracles, exact science/signatures/repeats, work/resource traces and cross-worker
results. All 149 plan bindings, 350 unique references and owned process outcomes
verify. Eighty endpoint snapshots retain desktop/browser/indexing activity;
they do not prove continuous isolation. All observations remain in the compact
controlled-results record.

**Decision:** discard this prototype and restore the exact `bounded.rs` and
complexity inventory from `016cb804`. The high-degree/four-worker case is slower
in both orientations, and the mixed evidence does not justify the additional
cancellation contract and 15 production lines. This is engineering triage, not
a statistically established regression or a formal nonregression verdict.
Do not add a worker-specific exception or expand the completed panel. The
earlier gate-inlining implementation remains. Public Auto, closed SCC outcomes
and Mac B remain unchanged; integrated qualification is still pending.

| Evidence | SHA-256 |
| --- | --- |
| Prototype patch | `67ba43671e43b860690359edab953114dcaa6d10697deb135d20a1e78d23e9bd` |
| Candidate plugin | `e61bbb29ab3008d33f2e08d9ab558bf72d867a17f08e50604cba52cb4c019d6e` |
| Fixed plan | `4cd65c10aaccaec3f3a303461b50a23a6fb10649930f2fb08a48a7358a35ef83` |
| Forty-call result | `fb366c8075429f0b44869323b5d298ab3e130709ed9ccc40b1008035916b24f5` |
| Independent raw review | `c4bd78b5a9c26a2549e25a9b20e0c6623509c5484c77c9062161d842ce66b497` |

### Serial CMG batch application diagnostic, 2026-10-17

A private prototype used the actual submitted RHS count to select a serial
batch workspace of at most four columns, after constructing the unchanged
hierarchy. Single-RHS calls kept the scalar path. The installed width stayed
fixed across reuse; live-column compaction did not change the arithmetic path.
Admission covered the scalar and batch workspaces, widened banks, component
scratch and initial application work. A selected construction failure had no
scalar retry or work refund. This affected private resource charges and
near-cap refusal behavior, not model semantics, tolerances, h16, sampling or
public Auto. The prototype added 160 production lines and two dispatch sites.

Upstream CMG `d1cd681d1065a6440d8c4359ddf8f10e43f93e73` supplies the reviewed
controlled batch-workspace API. Its scalar migration witness preserves all
3,763 boundaries exactly. The fereg prototype passes 1,947 Rust executions,
312 Python tests and seven portable checks in the source-only suite, plus
focused batch/admission/cancellation witnesses. Its private universal plugin
passes audit and ABI/load checks. These checks do not establish performance
or installed-package admission.

The fixed diagnostic compared cold-single, reused-single and cold-three-RHS
projections on the same small analytic graph at one/four workers, followed by
complete fits on the revealed high-degree weak A input. It used 24 core
processes (32 actual projections) and eight plugin fits, two observations per
artifact/cell, no warmups, retries, replacements or exclusions. Instrumented
core and release-plugin results remain separate.

| Observation | 1 worker | 4 workers |
| --- | ---: | ---: |
| Cold three-RHS application ratio, candidate/parent | 1.07038 | 0.99828 |
| Cold three-RHS complete projection ratio | 0.99558 | 1.02458 |
| High-degree weak complete native-fit ratio | 1.02317 | 0.97254 |
| Parent native-fit median, ms | 352.951 | 209.939 |
| Candidate native-fit median, ms | 361.130 | 204.173 |

Application accounts for only about 1.4%/2.4% of the parent's small wide
projection time. The measured application change therefore offers little
end-to-end opportunity on this fixture. Reused calls perform no new hierarchy
setup in either arm. The three-RHS candidate really installs width three,
versus scalar width one in the parent; one input column is already orthogonal,
so three submitted columns do not mean three difficult active columns.

The original run stopped after successful call 00 because its Python reader
incorrectly required a populated optional residual-norm cache. The Rust
projection certificate and analytic witnesses had passed. A separately
declared and independently reviewed amendment accepted an empty cache, while
requiring supplied entries to have the original shape and finite nonnegative
values. It revalidated and reused call 00 and ran only unstarted calls 01–31,
within the original conservative deadline, in 20.353 seconds. The original
failed result remains failed. The first reused-single/one-worker block has a
795.330-second interruption, retained explicitly; no original-protocol pass
or paired nonregression inference is claimed.

Independent Astra high review verifies the complete amended evidence and
recomputes its original numerical witnesses. All 32 processes complete cleanly;
same-worker repeats, cross-arm scientific bits and cross-worker decisions pass.
Extra batch initialization work is retained as declared. Ordinary desktop
activity is retained; endpoint snapshots do not establish continuous isolation.

**Decision:** archive the fereg batch experiment and restore its exact eight-file
parent, including dependency pin and complexity inventory. The separately
published upstream API remains available. The small, mixed timing changes do
not justify the added fereg code and admission surface. Benefits for much wider
active RHS sets remain untested, not disproved. Do not extend this completed
panel to seek a win or derive a width/core threshold from it. Public Auto,
closed SCC outcomes and Mac B/C status are unchanged.

A subsequent source/assembly audit finds that synchronous census already
caches canonical rows, avoiding a second sort during emission. Its comparator
is inlined. Replacing positive-weight `total_cmp` with bit comparison would
remove only four ALU instructions per weight tie-break, without removing calls,
the sift frame, swaps or work gates; no new timing exercise is justified by
that observation. More substantial construction changes require an explicitly
declared resource/cancellation contract, preserving independent science.

Complete source, build, protocol, amendment, output and review hashes and all
descriptive observations are recorded in the compact controlled-results JSON.
This is a closed engineering diagnostic, not routing qualification.

### CMG construction CPU profiles, 2026-10-17

The first `/usr/bin/sample` design stopped at its non-estimation feasibility
gate: its dummy body was sampled, but the required start-barrier frame was not.
Its eight solver captures never ran. The initial harness include-layout failure
and corrected build are also preserved. A separate Xcode Time Profiler design
first passed one non-estimation probe: the recorder's actual Darwin notification
arrived before release of the stopped target, and the trace contained 2,000
symbolized body samples plus one retained missing-stack sample. This establishes
recording readiness and observed coverage, not first-instruction completeness.

The separately reviewed eight-call diagnostic then completed in 83.979 seconds.
It uses the unchanged parent ARM64 artifact, high-degree weak A input, automatic
upgrading and explicit CMG-k1, at one/four workers with two captures each. Every
call passes the original numerical witnesses, exact fixed-worker repeats and
same-route cross-worker scientific/discrete checks. Every trace contains
symbolized setup frames from the bound plugin UUID. No replacement ran.

| Route / workers | Setup samples, captures 1 / 2 | Work-gate leaf samples, captures 1 / 2 |
| --- | ---: | ---: |
| Automatic upgrade / 1 | 65 / 63 | 26 / 18 |
| Automatic upgrade / 4 | 63 / 64 | 25 / 24 |
| Explicit CMG-k1 / 1 | 23 / 23 | 0 / 0 |
| Explicit CMG-k1 / 4 | 21 / 22 | 0 / 0 |

Across the four automatic captures, 93 of 255 observed setup samples have
`Work::gate` as the leaf (36.5%). Local sift routines account for 48, upstream
sift routines for 42, callback wrappers for 20 and other leaves for 52. These
are disjoint leaf categories within setup ancestry, not sums of inclusive
frame percentages. All 2,893 rows remain retained, including 20 missing-stack,
14 blocked and three rows containing unresolved frames. CPU weights across
workers are not elapsed time; adjacent samples and two captures do not supply
independent timing replications or a speedup confidence interval.

These profiles motivated the completed local gate-inlining diagnostic above.
Actual optimized hot-loop assembly confirms removal of repeated local calls,
while the finite paired comparison supplies separate complete-fit observations.
The observed 36.5% was not a forecast of removable work. No checkpoint/RHS
threshold, new SCC campaign, Mac B/C admission or public Auto activation follows
from this profile.

The original retained campaign is
`/projectnb/welfgr/fereg/runs/portfolio-unseeded-20261016/`; the compact evidence
records complete job, plan, aggregate and review hashes. Original setup failures
remain retained. No runtime source changed during terminal review. Public Auto
remains incumbent, all old campaigns/B remain closed, and this baseline-only
result blocks fresh Mac B12/C24.

Prospective Mac qualification tools now recognize the six-allocation schema and
the exact unseeded candidate. They rederive the complete discovery context,
reject a baseline-only outcome, and keep fresh Mac science, B12 selection,
public activation/equivalence and untouched C24 gates. Independent Astra review
and 90 final focused tests pass. An earlier 135-test run had one new NaN-fixture
assertion error; the repaired test retains rejection, and production bytes did
not change between runs. Runtime production and artifact inputs still equal
the frozen source. These checks do not admit B or C early.

The revealed A panel includes a wide-RHS case, but no matched comparison of
cold one-RHS, reused one-RHS and wide batches on the same topology. Low-memory
chunking is not a poolsize-one experiment. This panel can assess broad routing
and one-to-four-worker scaling, but cannot establish a general RHS payback rule.

A separate matched regression now checks those three RHS layouts on the same
analytic graph, weights and target, at one/four workers. The reused case first
solves a different RHS and retains the same hierarchy and setup ledger. Both
priming and target results pass the independent residual, reconstruction and
gauge witnesses, exact fixed-worker repeats and cross-worker discrete-decision
checks. Wider first actions receive the existing width-based allowance; later
actions at the same accuracy do not renew it. The focused test, independent Astra
high source review and complete source-only suite pass (1,935 Rust executions,
312 Python tests and seven portable checks). Production identity is unchanged across 192 runtime
modules and 58 artifact inputs. Suite output SHA-256 is
`10e9ea5e2c2711dfb7e1b276f87c936fd8c49645cc22da0dc0cfdc9f5c615853`.
Installed-package admission was not performed, and the Linux mock audit remains
unavailable on Mac. This adds correctness coverage only: it neither measures RHS
amortization nor changes the first-action allowance or upgrade policy.
Full hashes, preserved failures and scope are in the compact evidence linked below.

The prospective Mac witness helper now binds this compiled candidate and private
build pair, with independent source review and 23 synthetic tests. The earlier
16-test helper remains unchanged in its closed campaign. After complete
discovery/B-context and untouched-C-declaration checks, the new helper can reuse
certified B references, checking exact paths and hashes before parsing. It keeps
the shared Linux sampling descriptor byte-identical and separately requires
current Mac resource/source/build admission. This is source-only preparation:
no actual configuration, B/C numerical payload, reference target or native
execution was accessed for this review.

Retained SCC raw evidence and sealed products remain in the campaign
`/projectnb/welfgr/fereg/runs/solver-routing-20261014/`.
The [compact evidence](../bench/results/solver-routing-controlled-20261014.json)
preserves every earlier failure, both attempt reports and these final bindings:

| Final aggregate binding | SHA-256 |
| --- | --- |
| Canonical aggregate | `7ffe3acc72848035afc68d2b216830dd268a72b508ad44ba40d02e3a126ad76a` |
| Aggregate file | `56afacb1c018c979159203cba9bd53e6c7851f12343e2bdc0b5e7391f5664e50` |
| Collection envelope | `1c10892d9240557ffb83f1129a538847856d0ea1c9fae82e99562581000af23d` |
| Isolation binding | `97723e0fa92d0b1a53f0e8b6bb9fd189c58f78462cc4a3a2dd07673594d8ef20` |

The separate public-activation and C transition now passes independent source
review. Its prerequisite hook collects two hidden-policy and two uninjected
public-Auto calls on every original A/B cell, checks exact science and solver
signatures, and retains the actual resource-evidence provenance. Only complete
equivalence admits the original C panel, with 24 separate paired blocks per
required edge. No actual B selection, activation or C calls have occurred.
The earlier activation-tooling integration passed all 260 benchmark tests and
the full repository suite, including 295 Python tests with the available scientific dependencies,
1,331 core tests, 195 plugin FFI tests and ten isolated allocator audits. The
Linux mock audit remains unavailable on Mac. Production identity is unchanged
against `09f7c134`; the compact evidence retains source and check-output hashes.

The narrow Mac admission fix now accepts the original Linux archive/commit/input
identity through the sealed parent's candidate-artifact bindings. Mac and newer
explicit plugin identities retain their existing checks. Independent high-effort
Astra review clears the source and tests. At that earlier boundary, all 17
focused block tests, the
`./scripts/run_tests.sh` suite and all 299 benchmark tests passed; the benchmark
suite completed in 176.505 seconds. Production identity is unchanged against
`abf20e8f` for 192 runtime modules and 58 artifact inputs, so prior native
qualification evidence remains valid. No new native qualification or Mac B/C
execution is claimed.

| Earlier Mac-admission tooling check | SHA-256 |
| --- | --- |
| Coordinator source | `5456df8770f35c8a583f5ad5a6ba060256a00b944299327d1b51522325dd72f0` |
| Coordinator tests | `ab40d1de2d8720d764dfe5c2212911a8789826bd90c6d0984aa3116cc31e61ee` |
| Full repository suite output | `18808c4552956f2f0e5956bbabcad3f6bf3a808eb6dc540be6477831b0f13fdb` |
| 299 benchmark tests output | `3dbd2699c63dbeb5c1f8bfc00cb9d7887101f78bbcabaf97d87af6798515802a` |

## Authorized routing qualification recovery, 2026-10-14

After reviewing the outcome below, Johannes approved the bounded recovery and
explicitly authorized future retries. This supersedes the requirement to ask
again before another retry; it does not turn any existing failed or inconclusive
run into a pass. Before each additional attempt, freeze its eligibility, bounds,
host conditions and analysis, retain previous observations, and account for
repeated looks. Scientific tolerances, model semantics and performance allowances
remain unchanged.

The first approved recovery preserves the candidate, baseline, harness and inputs.
Only the two inconclusive standard cells receive 14 additional paired repetitions
each, with one warmup per artifact and cell (60 calls maximum, 90 seconds per call,
600 seconds total). All seven original pairs and 14 new pairs remain in the final
21-pair comparison. It uses 10,000 bootstrap draws with seed 739391 and the nominal
99% two-sided percentile interval (zero-based endpoints 49 and 9949), retaining
the existing `max(5%, 1 ms)` allowance. Original, new-session and pooled results
remain visible; only the frozen pooled upper bound decides this recovery.
The original nominal 2.5% upper-tail allocation plus the final 0.5% gives a nominal
3% per-cell allocation across the two looks. Percentile-bootstrap coverage is
approximate, pooling assumes comparable sessions, and no exact or simultaneous
error-control guarantee is claimed. Any later statistical look needs its own
prospective allocation under the standing retry authorization.

One complete replacement policy A uses the same 42 cells, finite 12-policy grid,
comparators, three paired repetitions, 120-second call and 14,400-second panel
limits. The entire interrupted A run remains in the evidence and is ineligible
for selection because of independently observed host interference. Its three
partial performance summaries were not read before that decision. Existing
scientific prerequisites and terminal optional unavailabilities are reused;
unavailable optional arms are not retried. No ablation rerun or kernel promotion
is implied. B requires a complete eligible A selection; public-default C also
requires resolved standard qualification, a qualifying B winner and the existing
activation/equivalence gates. The recovery observations below remain separate
from that authorization.

Admission precedes launch in a separate inspected step. Freeze the logical CPU
count and activity classifications, then collect six process snapshots ten
seconds apart. Every snapshot must keep unrelated aggregate CPU at or below
10% of logical capacity, every unrelated process at or below 50% of one core,
and competing computation at or below 5% of one core. Ordinary desktop processes
remain counted. During measurement, a fixed five-second monitor stops the owned
measurement process group for any unrelated process at 100% once, aggregate
excess in two successive snapshots, or competing/unclassified computational
activity above 5% in two successive snapshots. Monitor failure also stops the
attempt. These are practical interference guards: decayed process CPU averages
and discrete samples cannot establish absence of all noise. No unrelated process
is terminated. Each trigger ends that attempt and preserves its observations;
the standing retry authorization does not make a contaminated attempt eligible.

The approved proposal has SHA-256
`fb0cabaa34c687adae88a19071b60a88a0735e1e60195d5491e19a64f72cb5b9`.
Its methodology and hardware-scaled host thresholds received independent Astra
review. The pooled-analysis coordinator passed 11 focused tests and independent
review; the guard passed 20 focused tests, a bounded detached-child cleanup
witness and independent review. The coordinator has SHA-256
`5db3478d8640bd63aec3bcf7a616037876816604a977bae49187cf935e782722`;
the guard used for measurement has SHA-256
`7d20233d4a606ea51d71fc0569bf5db0944c18db1e1b5e600bb49f7f42ad1258`.
Before any measurement, the unknown-process inspection rule classified only the
exact Apple USB audio service executable as ordinary desktop activity. It remains
subject to aggregate and individual CPU limits. Previous guard bytes, admissions
and the original protocol are preserved; only the guard binding was updated.
Two immediate launch checks rejected dispatch before any benchmark call. A fixed
15-second idle pause before the next launch check allowed dispatch overhead to
settle without changing the guard or its thresholds.

The first launched confirmation was stopped at 20.07 seconds after Core Spotlight
reached 110% CPU. The monitor confirms cleanup of the owned process tree. The
whole attempt was declared ineligible from the host receipt before any timing
values were inspected. Its failed timing summary retains eight observations;
those are preserved but excluded from the eligible confirmation pool. The host
receipt has SHA-256
`22f3c93b5513e078db5d079d19d1a4c6a70c3c4916120a41b8043a4d7cabc644`;
the interrupted summary has SHA-256
`b685a892d84ebe91321d0082d7186a0954e723d4c17862b97960cbe75a5447bf`.
No pooled analysis or additional eligible statistical look occurred.

Under the standing retry authorization, one further attempt is declared before
collection: identical artifacts, inputs, 14 new pairs per cell, 60-call maximum,
90-second call limit, 600-second panel limit, guard and pooled 99% analysis.
The original seven pairs remain included; the externally invalidated eight
observations do not enter the pool. A new output location preserves the failed
attempt. The nominal two-look allocation remains unchanged because no additional
eligible performance analysis occurred. Independent Astra methodology review
agreed with this treatment. Another interference trigger ends this attempt and
preserves its outputs; it does not authorize automatic repeated sampling.

That retry completed with all 60 calls exact and a passing terminal host record:
19 monitor snapshots, no interference trigger, maximum unrelated aggregate CPU
50.8 percentage points, and complete owned-process cleanup. No background service
was paused. Collection ran 10:16:02--10:17:25 America/New_York. The original and
new session summaries remain visible; only the declared pooled result decides.

| Confirmed cell, 21 eligible pairs | Median ratio | Nominal 99% interval | Outcome against 1.05 |
| --- | --- | --- | --- |
| OLS, 129 regressors, four workers | 1.011018 | [0.994271, 1.054836] | Inconclusive |
| PPML, five regressors, one worker | 1.001253 | [0.989739, 1.006928] | Pass |

At this intermediate look, 19 of 20 standard cells passed; overall standard
qualification remained unresolved. The completed confirmation summary has SHA-256
`f6c523cad77c6006a7b69d81374dd833baa7246e652681288b2b53bb1c3a9eca`,
the pooled report
`538c1285da28f0a0c9b14202a61eb60a820d418b79219923ea0d9555fde6230c`,
and the terminal host record
`440badae9e2febf3fc634e7242ecf76baf0d23774e7a70a288fe17288b5121db`.

One terminal continuation was declared for the unchanged artifact under the
standing retry authorization: only the unresolved wide-OLS cell receives exactly
42 additional paired repetitions and one warmup per plugin (86 calls maximum,
90 seconds per call, 900 seconds total). All original seven, completed-confirmation
14 and new 42 pairs enter the final 63-pair comparison. Keep the other 19 passing
cells and preserve the externally invalidated eight observations separately.
Use the unchanged median paired ratio and `max(5%, 1 ms)` allowance, 10,000
bootstrap draws, seed 739391, and nominal 99.8% interval endpoints 9 and 9989.
Only the final pooled upper endpoint decides; session-only results are descriptive.
The same admission and interference guard applies, with no automatic restart.

Independent Astra review accepted this as a final prospective amendment within
the existing approximate protocol. The three upper-tail allocations sum to
`0.025 + 0.005 + 0.001 = 0.031`; this is a nominal allocation, **not** guaranteed
3.1% sequential error control. Percentile-bootstrap coverage is approximate,
the additional sample size and confidence level were chosen after earlier
results, and pooling assumes comparable paired-ratio distributions across
sessions. A remaining nonpass ends the current qualification attempt on this
unchanged artifact; further work turns to profiling or reconsidering the artifact,
rather than adding eligible observations until a pass.

The intermediate compact export appended eight invalidated and 60 eligible
recovery observations, every pooled pair, twelve compact host records and the
then-unrun final continuation declaration. That 739,715-byte version had SHA-256
`429d8d6df1b576ea263e40cbabc895aacd28c2711c7f7fca0337c3ee4398f213`.
The corresponding routing PDF was 21 pages (617,007 bytes), with
the recovery interval plot and explanation on page 21; original results remain
historical on pages 18--20. Compilation and changed-page visual checks pass.
Its SHA-256 was
`b978b696c5af7f06da1b62d55dfb66c1dcf5d4c179930d2af7bbdab80ef84318`.
The final continuation subsequently passed its fifth admission and launched.
After 70.07 seconds, Core Spotlight reached 120.6% CPU and the guard stopped
the owned process tree, confirming cleanup. The whole attempt was declared
ineligible from the host record before inspecting its timing values. No final
pooled analysis occurred. One replacement attempt under the standing retry
authorization preserves these interrupted observations separately and retains
the same fixed 42 new pairs, 63-pair eligible pool, bounds and terminal decision.
The replacement changes output locations only; it adds no eligible statistical
look and does not relax the host guard.

That replacement completed 11:03:08--11:05:34 America/New_York with all 86 calls
exact. Its terminal host record confirms 32 snapshots, no interference trigger,
maximum unrelated aggregate CPU 44.5 percentage points, successful coordinator
exit and complete cleanup. No background service was paused. The host record
was inspected before the pooled analysis. All original seven, first eligible
confirmation 14 and terminal 42 pairs are retained; the failed eight- and
40-observation attempts remain separate and excluded.

| Final wide-OLS comparison | Median ratio | Interval | Outcome against 1.05 |
| --- | --- | --- | --- |
| All 63 eligible pairs, declared nominal 99.8% | 1.003589 | [0.992942, 1.015636] | Pass |
| New 42 pairs alone, descriptive nominal 95% | 1.001589 | [0.992047, 1.009302] | Descriptive only |

**All 20 standard cells now pass the declared nonregression gate.** The terminal
pooled report has SHA-256
`2478806f2ffaaac9ba56517c572907b5c47e82558b0a2fa0b818c5ff4f3f3b5e`;
the clean host record has SHA-256
`96dff2b9068ecb781485eb28469b1af9360004a54f4b003b037912cd09545ec8`.
The approximate-bootstrap and session-comparability limitations above still
apply. This closes the standard continuation; it does not select a routing
policy, qualify the exploratory ablations, or change public Auto.

The current compact export preserves every previous value and appends the
interrupted 40 observations, final 86 observations, all 63 eligible pairs, bound
protocols and seven more compact host records. Independent row-by-row comparison
and bootstrap reconstruction agree exactly. No full process inventory or
personal path is retained. It is 877,202 bytes with SHA-256
`4b2f11426c6d38fc01699397dfbd2d7c828902e7490851e87ebc331887a95e8a`.
The updated 21-page routing PDF is 616,969 bytes, SHA-256
`72844a8dc6debb269d293bb3bba510eb2bc376a6adbaae9d8bb189288520012b`.
Its first and final pages distinguish the passing standard gate from pending
routing selection. Compilation, all page-number checks and visual inspection of
both changed pages pass. Independent Astra reconstruction also confirms the
eligible pool, original scientific fingerprints, retained 19 passes and final
bootstrap decision. Runtime artifact inputs remain unchanged.

Before replacement policy A collection, a reviewed coordinator also binds fresh
outputs to the guarded process. It rejects existing output, executes the frozen
native command unchanged, and returns success only after complete policy records
are hashed into its completion record. This prevents an early producer failure
from attaching a new host record to stale results. Its four focused tests and
independent Astra review pass; its SHA-256 is
`fca17b66d89a36b6398d642ffbbe2ad133512c21d4b4d3404efa085db1616512`.
Replacement policy A subsequently passed quiet-host admission. Its first final
launch check rejected an unrelated background-service burst before any native
call. The next launch started 11:13:47 America/New_York and was stopped after
60.06 seconds when Spotlight's index store reached 207.6% CPU. The guard confirms
complete owned-process cleanup and coordinator exit by interrupt. The whole
cohort was declared ineligible from the host record before its performance
values were inspected. Its host record has SHA-256
`4dea30ef0be2499bd4dcea6047cac9cafdd3480ef19c5327e6dd379c8d8cd546`;
its failed records have SHA-256
`968275094718348c5c31a46a4cd277f7c3f8ecbdef4f555c8912492a8082d33b`.

All 512 completed timing calls (128 warmups and 384 measured calls) and 139
completed traced calls pass their science and certificate checks. One additional
traced dispatch has no terminal result. Eight partial performance summaries are
preserved without a selection or performance conclusion. These interrupted
observations do not alter the completed standard gate. B and C remain unopened,
and public Auto is unchanged. Johannes subsequently directed another retry while
away from the computer, with Spotlight left running. No service has been paused.
Documentation and evidence edits do not change runtime artifact inputs.

The separate [interrupted policy evidence](../bench/results/solver-routing-policy-a-interrupted-20261014.json)
retains every source field, observation and partial summary in lossless column
tables, plus three compact host records. Independent reconstruction reproduces
the entire original source object, including absent fields on the unfinished
call. It is 819,578 bytes, SHA-256
`e5ca973e7379ba53513d844332b6b3607e4c41d42437d81feb46ff924e39e335`.
The completed standard evidence remains unchanged.

This further policy A attempt is declared before collection. It retains the same
42 cells, 12-policy grid, comparators, three paired repetitions, one warmup,
120-second call limit and 14,400-second panel limit. The 18 previously unavailable
optional comparisons remain unavailable. Fresh outputs preserve every previous
attempt; neither interrupted cohort can enter selection. The same independently
inspected admission and runtime interference guard apply, with no service
changes. Only a complete eligible cohort may produce a sealed A shortlist and
open B. No standard continuation, policy retuning or additional eligible
statistical look is introduced by replacing a host-ineligible incomplete cohort.

This retry passed admission but stopped at 25.07 seconds for two consecutive
unclassified FSEvents-service observations at 5.1% and 5.5% of one core. Unrelated
aggregate CPU was 30.8 and 35.2 percentage points across 24 logical CPUs. The
guard confirmed cleanup; the entire attempt was excluded from the host record
before performance inspection. Its 166 timing entries include 164 completed
passes, one blocked `EPERM` infrastructure failure and one unfinished dispatch;
all 43 traced calls completed and passed. Neither unresolved entry receives an
imputed scientific result. The host record has SHA-256
`5918b81ec5ff9774122be6460208a3b645a101109c6c7e35a89e9705454f7b39`,
and original failed records have SHA-256
`9be0bd207b8cfdb3943ff1ed0dca42d23ed90935a3d8ef73725409857af78a05`.

Johannes authorized stopping Spotlight if possible. A noninteractive privilege
check required an administrator password; no service was changed. The next
attempt instead applies a narrow prospective classification amendment under the
unknown-process inspection rule. Only the exact inspected Apple FSEvents daemon
path joins the existing ordinary-OS category (named `ordinary_ui` in the guard).
[Apple's description](https://developer.apple.com/library/archive/documentation/Darwin/Conceptual/FSEvents_ProgGuide/TechnologyOverview/TechnologyOverview.html)
identifies its filesystem-event notification role; this does not establish that
the observed work was harmless or caused by the benchmark. It remains counted
under all aggregate and individual CPU limits. Spotlight/computation rules,
thresholds, sampling and cleanup are unchanged. The old guard and every failed
attempt remain intact and ineligible. The future guard copy has SHA-256
`cfb57443211d1946e174e4222894916c391a5167dd919a444bdc5e73646a6f34`;
22 synthetic tests and independent methodology/source review pass.

The infrastructure error is retained separately. Source inspection and four
mocked tests demonstrate that the existing process cleanup can mask an interrupt
with a signaling error; they do not establish the historical cause. One
prospectively bounded, unmeasured diagnostic repeated the affected prepared input
under the same artifact and harness, with a 120-second limit and no retry. It
completed, matched the earlier science/solver fingerprint exactly and passed the
independent replay oracle. This establishes fresh infrastructure completion,
not historical success or performance eligibility. Its outcome has SHA-256
`b7e10bea6592dfeb2854af70105fe30ac9aa429d3bb6a238dffa0af0ba032077`.
The next full A attempt
keeps the same fixed native command, panel, statistical rule and limits, using
fresh outputs and the newly classified guard only.

The [compact retry evidence](../bench/results/solver-routing-policy-a-retry01-interrupted-20261014.json)
retains all records, absent unfinished fields, summaries, protocol and host
identities, including the unresolved infrastructure failure. Reconstructing its
column tables reproduces the entire original source object. It is 267,924 bytes,
SHA-256 `84f8838c26d71b2ba608736d39e7a9bcc8b84a7ac7be6c8bfe91c8f9f26022ad`.

The next retry passed admission and launched at 11:45:17 America/New_York. At
60.07 seconds, Spotlight's index store reached 110.2% CPU and triggered the
unchanged individual limit. Cleanup completed, and the entire cohort was
excluded from the host record before timing inspection. Its host record has
SHA-256 `57c53e616d33decb1bcec009f69bdb59dcbc31b9f923b07094c563ab853f60d9`;
the original failed records have SHA-256
`2e936a1a7819fc41bd25a8992009b32b4ff4e660bacd1e782426cabe3103892c`.
All 512 timing calls and 141 completed traced calls passed their recorded science
and certificate checks; one traced dispatch remains unfinished. Eight partial
summaries are preserved without selection or a performance conclusion. The
[second retry evidence](../bench/results/solver-routing-policy-a-retry02-interrupted-20261014.json)
reconstructs the complete source object exactly. Its initial export was 823,401 bytes, SHA-256
`f137944e95409cfa128a3e853784d741437aab27e688aa6e8b6251004548d9e5`.
The completed standard gate and previous evidence remain unchanged.

One further replacement is declared before collection under the standing retry
authorization. It retains the same artifacts, inputs, native arguments except
output location, 42 cells, finite policy grid, optional unavailabilities,
repetitions, statistics and time limits. Only its fresh output directory gains
the `.noindex` suffix. [Apple's developer documentation](https://developer.apple.com/documentation/xcode/locating-a-missing-debug-symbol-file)
identifies paths containing `.noindex` as an obstacle to Spotlight lookup.
This is a local indexing mitigation, not evidence that Spotlight has stopped
or that earlier indexing was caused by the benchmark. The supported Settings
folder-picker attempt did not complete; no privacy exclusion was verified and
no privileged service change occurred. Independent Astra methodology review
found no blocker to this prospective output-only mitigation.
The relocation wrapper passed five focused tests and independent source review;
its SHA-256 is
`ed3fb34c9ef812b8476383e86982f3608e052edfe3187cc2fc2aa168e0ff761e`.
The frozen protocol has SHA-256
`6a886afd42f3523d59479a8114f1c0a8f66ea114a786e1472953de1a0d697109`;
the guarded command has SHA-256
`edf9a728036fa4c5bee3508d77a99a2cd79b57b76941836d299877a6ff8e7ad1`.
Its first admission rejected an unrelated FEVC Stata test suite, with unrelated
aggregate activity reaching 871.7 CPU percentage points. No native routing call
started. The admission record has SHA-256
`d5cf58513bcd5b0f09edfbab61d198edc3d6d51e8cae6f0ba6b0bc40ff0b9f1d`.
The unrelated test process was inspected but not interrupted.
After that test finished, two fresh admissions also failed: the first recorded
Spotlight and desktop-service activity; the second recorded sustained desktop
activity and an individual-limit violation. Their admission records have SHA-256
`0f93db215b5259eb76134fd8480c0497c43eacd3bf0ac39e1771fb70af18f2b3` and
`d0816cd2707e253a46768b60bf80adc74348ee25dd37360a0dd530b9c83a69b9`.
This task's computer-use session was reset before the final admission. No native
output directory or producer log was created. The bounded wait ended with the
fixed retry prepared but unlaunched; host availability remains unresolved.
The compact second-retry export now also retains all three follow-up admission
summaries and sample observations, without changing its original source object:
832,996 bytes, SHA-256
`c22f2b2c15f6eaba9ea61a5b56144f6d1eb6b55f0f368be109c700887e6a7bb7`.

Johannes subsequently requested another retry and reported the Mac idle. A fourth
admission failed only for sustained unclassified activity: the Apple aerial
wallpaper extension and Codex's shared computer-use helper. Unrelated aggregate
CPU ranged from 64.8 to 80.2 percentage points across 24 logical CPUs; no native
call started. Its admission SHA-256 is
`9f6376f1f415e8563b7e0cef4f8aedad8a261b58cccd6112590d9c816c7094ea`.

For future admissions only, the exact inspected Apple wallpaper executable joins
the ordinary-desktop category. [Apple documents aerial wallpaper](https://support.apple.com/guide/mac-help/choose-your-desktop-picture-mchlp3013/mac)
as a desktop and screen-saver feature. This classification follows the existing
unknown-process inspection rule; it does not establish absence of timing noise.
Its CPU still counts under the unchanged aggregate and individual limits.
Codex's helper remains subject to the tool-work check. It is shared with screen
history and other computer-use clients; no helper termination or setting change
was made. Every past admission remains rejected under its original guard.

The new guard differs only in that exact wallpaper classification, passes 25
synthetic tests and independent source/methodology review, and has SHA-256
`f66584ddb8b5b0f559e43c90a2c8a122a61f11b146974d57bc3413188744150a`.
A prospective host amendment, SHA-256
`2cd73b4d11d4300e14ba73d038a46531bd64f89fadfbb5211301e52ef5aa1e43`,
supersedes only the frozen retry's prospective guard choice. The original native
protocol, command, coordinator, outputs and previous guards remain unchanged.
It permits at most two fresh admissions and one launched cohort. Terminal A
eligibility and downstream B admission must bind this amendment and the actual
new-guard receipt; a failed prerequisite still blocks selection and B/C access.
Independent review verified all eleven amendment bindings. The compact export
now retains the fourth admission and amendment as well: 839,008 bytes, SHA-256
`986456c7d91af6cc9fed917588d8376f1f3b01225f6e9319da0b097fc0b29cf3`.
The first admission under this amendment also failed. The shared computer-use
helper exceeded the unclassified-work limit persistently; the last sample also
had an individual-limit violation amid background desktop-service activity.
No native call started. Its admission SHA-256 is
`87314101fa8d50d91fd9bed2f60e706fb8d4f40dbaa93c4054336c237867d944`.
The compact export retains this fifth failed admission: 841,966 bytes, SHA-256
`575fd7489329668a1509d4e741446e0cf89364d1c47f9239fa9c75e8ffecfdb1`.
The routing PDF's guard explanation was updated without changing the original
results or standard chart. Its 21-page compilation and changed-page visual check
pass; PDF SHA-256 is
`9be7ebd4e2aa5d31a3580d090061af9abe702ed877e6a344b91424ba8ccbf070`.

Johannes then explicitly approved temporarily pausing the shared computer-use
helper. The fixed-identity, PID-only controller passed 13 mock tests and
independent Astra source review. Its SHA-256 is
`612e9185124eccb256547a32bb90a6fd08510d09cd1d598b011739518962b18e`;
the tests have SHA-256
`d638603aa8a37768850231a6d5dde7047b0db59ceb1397c0059b29807b61cc37`.
A detached watchdog restores the helper on controller closure or its fixed
15,000-second lease. Identity checks precede each signal; no process-group,
parent-app or Spotlight signal is authorized by this controller. Shared
computer-use/history interruptions, non-atomic process identity checks and
watchdog/host failure remain limitations. The prospective pause amendment,
SHA-256 `519107feea44997aa8bcf5b418809be29df2527f98a67abacf43bb3481dc1aaf`,
preserves all native/statistical bounds and the two-admission batch limit.

The helper was confirmed stopped at 15:02:21 America/New_York. After the declared
30-second settling wait, the sixth admission ran from 15:03:02 to 15:04:02.
Every sample rejected unrelated FEVC Stata validation activity at 397.3--400.0
CPU percentage points; unrelated aggregate CPU ranged from 408.4 to 427.6.
No routing benchmark call started. The other validation job was inspected but
not interrupted. The helper was released after rejection and confirmed running
at 15:04:22; the controller exited successfully. The admission SHA-256 is
`90ab48e6891f3c5254177425d2f913c989890c3250b279f61bfdbd3e58fdb9f5`;
the restoration audit has SHA-256
`8af3d8d3e8cc8ee0f569acb357fb97ef68aad8c6c64715c7c89d6645fb046c8a`.
This exhausts the amended batch's two admissions without a launched cohort.
There is no eligible A performance look. The compact export retains all six
admissions, pause terms and restoration observations while preserving every
prior source value: 848,325 bytes, SHA-256
`67de42f6f031c23f7e06707cbc39e0c307442d0e8d5cb39ba3d48b6b61343486`.
Production identity against `b22c54a0` still passes after the intervening
installation commits: all 192 runtime modules and 58 artifact inputs match.
Existing scientific and standard timing evidence therefore remains applicable.
The updated 21-page routing PDF also records this outcome; compilation and
changed-page visual inspection pass. Its SHA-256 is
`340585e44f5be00ba5215f71e090a9989e04945f00ae8179d3b64545ce307690`.

On Johannes's next retry request, the competing FEVC job had finished. A new
prospective declaration allowed exactly one additional admission and at most one
cohort, preserving all limits and the six earlier rejections. Its SHA-256 is
`ef9d21caba946565c99baad62cdc1a9efcf71140a7f3a6e2436eabeb19c11820`.
The same reviewed controller paused the helper at 16:45:38 America/New_York.
After the declared settling wait, admission seven sampled Apple `suggestd` at
108.4% and 104.5% CPU in consecutive observations, violating both individual and
sustained-unclassified limits. Aggregate readings ranged from 15.3 to 139.3;
the aggregate limit was not exceeded. No native call or eligible timing look
occurred. The helper was released and confirmed running at 16:47:45; no other
process was stopped. The admission SHA-256 is
`0f0e99fabb7e42099b1aa34eca825b8d6b7041721267c53671dd188cb3ff6494`;
the restoration audit SHA-256 is
`561a6bb65c8eabc8d59c880086ef1ec40aea6b5394882679d4f1fd9ab30405a4`.
This single-admission retry is exhausted. All prior evidence remains exact in
the expanded compact export: 855,345 bytes, SHA-256
`f2a43c785ab655c03aefae6ffc94b5e68e47f2b0b6ed7fe46ea3e069ef1363b6`.
The PDF above remains the preceding six-admission snapshot; this paragraph and
the linked compact evidence record the subsequent retry.

The same amended guard, fresh separately inspected admission, terminal host
clearance and fresh-record completion gate remain mandatory. Another interference
trigger ends the attempt. All prior cohorts remain excluded under their original
rules; no eligible statistical look, selection, B/C access or default change is
introduced. All 20 standard cells continue to pass. The following section retains
the original terminal outcomes unchanged.

## Routing timing outcome, 2026-10-14

Commit `b22c54a0` implements the reviewed hidden routing experiments and the
qualified explicit-OLS-CMG numerical repair. Public Auto and explicit IT remain
unchanged. **Performance adoption is blocked.** The independent scientific
qualification below remains valid; timing does not replace it.

The [compact timing evidence](../bench/results/solver-routing-20261014.json)
retains all 320 standard observations, all 1,352 ablation slots (including 72
not-run slots), and all 232 interrupted policy observations in acquisition order,
with original source hashes, case/artifact identities, scientific fingerprints,
failure classifications and unchanged summaries. The original export had SHA-256
`00c338afbca1d2a5be599a51cfa8a5d6c018cecdf6a2d4e4fed652094f70a342`.
The current file appends the recovery evidence described above.
These compact observations and this explanation are retained evidence; ignored
raw captures are not promised as permanent records.
An independent row-by-row audit matched all retained values, order, statuses,
summaries, fingerprints and source hashes. The documentation/data update has
identical production tokens and all 58 artifact inputs to `b22c54a0`; runtime
qualification is reused. Local documentation links and whitespace checks pass.

| Panel | Outcome and limits |
| --- | --- |
| Standard 20 workloads | Completed one warmup and seven paired repetitions. All 320 calls retain exact science and solver signatures. Eighteen cells pass `max(5%, 1 ms)`; two are inconclusive. The complete panel does **not** pass nonregression. No retry or continuation was declared. |
| Backend ablation A | All 26 cells collected. All 320 new traced calls and 1,280 warmup/measured calls pass science/repeatability. Eighteen prior optional unavailabilities remain unavailable, with 72 dependent slots not run. A macOS storage scan started during collection, so **all ablation timings are exploratory**, not quiet-host qualification. Ablations cannot select a policy. |
| Policy A | Interrupted after competing Stata and storage work was detected at launch. The lead mistakenly dispatched the runner in the same tool batch as the host check, then stopped only the owned runner. Its 58 traced and 232 warmup/measured calls passed science; only three cell summaries were complete when interrupted during the fourth summary calculation. The producer records `failed`, not a complete panel. No policy selection occurred; B and C remain untouched. |

The two standard nonpasses are reported without trimming or substitution:

| Standard case | Baseline / candidate median (s) | Paired median ratio | Nominal 95% bootstrap interval | Allowance |
| --- | --- | --- | --- | --- |
| OLS, 129 regressors, four workers | 0.373374 / 0.379300 | 1.013664 | [0.990950, 1.103904] | 1.05 |
| PPML, five regressors, one worker | 0.429588 / 0.430434 | 1.006928 | [0.980497, 1.062558] | 1.05 |

Neither interval establishes a regression, but neither establishes the required
nonregression bound. The original fixed-run stopping rule remains terminal.
Successful A/B experiments cannot supersede this unresolved adoption prerequisite,
and the final C holdout must remain untouched while it is unresolved. At the
close of this original report, no further qualification amendment had been
approved. The subsequent authorization above permits bounded recovery and
future retries while preserving these observations and repeated-look accounting.

Host observations use America/New_York local time. The standard panel ran
06:49:36--07:00:30, before the observed storage-service start. Ablation ran
07:01:56--07:20:02; StorageManagementService started at 07:11:52. At the policy
launch, a process snapshot showed Stata at 162.3% CPU, that storage service at
112.2%, and ApplicationsStorageExtension at 84.4%. Policy observations span
07:21:22--07:21:35 (13.326 seconds). These observations establish possible
ablation overlap and competing work at policy launch; they do not establish
continuous utilization, a per-cell contamination boundary, or the cause of any
individual timing. Activity Monitor had been quit with Johannes's authorization;
no competing application was terminated. Builds, tests and agent host workloads
were stopped throughout the measurements.

The standard timing command first aborted in preflight because historical
qualification receipt 19 still named the former IT checker at its live path.
No native call or timing output existed. An independently reviewed task-local
wrapper relocated only that exact receipt, in memory, to its byte-identical
saved source; current reviewed checker/bridge validation runs before and after
the unchanged recipe. Four focused tests and one nonnative preflight passed.
All original files, the abort, command, cases, artifacts, statistics and bounds
remain unchanged. This reference correction did not repeat a measurement.

| Timing provenance | SHA-256 |
| --- | --- |
| Standard terminal summary | `3514797db3806da7b625dc4683bfbbed82a86610bbbac615b5671ac1209fd7ed` |
| Standard terminal wrapper log | `2e6cc29f8cb4c2658626ffd4939fcb6d16dd61639e8cc6a73b9eb675b3784c1f` |
| Standard cell log | `7d82d44c725b4f4e366fdfc170d8bb594983b7e4b922ee812cb68ea935c5e261` |
| Original preflight abort | `39a8b2a91c5c21aff7449014908e7e9da4e77e8fe07684d7c20db5f77d5d4903` |
| Reviewed receipt relocation wrapper | `25ae3deb1aa6e82690aee3a062c7a0c36cd2cafbc0441a68f0346087e4af132a` |
| Complete ablation A collection | `d9f33acbccfc7f485597fbae741181fdcb0dfad2635673a180d87f30f270951a` |
| Interrupted policy A collection | `5a48c8a08f7f5b678b711904f5d9ec7aba17276acc0a7e689e8cd7c88fb1ef57` |

## Integrated experimental routing, 2026-10-14

The integrated implementation adds serial checkpointed Schur and general-LSMR
pair replacement, deterministic CMG preparation on one existing worker,
categorical None/MAP4/compact-IT4 prefixes, separate diagonal/cell/direct arms,
and topology-only PPML reuse. All remain sanctioned hidden experiments.
Ordinary Auto and public explicit IT retain their incumbent behavior.
[SPEC](SPEC.md#solver-support-and-dispatch), the
[protocol](plugin_protocol.md#developer-injections-hidden-adapters-only) and
[editable routing explanation](solver_routing.tex) define the contracts.
Implementation and scientific validation do not select a new default:
whole-policy performance qualification remains pending.

The explicit-CMG polishing repair has a prospective, exact-input-bound scope
in [`2026-10-14-explicit-cmg-polish-macos.json`](../tests/replay/scopes/2026-10-14-explicit-cmg-polish-macos.json).
It covers only OLS cases 08, 11, 14, 26 and 29; the other 68 ordinary cases,
including PPML, retain exact parent comparisons. This declaration is not a
native qualification result for the repaired artifact.

The independent [`replay_cmg_reference.py`](../crates/fereg-oracle/python/replay_cmg_reference.py)
uses 70-digit arithmetic on exact binary64 inputs and an identified full dummy
design. If `G = Z'WZ`, its reporting transform keeps the two finite coefficients
and maps the dummy coefficients to their weighted mean, the reported constant.
Applying that transform to `G^-1` and to iid, HC1 or cluster score covariance
independently determines every reported covariance entry. Frequency-weight HC1
uses one weight per replicated observation; analytic/probability scores use
squared weights. Each saved FE channel is centered at its own weighted mean.
The frozen fixtures are connected and full rank after one anchor per later FE.
Analytic Walsh witnesses, explicit frequency replication, common-weight scaling,
hand-summed cluster scores, and deliberately corrupted constants/cross covariance/
gauges/certificates test these calculations. The original pinned finite-block,
residual and fitted-value targets remain a second independent cross-check.

The declared numeric fields are coefficients, full covariance, residual/fitted/
finite-xb/absorbed/saved outputs, the twelve fit statistics, mathematically
affected FE and inference diagnostics, and inner iteration work. Independent
point targets cover fit statistics, the two-column rank condition proxy,
reported bread minimum, covariance determinant and applicable covariance minima.
Roundoff residual diagnostics must be finite, nonnegative and no larger than
the frozen requested tolerance `1e-10`; the robust normalized PSD pivot must lie
in `[0, 1 + 1e-10]`. The checker separately recomputes weighted residual moments
against every finite regressor and every FE indicator from the original input.
FE absolute diagnostics have an input-derived bound; relative diagnostics retain
the requested bound. These checks supplement the unchanged SPEC coefficient,
covariance and observation-output tolerances. Presence flags, acceptance,
requested tolerances, sample, rank, omissions, VCE and all other nonscoped fields
remain exact.

The typed diagnostic exception is applied only to a validated parent comparison:
primary CMG must remain CMG; Fast final verification may report certified CG
with exactly one verification/correction warning; Low final remains CMG.
Preconditioners and pilots remain absent. Warning bounds use the actual memory
mode, pool size and declared coefficient capacity, never an observed fit to
results. Every candidate, repeat and worker result is validated before the
exception is considered. Fixed-worker repeats retain full exact signatures;
cross-worker comparisons retain the concrete engine, fallback and warning
counts. Source, artifact, frozen input, independent reference and scope hashes
must match before every native dispatch. The prospective wrapper additionally
binds the successful full suite/build, limits calls and time, requires the frozen
100,000-row dynamic oracle first, and preserves every failed qualification.

| Frozen integrated identity | SHA-256 |
| --- | --- |
| Universal Mac plugin | `b0e53fa3f305f1288673204e047a6d331b2632a94e29464a397fe002fc7ef1f9` |
| Production tokens | `29e1764eba5d93c2dc63baf546791a5ea93e549911037d47600fddad242d8b3d` |
| Artifact inputs | `1cb7d506617ac2f77e35d8b7d2dc08409b14cf4f9eef348a12bf697a726609eb` |
| Source/test identity record | `bc242b414c46d4b35a5844eae61b4be921e037b8d08c3464054dafeb65f03046` |
| Full repository suite | `268bdfa6f8566144fa7c8038db6286b7e2435eb543a27e0d29a1ec8924faaf64` |
| Universal build, binary audit and ABI/load/self-test | `b79c5d6eceaa5a10c9addfde24f859ef9802260445f387c635822f7e6929bb4c` |
| Complete native qualification transcript | `32825bc4fb9b94438a0948fe07962a190f76207bddab5aee6dc914caf37916e0` |

This uses published CMG `b989eddfc3bf7ad130c76df8d0cceaf9801e1d72` and
within/schwarz-precond `efe35ee386ad989b043a5076d01d879ca410ff19`.
The comparison artifact remains
`3539b15fb777360f1de549697319ee77ed5cd30bc08811d51c2dbad5c73c226a`.
The full suite passed: Python 220 (13 platform/dependency skips), core 1,324
(three declared allocator-test exclusions), plugin 195 plus the separate
process-global allocator audits, scientific oracles, strict Clippy, fuzz,
complexity and bridge guards. Linux's GNU ELF audit correctly skipped on Mac.
The subsequent narrow Python trace-guard repair passed all 13 focused checks
(`26e4fc70ea6b6384bf46375bf00ad2ce38024aabcad5482770bd95e72ba6bbc6`); runtime tokens, artifact inputs and
plugin bytes are unchanged. These checks establish no new Linux or Windows claim.

The bounded native run completed **1,131 calls**, with no missing panel:

| Native panel | Outcome | Summary SHA-256 |
| --- | --- | --- |
| Ordinary integration | 73 cases; zero exact differences, fixed-worker repeatability and 2/4/8/16-worker scientific/discrete agreement | `96ca88cf0422865b8032a9322d89d8469257f98fe86e2c7f65010d917f5165c2` |
| Supplemental routing/IT | 23 cases, declared numerical scope, exact unaffected fields and repeats | `00768fe950faafd540ef584ce3b0ed690b536a9d01022069d441855739ec8bbb` |
| Direct Schur | Four unit/weighted Fast/Low cases, including 65 active RHSs | `bd147e70b86eaac022a6e980ffe40d00301c2d04148f5c5b3cf00eedea55f739` |
| Parallel preparation | Six cases and 72 calls at 1/2/4 workers; exact serial fallback at one worker, positive executed builder work, matched joins, fixed-worker trace/results and independent science | `d51417e13181b82b39e5f2a9c7ede2096cd50a6a19eaa7d5cc517b788ac47c9c` |
| PPML topology reuse | 19 cases and 114 calls; accepted/pending lifecycle and current-weight science/signatures preserved | `b1706a4d9ade108f2977595741fe06265bf993fa993fadc3c195df81b337f3a9` |

All eight mapped Stata fixtures passed. All five licensed Break runs passed:
Auto, sampled CMG-Schur, public PPML, and concurrent preparation at two and four
workers. Each builder test interrupted twice after an actually executed
64-unit build operation, then verified the complete prior e() state and sample,
absence of partial output variables, exact ordinary reset and a successful new
overlap action. Builder-test transcript hashes are
`735d7324d7641f522d5e73ac688a2c3835f3a577ec72dc12c1c8becc7cbcc51d` and
`27df82b461a6133c04c602ffc23e141c5b9550357f883d017358cc22304cb812`. The complete native
transcript above binds all mapped-fixture and other Break hashes.

Integration review found and repaired two resource defects before this artifact:
executor admission introduced a per-acquire allocation, breaking existing warm
PPML allocation guarantees; value leases now preserve those guarantees. A
parked general-LSMR builder also retained a smaller foreground-memory estimate
across Low chunks. Admission now refreshes the shared parent/budget before
resuming, and every later allocation reads it. Tight regressions cover wider
RHS blocks, same-width parent growth, budget tightening and already-held storage.
Work caps/grants remain unchanged. Independent source review and a fresh bounded
Astra recheck cleared the repairs. Two-FE sessions already reserve their maximum
Low batch storage before the outcome projection.

Two failed native witness attempts remain failures. The first overlap panel
incorrectly required installation for every Low case; its summary hash is
`ac83361b3520799fc99691d91b5bee13ed7623de1c31f26d5ffa0cbae023043b` and stopped-run hash is
`48a3dc2b737398db7ee65bb6ffcad4e723c88b070a6b774d25cfeceb70ff1476`. The original Low fixtures have one initial
RHS: W0=185,152, total allowance 5,924,864 and builder allowance 5,184,256 after
protecting four iterations for restart. Their two required load/sort passes
alone cost over 7.52 million tariff units, so refusal is correct. The new panel
separately requires that exact bounded refusal, while preserving the old failed
expectation. Two additional Low fixtures use three genuine independent Walsh
regressors and a cell-centered outcome. They must certify the outcome before
setup, then install at the **first** h8/k1 handoff with the three-RHS allowance
11,958,272. Independent dense QR verifies finite rank 3, absorbed rank 126,
residual DoF 8,831, coefficients, HC1 and saved-effect gauges. No padding,
routing threshold, scientific tolerance or work allowance was changed.

The first builder-Break fixture failed before reaching an interruption because
an e() name produced an overlong Stata local name (r(198)); failed transcript
`6d85a8bd3016deefd29a91afc4b77f2f8f5d49433279f2cc8a5fcc4372272dc1`. Numeric indexed snapshot names
fixed that harness error without changing rollback requirements or the runtime.
Both failed attempts are explicitly bound as prior failures in subsequent
resource/sampling evidence, not silently replaced with passing outcomes.

A predeclared two-process CPU sample on the frozen wide-129, one-worker design
produced symbolized native stacks and identical scientific fingerprints. Dense
robust meat and finite moments were material observed costs; Schur application
was smaller. Candidate/baseline top samples were 115/108 for dense meat, 82/83
for finite-moment production and 25/23 for Schur application. These are single
statistical captures, including loader/I/O and blocked coordinator stacks;
they establish neither speedup nor absence of overhead. The reviewed capture
summary is `d4a838fe79165382f32c2dad3c145edf9e39ffc2d8f50a5d6b8c7f42c421d563`.

The final resource and sampling wrappers bind actual passing checks plus the
preserved failures: `8e495c6f6bc546da5ad39740b2f6889db71de99808a4e5465876e04f7ac27571` and
`54d09761d0a614ac5341a80d895e38d470f5fd0c1fdf40127f8334b59c28f975`. The finite policy inputs
are prospectively sealed; performance screening, independent confirmation and
holdout remain unqualified. No real-world firm-worker dataset or external
fixest/reghdfe performance comparison is supplied by these synthetic witnesses.
The first untimed ablation prerequisites completed all 26 cells: 338 initial
calls and 300 planned successful repeats, with 300 passing arms, 18 bounded
solver exhaustions/timeouts and 20 blocked arms. Records hash
`b6a8499463a23c0f5548aeb14f845af9be39973169b48215d959767c957a3359` retains every outcome.
Eighteen blocks came from an overly strict explicit-CMG diagnostic mapping:
the existing certified final CG correction reports requested CMG, used CG,
fallback one and exactly one correction warning. A separately reviewed narrow
checker repair must assert that complete tuple before admitting its two extra
diagnostic fields; the old blocked observations do not become passes.

The other two blocks are scientific failures of explicit CMG on the same
dynamic-weight design at one and four workers. A prospectively bounded
four-call diagnostic (plan hash
`9ba37d1b5e76c5a16ff6d4d56345b9927145c59f8af2cad2fcdbdc3be61ec9e2`)
reproduced identical science and full output columns in two current-CMG calls
and the baseline artifact's explicit CMG call. Maximum residual error was
`3.855834879e-8`: eight rows failed the existing relative/absolute residual
comparison and ten exceeded the separate `2e-8` guard. The current LSMR call
passed with maximum error `6.364e-11`. Analysis hash
`917e9569fd7bc9548849ff6ff231e9cd8abff24d3dd06d2dd68032b691ad2041` binds the retained
arrays and row diagnostics. This is a pre-existing explicit-CMG accuracy
failure, not evidence of a candidate regression or permission to relax the
oracle. Investigation is scoped to CMG polishing/correction convergence;
unaffected routes must remain exact. No timing or default selection follows
from this blocked screen.

Three bounded polishing attempts have not qualified a repair. Removing the
absolute Hestenes decrement floor alone still left an analytic weighted-cycle
error of `8.135e-8`. Adding relative recurrence convergence passed that witness,
but a smaller FE perturbation exposed certificate-only initial acceptance.
Removing that shortcut reduced the smaller witness's error to `1.1992e-10`,
still above its unchanged `1e-10` bound, and exposed missing retained coefficient
corrections. A 100-digit closed-form cycle projection on the exact binary input
localized the remaining error to the FE component: input rounding changes the
ideal paired contrast by only `1.18e-16`. The initial computed projection action
has weighted error `2.762e-16` against an exact action norm `5.686e-14`.
At termination the recursively updated residual norm is approximately
`1.14e-30`, while a fresh action is approximately `1.29e-15`.
Arbitrary value scaling before compensated summation can introduce cancellation
error that a weak graph amplifies. Requiring a fresh relative residual below
roundoff is not a justified repair.
The first and second attempt summaries are
`bc3413db24ee11a405609ae766d9dda71eb98e75142165905bc16d93ae71a14b` and
`50784fa8d48c3af711622aa03f1da828d37128b3882ddb513d90185381c30bbb`;
the third attempt's independent cycle analysis is
`fd5ab23103bc70044e1bcc6f6768bc1493356b13c2afa7bc32131650225e9bd4`.

The fourth bounded attempt added binary value scaling and zero-energy
underflow rejection; its smaller analytic witness still failed at
`1.1992e-10`. Its summary is
`5388640f7b4c2f834b71c82a1d1eb27e8557d8afb41129f0738de00028e5f69b`.
Two subsequent offline comparisons localized the remaining action error to
rounding the pointwise difference and weighted products before reduction.
Retaining the general TwoDiff high/low subtraction parts and FMA product-error
terms reduced relative action error on the captured initial/final vectors to
`1.18e-16` and `4.85e-17`. These are small mathematical prototypes, not native
qualification or timing. Their summaries are
`e4f9dae090fba2213e8a7a9219b43d3a32a49ded55b6e8a8191876e1740eaaca` and
`3eef52f89c7cfa2bf0da47e1dc5d087eb5b67a2d0f0a8f274d868d59ea9ea483`.

The fifth declared candidate changes only explicit OLS CMG polishing: those
compensated operations with binary scaling, strict convergence with zero-energy
underflow rejection, and admitted coefficient corrections when bulk
coefficients are retained. It
preserves ordinary Auto/CG/LSMR, recovery and PPML arithmetic, scientific
tolerances, iteration limits and all sample/rank/omission/separation decisions.
Both analytic amplitudes, extreme scaling, retained products and joint memory
admission must pass before a new native artifact is qualified. Earlier failed
observations remain failures; no performance inference uses them.
The fifth candidate passes its focused checks: 37 CMG tests, six polishing
tests, ordinary CG and memory checks, strict Clippy and formatting. The actual
RHS width can exceed the configured CMG batch: a five-RHS/batch-two witness
verifies reconstruction at the exact admitted memory boundary and rejection
one byte below it. Fresh independent arithmetic and memory reviews found no
blocking issue. Summary
`c3f2a6d3199a0b91138400dcec3fbade8f51e20d90980524269a5cd92496305a`
binds the source and preserved earlier failures. These focused results do not
replace the pending full-suite and native qualification.

The repaired universal Mac artifact is
`72ff85cc33e088ec85c9e30feddc589a3f2aa1068bca1d812e74fba498c3207e`;
its build, slice audits and actual-plugin ABI/self-test passed (transcript
`4f7c678aa2526ce42d849170bd35e34773dd6dfdc3f73cf389a20536a0568b72`).
The first full-suite attempt passed 239 Python tests (13 existing skips),
strict Clippy and 1,331 core tests (three declared exclusions), then stopped at
an outdated testkit assertion requiring Fast final CMG instead of the declared
CG verification. The failed transcript remains
`661969ee53af54272377e59b18a9fc38b05a518a230acbae07c7845857a3284d`.
Updating that diagnostic assertion does not relax any numerical witness or
change the frozen production artifact. Its focused eleven-test oracle suite
and the subsequent full repository suite passed. The completed full-suite
transcript is `c7934a89fbc0034311bda55fe8449594bdf6601339dd6a6272a2c31665297ad8`:
239 Python tests (13 existing skips), 1,331 core tests (three declared allocator
exclusions), 195 plugin tests plus ten isolated allocator audits, scientific
oracles, strict Clippy, fuzz, complexity and bridge guards. The Mac-only run
does not qualify Linux's skipped ELF audit. Production-token identity is
`a9dd2fec088347492570a17d8d8d2d4761570e8444d6e74412ee5fed444899be`;
artifact-input identity remains `1cb7d506617ac2f77e35d8b7d2dc08409b14cf4f9eef348a12bf697a726609eb`.
The source/suite/build record is `5cc5e697c406c8e350d53cfe02848eca04b3b49de596801673699b24165ed5ae`.
Fresh native qualification is separately required; its prospective identity is
`34ae3fce63ddda06a859eea8bde3b87025bf061c44bb77f74916e61691ab81b4`.
Its first eight calls passed the frozen dynamic-weight oracle, fixed-worker
repeats, cross-worker discrete/scientific checks and CMG-versus-LSMR comparison.
CMG's maximum residual errors were `6.51098e-11` and `6.51108e-11` at one and
four workers; LSMR's was `6.36394e-11` at both counts. The eight-call summary is
`4ecb5db2db6330a091725b11fff23c85337fca59f9f8411489f41af346060680`.
This clears the weighted numerical prerequisite without changing its oracle
or tolerances; it is not timing evidence.

The complete fresh native run passed all 1,139 replay calls, eight mapped Stata
fixtures and five Break checks. Terminal transcript:
`3eb756e3177e3e206cef688b78628e083e2f49b6a7ed037c9d5159fe5a1fd180`.
The 73-case ordinary panel has 68 exact parent comparisons and five independently
witnessed, typed OLS CMG scopes; all fixed-worker repeats and 2/4/8/16-worker
scientific/discrete checks pass. Its summary is
`da8ba68125da19d5d178ba6ef9cb059a7dd63879ac36f1bd5cd4f6ad88e84ff4`;
the manifest binding all 55 raw typed results is
`5639f28a355d8cbfd150e35b85580f1cace38a69d59509f5975bfe531745cbc4`.
Supplemental routing/IT, direct Schur, overlap and PPML topology panels also pass:
respectively `d48af7601831f7aa31956e301261560ae149666b2a4270a2b13aeb4940f3089a`,
`5f8b5621bb3a9bc9247126d038d550bd7cc85acb608d9291f76317509ed7f859`,
`9999f8c23363106a442947dd8d3627695cb630f6167c19bb299e0db15e0c9605` and
`a7d8abc715a3ad847981481c9533e61ff92d914d8b286212ae646c5621c1deb1`.
Fresh resource and sampling wrappers are
`846748f1ebfac51b0bc42f99e5aa8c75aee428bd0dff83df4bdc081fb5993d40` and
`21962f7286eb3d6646e450ab92ee45e82dbb49ffbabfaf7544a1acc7af427e54`;
they retain the earlier failures and bind the repaired artifact's checks.

Two new wide-129/t1 CPU samples have identical scientific fingerprints. Their
candidate/baseline top-of-stack counts are dense robust meat 119/116, finite
moment production 84/83, and Schur application 26/24. Review summary
`8786408a4836561ff3cd859bf76200eb8c692fb1c7c7847a511d83eb7a489239`
binds both captures. These statistical samples include loading/I/O and blocked
coordinator stacks; they do not establish speedup or nonregression. A separate
two-call CMG sample could not attach inside the sandbox (sampler exit 255 for
both processes); both native calls returned zero, the repaired result passed
the frozen oracle, and the baseline reproduced its known residual failure.
The failed diagnostic is preserved without retry or a CPU-cost claim.
Its summary hash is
`bf32b39e69ee347ee11091be18441b54579c94eabc0ac0603922af0470161690`.
The new prospective ablation and policy protocols are
`c5e58629f83b19d65588515c6c910bbaf078125c6de850aa494e75cb44cc3afc` and
`e6a17d75eeb6dff193b2c7a2277063be8450771fa4092000e91682de46d6b416`.
They change only artifact/evidence bindings and the separately reviewed
availability/checker contract; original input bodies, finite grid, repetitions,
statistical margins and stage gates remain fixed. These are prospective
protocols, not passing timing results.

The repaired untimed ablation screen completed all 26 cells: 338 initial calls,
320 passing arms and 320 passing repeats. The other 18 arms were the frozen
MAP/IT/compact-IT comparators on weak, high-degree weak and dynamic-weight
designs at one and four workers, with iteration exhaustion or the 120-second
limit. There were no scientific/resource blocks. All 26 checked wrappers are
complete; records hash
`7eef4b409ecae7fdde593239f7886130547392f769f814410f50e5f41ffa9af7`.
These successful checks do not relabel the earlier failed screen and are not
timing observations.

The subsequent policy A correctness screen exposed two checker blocks for
explicit IT on slope models, at one and four workers. The native calls passed
their independent numerical witness, then differed from Auto only in solver
warning-count leaves 9 and 23. Current and parent generic accelerated IT emit
one informational notice per projection, including zero-iteration certification;
Low combines the response and regressor-block notices. The observed primary/
final counts were `2/1`, against Auto's `0/0`. A separately reviewed checker
correction permits only these source-bounded counts for the typed generic-IT
tuple and parsed slope/mixed metadata. It changes no solver behavior or numerical
tolerance. The original blocked observations remain failures; fresh bounded
checks and explicit source-provenance validation are required before their
replacement evidence can qualify timing. Unaffected valid checks are retained.

That original policy screen completed all 42 cells and 864 initial slots:
844 initial/repeat passes, 18 optional exhaustions/timeouts, and the two IT
diagnostic blocks (their repeats were not run). Original records hash:
`1667a286b187dd58c230a2dcb9d24e228614780fd596092d2a18ba6a80e66017`.
The narrow checker patch passed independent source review and 58 focused
producer/deployment tests. Under a prospectively frozen six-call correction,
both workers' fresh baseline control, traced IT call and exact IT repeat passed.
Baseline fingerprints match the originals; the new IT tables and full columns
exactly match the retained blocked observations, and independent science,
resources and protected cross-worker diagnostic tuples pass. The correction
plan is `d26445ab5cbcc9b54e36ccba764165153bf96eca740a2bf3796f7581bf67788c`;
new observations are `6d42559fe2641c78825c272bbe89798141962ae080dc82f7222a229e06b79104`.

The reviewed provenance bridge binds original terminal records, old checker
bytes, the exact patch, unchanged other tools, oracle/input identities and
explicit dependency relocation. It retains all original failures and does not
retry unavailable solvers. New validation evidence passes all 42 policy and
26 ablation wrappers: policy has 846 initial and repeat passes with 18 optional
unavailabilities, ablation retains 320 initial and repeat passes with 18 optional
unavailabilities. New policy/ablation records are
`ad99e628051c0044ccab162ff2eea17a4fafe01bcd18bdcaa08763337328dca4` and
`252ca33bcf9e167d5cd5b77d50e2d9f806abbcce9af306df7e65c97d0c07645e`.
Checker source is `e3c1cd5c05436376f1ed228494e98891092aa415a279799abd18fae91e41e31e`;
reviewed bridge is `2240ccb47d2466258facd1426299a7d872a449e1204cd2fd5d554acc9c014561`.
All of these are correctness/provenance checks; no timing result follows.

Before any timing, an independent architectural review identified that the
original C stage could qualify only a hidden injection, not an activated public
default. The prospective, wiring-only deployment amendment was reviewed and
sealed under the [SPEC contract](SPEC.md#solver-support-and-dispatch).
One passing nonbaseline B policy may receive fresh non-C qualification and be
frozen as an uninjected public OLS Auto candidate before C access. C retains its
original data, statistical rules and comparators and additionally protects the
selected hidden artifact. Failure, inconclusive evidence or no B winner leaves
the incumbent default. Eight focused synthetic tests pass; independent Astra
review found no remaining blocking source issue. Declaration hash:
`9b5a467ad680739862f29369c9d0912f7d772b94270d1bc55c4d5399c5434446`;
validator source: `f21bcdd3423468d06921b8b22708dd32e4826d24aca9019c06ddbdcf45489de1`.
The tool validates bindings and reviewed evidence; chronology and one-attempt
enforcement remain operational controls. Public execution hooks remain deferred
until a B winner exists. No production default has changed and no timing has
started.

Raw captures are disposable; these compact outcomes, identities and maintained
independent witnesses are the retained record.

## Serial routing development candidate, 2026-10-14

This serial development artifact implements opt-in routing through sanctioned hidden OLS
injections: checkpointed CMG-Schur, one fixed categorical-pair replacement within
LSMR, and separate diagonal/cell/direct experiments. Ordinary `solver(auto)` is
unchanged. The coefficient-space explicit categorical IT path is also a
**development candidate, not qualified for promotion**. [SPEC](SPEC.md#solver-support-and-dispatch)
records the scientific scope, certificates, shared budgets and refusal contract;
[the protocol](plugin_protocol.md#developer-injections-hidden-adapters-only)
records injection validation. This entry establishes no performance, default
promotion, release-candidate or whole-plan-completion claim. This artifact does
not include concurrent CMG preparation or PPML routing changes; subsequent
implementations require their own evidence.

These SHA-256 identities bind the results to the tested universal Mac artifact:

| Identity | SHA-256 |
| --- | --- |
| Universal Mac plugin | `a00a78f1e158ebc5d4ef1273182c45a7b1430b880c1e02205bd6c646b3837d5a` |
| Production tokens | `6fc8a81e94b022b2ce14a8a571cac34f399eabf3047d10f1b6bc63f0ed6aee57` |
| Artifact inputs | `14ab925f5124c01c22e5020ff2f7c07712e7755fff17016a2a7feaf70477fe6c` |
| Source-identity record | `d7c53be9fbd3e0058c2791d3e3f2260e26a87802fab33ae7db4dc5e235c77911` |

This tested artifact used CMG
`c6ecb0a56bb16c9e2b9e28180123b29d6ee8109d` and within/schwarz-precond
`0a828c5038552eede16a12b057aeb7b6be261923`. Subsequent development uses the
published [dependency revisions](../crates/fereg-core/Cargo.toml)
CMG `b989eddfc3bf7ad130c76df8d0cceaf9801e1d72` and
within `efe35ee386ad989b043a5076d01d879ca410ff19`; the latter preserves the
genuine merge of the prior pin with upstream main and adds allocation gates. Both were normally pushed with CI skip
markers, with zero push workflow runs reported. The serial evidence below
does not substitute for verification of that later implementation.
The comparison plugin is
`3539b15fb777360f1de549697319ee77ed5cd30bc08811d51c2dbad5c73c226a`.

Completed checks on the candidate:

| Check | Result | Capture/summary SHA-256 |
| --- | --- | --- |
| Full `scripts/run_tests.sh` | Pass, including scientific oracles, both strict Clippy modes, Rust tests, allocator audits and complexity ratchet; GNU ELF audit correctly skipped on Mac | `f7167b5d7b75ade65b52b1f0c89985c39a68a1574770b977e34e4d5d70130cac` |
| Universal Mac build and ABI/load/self-test | Pass | `8d26c3d20d9976b7cbbbbd6058e166414b2bc835da7ebc569443f5304cd1372a` |
| Ordinary native integration replay | 73 cases pass with zero exact differences; 2/4/8/16-worker science, discrete-decision and fixed-worker repeat checks pass | `f56a5fcecd52ead8122aa3922ae4fda2e73e6f6ecd3576b2ff0af3672eb6533e` |
| Supplemental native routing/IT replay | 23 cases pass at 1/4 workers with declared numerical scope, exact unaffected fields and repeatability | `6ef2916e85788bf73d8bd386a1bcda6a3de63ac9a087625d9028bf97223d7a98` |
| Corrected direct native panel | Four unit/weighted Fast/Low cases pass at 1/4 workers, including certified Fast admission of all 65 active RHSs | `b06c90f1e2111e50e55a13da804782a123105db64f8e44adb14a519f7aac3f01` |
| Mapped licensed Stata | Eight fixtures pass with successful process and Stata return codes, including explicit solver, reuse, OLS transaction and unchanged PPML boundaries | `1e95f07f361f7927ec122b1555b5f13fdea9489a6fc7c1eb570197e42e19dc54` |

Three final licensed Break checks pass: OLS Auto
(`333a6ad1b7a72ef326613576c7b3dfe0a347ea3cf31b866fcd50605b6b672cf6`), sampled CMG-Schur
(`a1f32c86df4aa95171e2882a50b6274b0731c50d611841d211d95caf97747002`), and PPML
(`f5aada3ea8df93ebc394b615bb07031d17c06bbe03831bcbf2f1ade2b1a2f3fb`). The sampled check covers the native wait
checkpoint and postplugin publication interruption; it does not establish
parallel-build cancellation.

Seven focused [direct-admission tests](../crates/fereg-core/src/fe/routing_direct.rs)
pass, as does the independent [full-batch dense-QR witness](../crates/fereg-oracle/tests/routing_direct.rs)
covering 65 packed RHSs, Fast/Low, unit/unequal weights, 1/4 workers, HC1 and saved
FE reconstruction. Their capture hashes are
`c6c759a6b9ffb1232ba018eb70efcda1f75ff872dd4b6cd68ec51f5f840a21e2` and
`185bc2705d61e7435e35d929ad35dc7c89ef66e19906077ff29906421e491bf0`. The first native direct panel incorrectly
expected width 64: the actual batch has 63 finite controls, the reported
intercept and the outcome, hence 65 RHSs (64 reported coefficients). Correcting
only that assertion produced the passing panel above, without a production
edit. The original failed summary remains identified by
`f83d6fba2b82b1e986393115ad068267278438ccd50414571ef8b28023d3eab0`; its failure was the admission
assertion, with no scientific or discrete-decision errors.

Explicit IT qualification remains failed. The frozen 38-cell old/new panel
(protocol SHA-256 `bd0cb11e0374410156cd16d0452bb8ce0afa8c217f5ba0db05ab216a1b47f6b3`) passed its first 16 scientific
comparisons, then stopped when **old IT** on `weak-t1` returned r(459), iteration
exhaustion, before dispatching the candidate on that cell. The failed attempt
is retained by SHA-256
`fb1c97522ee8888ea4092eb4abf408c58aaf0b4a620e0df3ffa70150a9e45dbe`.
A separate untimed completion reused only those 16 individually verified cells
and the observed old-IT failure, then obtained terminal observations for every
remaining arm under the same 120-second process cap, without retries. Both
versions pass 32 of 38 cells: 16 reused and 16 newly completed, with independent
truth, two exact repeats, scoped compatibility and cross-worker checks.

| Remaining family | One worker, old / candidate IT | Four workers, old / candidate IT |
| --- | --- | --- |
| Weak graph | r(459) / r(459), iteration exhaustion | r(459) / r(459), iteration exhaustion |
| High-degree weak graph | 120-second timeout / timeout | 120-second timeout / timeout |
| Dynamic weights | r(459) / 120-second timeout | r(459) / r(459), iteration exhaustion |

Thus the two versions share the six unsuccessful cells, but their failure modes
differ on one-worker dynamic weights. The completion summary SHA-256 is
`d7d1deeafabc553a2ad16fd40b62d8df0252eb1ad86e63ead45b40204af750bc`;
its protocol file SHA-256 is
`ebbbf0601ec830776da92231df5f1d50e561b9a84417b7b36a92ca0b6f209d19`
(canonical protocol identity
`5f2b4afaa526718e1c4fc018c1c4e5f1859245dbed470d594f708d76c08ecac0`).
The completion uses the candidate and comparison plugin identities above and
retains `qualification=false` and `performance_claim=false`. All observations
are terminal; candidate-wide scientific acceptance and the original all-success
qualification remain false. No timing qualification was run. The passing
[dense-QR IT witnesses](../crates/fereg-oracle/tests/irons_tuck.rs) and supplemental
replay do not override these failures.

Still missing are clean whole-policy performance qualification, the routing
selection/holdout campaign, real-world firm-worker routing evidence, new Linux
or Windows qualification, concurrent-build qualification and PPML routing
qualification. Existing historical timings and platform results below do not
supply those missing checks. Raw task captures are disposable; the identities,
compact outcomes and maintained source witnesses above are the retained record.

## Sampled CMG-Schur extension, 2026-10-13

The opt-in `cmgsamples(k)` extension replaces above-cutoff worker stars with
averaged weighted trees on their distinct firms. For example:

```stata
fereg y x, absorb(worker firm) solver(cmgschur) cmgdegree(4) cmgsamples(1) cmgseed(0)
```

The default count is zero, preserving the existing hybrid graph and scientific
bits. Positive counts sample only the preconditioner: the implicit Schur
operator, observation reconstruction and original-problem certificate remain
exact. Low-degree worker cliques remain exact too. Each weighted tree connects
every distinct firm of its worker. SplitMix64 streams are keyed by seed, worker
and tree, and the graph stays fixed throughout PCG and residual correction.
The implementation retains lazy construction, hierarchy reuse and batched
fixed-cycle application. Count/seed and memory validation occur before work;
nonfinite or underflowed edge weights fail explicitly. See [SPEC](SPEC.md) and
[the protocol](plugin_protocol.md) for the supported range and numerical scope.

The [aggregate evidence](../bench/results/cmgschur-sampled-20261013.json),
[all sampled-panel observations](../bench/results/cmgschur-sampled-20261013-observations.csv)
and [standard observations](../bench/results/cmgschur-sampled-20261013-nonregression.csv)
retain the predeclared protocol, artifact/input identities, graph diagnostics,
all repetitions and unsuccessful validation attempts. The parent is
`f9bf345e50c4c853f29e0171538b68c03e0109df`; the candidate universal Mac plugin is
`3539b15fb777360f1de549697319ee77ed5cd30bc08811d51c2dbad5c73c226a`.
Production-token and artifact-input hashes bind these measurements to the
implementation; subsequent evidence-only edits do not change those inputs.

The bounded panel uses three synthetic 600,000-row graphs, two controls,
uniform observation weights, cutoff four, seed zero, tolerance 1e-8, 1/4 threads,
one warmup and seven rotating repetitions. A separate 3,000-row degree-30 dense
OLS/HC1 prerequisite gates each arm/thread; every input/artifact also has a
strict single-worker LSMR reference at 1e-12. Complete native-call time includes
loading, estimation, inference and reconstruction, and excludes replay-file I/O.
The 2,400-second total and 120-second process limits were unchanged; the panel
finished in 394 seconds with no failed cells or extra repetitions.

**These are exploratory timings, not clean release-performance qualification.**
All builds, tests and agent workloads stopped before timing. Spotlight still
used roughly one CPU core after a bounded 90-second wait, so this limitation
was declared before the first timed panel. Spotlight later subsided, but desktop
activity continued; both panels retain the conservative exploratory label.
No unrelated application or service was stopped. Bootstrap intervals describe
the observations, not a guarantee against background-load confounding.

| Synthetic input | Threads | auto (s) | CMG (s) | Hybrid (s) | 1 tree (s) | 2 trees (s) | 4 trees (s) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Mixed, maximum degree 30 | 1 | 0.22828 | 0.59157 | 0.31511 | 0.28234 | 0.30418 | 0.30605 |
| Mixed, maximum degree 30 | 4 | 0.11869 | 0.26429 | 0.19778 | 0.18583 | 0.21460 | 0.21184 |
| Weak connections, maximum degree 30 | 1 | 0.46040 | 1.43691 | 0.39723 | 0.25788 | 0.25545 | 0.26603 |
| Weak connections, maximum degree 30 | 4 | 0.19737 | 0.50028 | 0.24791 | 0.15489 | 0.15747 | 0.16453 |
| Degree-60 stress | 1 | 0.20622 | 0.61988 | 0.38018 | 0.34946 | 0.38419 | 0.40001 |
| Degree-60 stress | 4 | 0.11135 | 0.27770 | 0.21856 | 0.25941 | 0.29440 | 0.30616 |

One tree's paired median ratio to the hybrid is 0.648/0.624 on the weak graph
at 1/4 threads, but 1.187 on degree-60 at four threads. Thus high degree alone
does not make sampling faster. Auto wins on the ordinary mixed and degree-60
graphs. Increasing the sample count increases assembly and graph fill and does
not reliably repay that cost. This evidence supports an explicit experimental
option; it does not support changing the default solver or sampling count.
There is no new real-data, Veneto, SCC, Linux or Windows claim in this extension.

Graph diagnostics clarify the storage tradeoff. The weak graph's hybrid has
6,857 vertices and 121,626 merged edges; one tree has 1,000 vertices and 42,058
edges. For degree-60, sampling removes all 4,000 auxiliary worker vertices,
but four trees produce 945,000 raw contributions and 378,346 merged edges,
versus the hybrid's 241,000 edges. Fewer vertices need not mean a cheaper CMG
hierarchy or better thread scaling.

Validation completed:

- The full `scripts/run_tests.sh`, universal Mac build, ABI/load audits and
  18 benchmark-tooling tests pass. The suite includes 206 Python tests (13
  platform/environment skips) and 1,800 successful Rust test executions,
  including isolated subprocess and allocation audits. The GNU ELF audit is
  correctly skipped on Mac; this is not cross-platform qualification.
- Exact enumeration independently checks the weighted-tree distribution and
  averages of 1–4 trees. Dense weighted dummy QR/HC1 witnesses cover 32
  degree-30/60 configurations with repeated cells, weak links, disconnected
  components, omissions, saved effects, both memory modes, seeds and threads.
  Dedicated witnesses check kernel preservation, symmetry, positive energy,
  weight scaling, pinned RNG answers, admission and edge underflow.
- All 358 native panel fits pass the existing science thresholds and fixed-thread
  exact-repeat guards. Maximum sampled-output difference from strict LSMR is
  2.52e-13; the maximum across all arms is 2.83e-10. Unsampled CMG-Schur exactly
  matches the parent, including solver signatures. The existing 73-case native
  replay also has zero exact differences.
- Seven mapped licensed Stata fixtures pass, including the new weighted
  degree-30 fixture, exact maximum-seed transmission, invalid-option rollback,
  genuine iteration exhaustion, existing CMG oracles, memory and eclass
  transactions. Sampled native-wait and postplugin Control-C checks pass.
  A separate isolated core witness cancels during actual sampled-edge emission
  and observes the 1,024-edge checkpoint.
- All 20 frozen standard workloads pass exact parent/candidate replay and their
  numerical timing allowance, max(5%, 1 ms), with seven pairs each. All 320
  observations are retained; no continuation was used. The background-load
  caveat prevents promoting this to clean performance qualification.

The extra legacy auto-CMG fixture again fails its obsolete code-seven assertion,
as already documented below and reproduced on the previous parent. An initial
transaction invocation used an incorrect path; the maintained fixture then
passed. Clippy's seed-bound style finding was fixed, and a source-archive check
that overlapped editing was rerun after all sources were frozen. The final full
suite passed. No scientific tolerance, model decision or production solver
policy was relaxed to obtain these results.

## CMG-Schur optimizations, 2026-10-13

The explicit `cmgschur` route now batches fixed CMG cycles across right-hand
sides, skips inactive and exactly zero projected columns, fuses component
summation with gather and centering with scatter, and parallelizes independent
search-direction updates. It constructs the hierarchy only after the initial
requested-tolerance certificate fails, then reuses it. The recurrence initially
targets half the requested tolerance; a failed full certificate permits one
strict correction within the original iteration allowance. Successful results
still require the existing requested-tolerance certificate. Sample, rank,
omission, separation and automatic routing policies are unchanged.

The additive CMG batch API is pinned at
`f45c3b8b2895a9fdcf32db3b768717230ed594b9`. A separately admitted scalar workspace
serves projections whose original width is one. Multi-RHS projections retain
the batch cycle even when only one active column remains, keeping the selected
preconditioner fixed throughout each solve. These algorithm and reduction-order
changes have the explicit `cmgschur` scope declared in [SPEC](SPEC.md); existing
auto, CMG, LSMR and PPML scientific outputs and solver signatures remain exact
in the parent/candidate replay.

The [retained aggregate evidence](../bench/results/cmgschur-opt-20261013.json),
[final observations](../bench/results/cmgschur-opt-20261013-observations.csv),
[initial candidate observations](../bench/results/cmgschur-opt-20261013-initial-observations.csv)
and [standard timing observations](../bench/results/cmgschur-opt-20261013-nonregression.csv)
preserve both candidates, input/artifact identities and the predeclared protocol.
The parent is `17b4e884d9ef03e81478f660b70ab1b733d5d5da`. The final universal Mac
plugin SHA-256 is
`654d867aec07e62e7e656d928cd47b90259201a5eaa29fe941ac07bf33ac585e`;
the final Linux plugin is
`fe877392b315aae65f1e195fbee1b2ccd2d048cb8a9e12d7766a6af40457048d`.
Source token and artifact-input digests bind the measured candidate to the
committed implementation; later evidence-only edits do not alter those inputs.
Maintained reproduction tools are described in [bench](../bench/README.md#cmg-schur-prototype-comparison).

The final Mac panel uses the same M2 Ultra, one warmup and seven repetitions,
rotating paired arm order, 1/4 threads, tolerance 1e-8 and identical requested
outputs. Five inputs cover short and weakly connected panels (100,000 rows),
a long panel (one million rows), a 16-control panel (100,000 rows), and a small
wholly FE-orthogonal case. Each input/artifact has a strict single-worker LSMR
reference at 1e-12; a separate dense OLS/HC1 prerequisite gates every arm and
thread count. The fixed budget is 2,400 seconds with 120 seconds per process.
All 668 invocations pass science checks and fixed-thread repeatability; the
largest output difference from the strict reference is 2.77e-11. Quiet Mac
timing excludes competing builds, tests and agent tool workloads.

The primary clock surrounds the complete native call, including loading,
projection and reconstruction, but excluding external input parsing and output
export. The table reports median seconds for cutoff four; all cutoffs and
paired bootstrap intervals are retained in the evidence.

| Mac input | Threads | Parent cmgschur 4 | Optimized cmgschur 4 | Time change | auto | cmg |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Short | 1 | 0.04916 | 0.04792 | -2.5% | 0.04590 | 0.17926 |
| Short | 4 | 0.03465 | 0.03420 | -1.3% | 0.02569 | 0.07620 |
| Long | 1 | 0.61986 | 0.49557 | -20.1% | 0.45260 | 1.09796 |
| Long | 4 | 0.31565 | 0.30058 | -4.8% | 0.21619 | 0.46199 |
| Weakly connected | 1 | 0.04607 | 0.04259 | -7.5% | 0.09152 | 0.34717 |
| Weakly connected | 4 | 0.02657 | 0.02689 | +1.2% | 0.03736 | 0.11830 |
| Sixteen controls | 1 | 0.13782 | 0.12106 | -12.2% | 0.10532 | 0.23582 |
| Sixteen controls | 4 | 0.08424 | 0.08379 | -0.5% | 0.05609 | 0.12033 |

The clearest gains are at one thread. The long-panel four-thread paired ratio
is 0.9543 with a 95% bootstrap interval [0.9310, 0.9769]; weak and wide
four-thread cells are effectively unchanged. Auto remains faster on ordinary
synthetic short, long and wide inputs, while CMG-Schur wins on the weak graph.
The 600-row orthogonal case avoids hierarchy construction and saves only
0.17/0.26 ms at 1/4 threads, despite larger percentage changes. Its fixture
explicitly excludes an intercept; a preliminary diagnostic with an implicit
intercept was corrected before either timing panel and is labeled separately.
These are combined-optimization measurements, not individual ablations or
evidence for changing the default solver or choosing a universal degree cutoff.

The initial candidate was about 6% slower on the long four-thread input. Traces
identified the final single-RHS projection using a capacity-four batch workspace;
its median solve interval rose from 37.05 to 76.39 ms. One predeclared production
follow-up added the scalar workspace described above. A fresh complete panel
on the final artifact gives 34.96 versus 37.49 ms for that interval, and improves
the full call by 4.8%. Timings from different artifacts are never pooled. Long
main/final iteration counts change from 9/6 to 7/7. Whole-process RSS, including
the replay host, rises from 237.5 to 238.8 MiB at one thread and 255.1 to 257.1 MiB
at four threads for cutoff four.

The matched real-data follow-up uses the same selected Veneto 1996-2001 panel:
5,163,428 rows, 1,106,419 workers and 86,647 firms, with the existing adjusted
outcome, no additional controls, unit weights and robust covariance. Source
selection and transformation are unchanged; row-level data stayed on SCC.
The run reserves all 28 physical cores of an Intel Xeon Gold 6132 node, with
1/4-thread fits sharing CPU mask 0,2,4,6 and NUMA node zero. It uses GCC 12.2.0,
Python 3.12.4, NumPy 1.26.4 and Rust 1.85.1. The fixed design has one warmup,
seven paired repetitions, a 2,400-second budget and 300-second process cap.
All 40 invocations pass, including strict references, dense prerequisites,
fixed-thread repeatability and discrete decisions; terminal job accounting
also passes. The final Linux build separately passes 20 smoke invocations.
The final output difference from the strict reference is at most 2.74e-12;
main/final iterations change from 17/18 to 13/19 at both thread counts.
These SCC seconds are not directly comparable with Mac seconds.

| Veneto cutoff four | Parent (s) | Optimized (s) | Time change | Paired ratio, 95% bootstrap interval |
| --- | ---: | ---: | ---: | --- |
| 1 thread | 6.20014 | 5.82412 | -6.1% | 0.9408 [0.9333, 0.9447] |
| 4 threads | 4.06824 | 3.78044 | -7.1% | 0.9289 [0.9126, 0.9405] |

This final real-data panel compares only parent and optimized CMG-Schur at
cutoff four. The earlier prototype's auto/CMG/cutoff comparisons below remain
separate evidence. The initial optimization candidate's Veneto observations
are also retained separately and are not pooled with the final candidate.


Validation on the final artifact passes:

- `./scripts/run_tests.sh`: 1,783 passing Rust test executions, formatting,
  clippy, bridge/guard checks and the complexity ratchet; universal Mac build,
  ABI checks and self-test also pass.
- Actual-plugin parent replay: 73 cases, zero exact scientific or solver-signature
  differences. Seven mapped licensed Stata fixtures and native/postplugin Break
  checks pass, including rollback, memory and eclass transaction coverage.
- Independent witnesses: the original 48 dense configurations plus 144 mixed-RHS
  and 16 wholly FE-orthogonal configurations; 72 sealed projection configurations
  remain covered. Dense QR checks coefficients, residuals, HC1 and saved-FE sums.
- All 20 standard timing cells pass the existing `max(5%, 1 ms)` allowance, with
  exact outputs across all 380 final-artifact records. The first final pass had
  18/20 conclusive cells. The predeclared single continuation added 14 pairs only
  to the two inconclusive four-thread cases (64 and 129 regressors), pooling all
  21 pairs per case. Their final 98.75th-percentile paired-median bootstrap upper
  bounds are 1.02923 and 1.04761, below 1.05 (100,000 draws, seed 739391; nominal
  coverage is approximate). Initial-candidate evidence, including its 19/20
  conclusive standard cells, is retained separately. There were no further looks.
- Fourteen benchmark-tool tests pass. Bounded Astra design assistance and
  independent high-effort architecture review cover the fixed-cycle API,
  recurrence, lazy admission and scalar correction. An initial diagnostic
  setup/solve double count was corrected before timing.

The upstream CMG full suites still have two pre-existing `hierarchy_admission`
live-allocation equality failures, independently reproduced on the clean parent:
`prepared_numeric_generations_share_parents_but_admit_new_builds` and
`serial_hierarchy_bounds_preserve_all_stopping_and_component_policies`.
Default and all-feature release runs report 101/177 passing tests respectively
and those same two failures. All new batch tests pass across default,
parallel-only and all-feature configurations; formatting, clippy and rustdoc
also pass. This is not a claim that every upstream test passes.

Core production lines increase by 324 and dispatch candidates by seven; long
file/function counts remain unchanged. The scoped increase is recorded in the
complexity ratchet. No benchmark-specific routing or default cutoff change was
introduced.

## CMG-Schur prototype, 2026-10-13

`solver(cmgschur) cmgdegree(4|5|6)` is an opt-in OLS prototype for exactly two
categorical intercept FEs; the experimental default cutoff is five. It uses the
existing implicit all-worker Schur operator with a fixed CMG cycle built on a
hybrid graph. Workers above the cutoff remain auxiliary vertices only in that
preconditioner. Weights, controls, saved effects, disconnected components and
both memory modes are covered. Automatic routing and PPML are unchanged.

The [retained aggregate results](../bench/results/cmgschur-20261013.json),
[individual solver observations](../bench/results/cmgschur-20261013-observations.csv)
and [standard timing observations](../bench/results/cmgschur-20261013-nonregression.csv)
contain the numerical checks, input/artifact hashes, timings and diagnostics.
The maintained generator and runner are documented in [bench](../bench/README.md#cmg-schur-prototype-comparison).
No row-level data or raw logs are retained in Git.

The synthetic panel ran on an Apple M2 Ultra Mac with 24 cores, with no competing
builds, tests or agent workloads. All arms use FE tolerance 1e-8 and the same
inputs, controls, robust covariance and requested residual/xb/xbd/absorbed-sum
outputs. A strict single-worker LSMR fit at 1e-12 precedes each input; a separate
600-row dense dummy OLS/HC1 oracle gates every arm and thread count. Each cell has
one warmup and seven sequential repetitions with rotating paired arm order,
a 30-minute total budget and a 90-second process cap. All 254 invocations passed
scientific checks, including exact fixed-thread repeats and discrete decisions.

The primary clock surrounds one complete native plugin call; input parsing and
result export are excluded. Internal `RRUNTIME` native/total intervals and solver
setup/solve intervals are separate diagnostics, not the primary clock and not an
exhaustive decomposition. These timings include reconstruction work as requested
by the harness. In particular, the existing CMG arm uses its CG route for the
final projection; this comparison does not isolate hierarchy quality alone.

| Mac synthetic input | Threads | auto (s) | cmg (s) | cmgschur 4 (s) | cmgschur 5 (s) | cmgschur 6 (s) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Short, 100,000 rows | 1 | 0.0458 | 0.1699 | 0.0468 | 0.0484 | 0.0474 |
| Short, 100,000 rows | 4 | 0.0285 | 0.0807 | 0.0371 | 0.0378 | 0.0367 |
| Long, 1,000,000 rows | 1 | 0.4688 | 1.1463 | 0.6398 | 0.6693 | 0.6980 |
| Long, 1,000,000 rows | 4 | 0.2199 | 0.4776 | 0.3246 | 0.3378 | 0.3531 |
| Weakly connected, 100,000 rows | 1 | 0.0936 | 0.3525 | 0.0473 | 0.0464 | 0.0468 |
| Weakly connected, 100,000 rows | 4 | 0.0388 | 0.1253 | 0.0267 | 0.0268 | 0.0266 |

Values are medians of seven full-call observations. The hybrid beats full CMG
in every synthetic cell, but beats auto only on the weakly connected input:
roughly 50% lower time at one thread and 31% lower at four. On the long input,
cutoff four is fastest among the hybrids. Raising the cutoff from four to six
reduces vertices from 12,589 to 6,788 but increases merged edges from 123,073 to
160,489. All three take nine main-solve iterations; the extra fill costs more
than the small final-projection iteration saving. The weak-graph cutoff
differences are small, and all short-panel workers have degree at most two, so
its three cutoffs construct the same graph. No universal cutoff or automatic
routing change follows from these observations. Median whole-process RSS on the
long one-thread input is 224 MiB for auto, 256 MiB for CMG and 238/241/242 MiB for
hybrid cutoffs four/five/six; this includes the replay host's observation bank.

The real-data follow-up uses the existing selected Veneto 1996-2001 adjusted
outcome panel described in the census below: 5,163,428 input rows, 1,106,419
workers and 86,647 firms. No new outcome transformation or input selection was
applied. This is a pure worker/firm AKM fit with no additional controls, unit
weights and robust covariance, with the same requested output types as above.
The private input and row-level intermediates stayed on SCC.

The run reserves all 28 physical cores of an Intel Xeon Gold 6132 node for
isolation; one- and four-thread fits share CPU mask 0,2,4,6 and NUMA node zero.
It uses GCC 12.2.0, Python 3.12.4, NumPy 1.26.4 and Rust 1.85.1. The same fixed
seven-repetition design has a predeclared 5,400-second total budget and
300-second process cap. All 92 invocations pass, including the strict reference,
dense prerequisite and exact repeats. The maximum observed output difference
from the strict reference is 1.34e-11 across all arms and at most 4.30e-13 for
CMG-Schur. These SCC seconds must not be compared directly with Mac seconds.

| Veneto 1996-2001 | auto (s) | cmg (s) | cmgschur 4 (s) | cmgschur 5 (s) | cmgschur 6 (s) |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 thread | 15.4161 | 79.8875 | 6.2524 | 6.3068 | 6.4541 |
| 4 threads | 9.5920 | 30.1074 | 4.0475 | 4.0943 | 4.1704 |

Cutoff four lowers full-call time relative to auto by about 59% at one thread
and 58% at four, and relative to CMG by about 92% and 87%. Its main/final
projection iteration counts are 17/18, versus auto's 71/132 and the CMG arm's
41/95. Cutoffs five and six use 18/17 and 18/19, respectively. Those counts are
stable across both thread counts. Cutoff four is the conservative starting
point on these inputs; the sub-5% differences between Veneto cutoffs remain
small performance observations, not a universal optimality claim. Increasing
the cutoff has not produced a speed gain here. The explicit prototype remains
configurable, and auto stays the package default.

At four threads, median whole-process RSS is 839 MiB for auto, 1,289 MiB for
CMG and 840/842/842 MiB for the three hybrids. Linux setup/smoke job 8062967
finishes with failed=0 and exit_status=0 in 254 seconds (four slots, maxvmem
2.790G). The isolated benchmark job 8066089 finishes with the same clean
accounting in 2,196 seconds (28 slots, maxvmem 2.206G). These job durations include
preparation and verification and are not solver timings. Linux build/native-load,
independent Rust oracle and synthetic smoke checks pass; this bounded evidence
does not establish general Linux or Windows qualification.

Scientific and transaction qualification:

- `./scripts/run_tests.sh` passes, including 203 Python tests, Rust/FFI tests,
  allocator audits, complexity/static checks and mock bridge guards. The new
  benchmark tooling has ten additional passing tests.
- Independent dense weighted OLS witnesses cover 48 configurations; sealed
  hundred-digit projection witnesses cover 72. Hybrid/Schur algebra, duplicate
  cells, weight rescaling, disconnected components, multilevel cycle symmetry,
  linearity, positive energy and admission failures have dedicated witnesses.
- Universal Mac build/load checks pass. All 73 native replay cases have zero
  exact scientific differences from the preserved parent plugin.
- Licensed Stata tests pass for weighted estimates, saved effects, all cutoffs,
  both storage modes, invalid-request rollback and real iteration exhaustion.
  Native and postplugin Control-C tests pass. Four mapped existing oracle,
  solver-report and memory fixtures pass.
- The extra legacy `estimate_ols_auto_cmg_route.do` fixture fails its obsolete
  first assertion expecting auto to report CMG code seven. The identical failure
  occurs on the preserved parent plugin; it is not a prototype regression.
- A pre-existing Low-mode finite-score failure for exactly zero residuals was
  reproduced for auto, CMG, LSMR and CMG-Schur. Low-mode sealed witnesses add
  exactly FE-orthogonal paired noise, preserving the known fitted values and
  giving analytically known residuals/objectives. Fast retains the original
  witness inputs. No scientific tolerance or production behavior was relaxed.

All 20 frozen standard workloads satisfy the timing nonregression criterion
against the parent plugin, with exact science. Nineteen passed the original
seven-pair panel. One had median ratio 1.00063 but upper bound 1.05508 against
1.05. A predeclared, single bounded continuation added 14 pairs on unchanged
artifacts, retaining all original observations. The pooled 21-pair median ratio
is 1.00033 and the stricter 98.75th-percentile bootstrap upper bound is 1.00886.
This passes the nominal criterion with an allowance unchanged at 5%; percentile
bootstrap coverage is approximate. The retained record includes the initial
inconclusive result, the continuation rule and all pairs. No further sampling
was performed. An independent architecture review found no actionable defects.

Artifact SHA-256 identities:

- Preserved Mac parent: `85dbf448ab1974122a47684ac9b9f17964b451109b267df409963200cdfbe9cb`.
- Qualified Mac prototype: `6a71297777eaddfa07b02dcc80aa0a2a138c990fdea0bf2e390f9572662f9cd4`.
- SCC Linux prototype: `3df6922745509cbe6eba9370b9859f36f34601e5c5215d25ae3c64f56a5c53a4`.
- Frozen source archive based on `9ee03a688882c835725d2d7c40056465df41e2d7` plus
  this prototype: `3a7c34145af1bfade340533e0e81cabf09b5bcb9505c0d40cb6174ed1e701289`.

## Worker elimination degree census, 2026-10-13

A read-only census at source `414b5f6a` examines whether an experimental CMG
graph should eliminate workers through degree four, five or six. Degree counts
distinct observed employers within the specified sample, after collapsing
repeated worker-employer observations. These are graph construction statistics,
not solver timings, a selected default or a change to the estimator.

The German source is the annual June 30 main-job extract for West German male
full-time workers with nonmissing outcome and identifiers. Its employer key is
an establishment ID. The nine-year subset adds only year >= 2001. The BW subset
adds years 2008-2009 and workplace state 8, matching the current benchmark do-file
without its optional worker-ID smoke cap. No new component, singleton, mover or
weight filtering was applied: these are input graphs, not verified estimation
samples. The Veneto six-year source is the existing 1996-2001 pooled adjusted
sample, already subjected to dominant annual-job cleaning, component selection,
KSS leave-worker-out pruning and retention of workers with at least two rows.
The samples therefore describe different selection rules and observation windows.

| Input window | Rows | Workers | Employers | Distinct worker-employer links |
| --- | ---: | ---: | ---: | ---: |
| West Germany 1979-2009 | 332,461,473 | 26,915,209 | 3,362,020 | 63,635,971 |
| West Germany 2001-2009 | 91,045,290 | 15,487,374 | 1,737,931 | 24,216,740 |
| BW 2008-2009 | 3,566,502 | 2,006,360 | 168,875 | 2,111,088 |
| Veneto 1996-2001, selected | 5,163,428 | 1,106,419 | 86,647 | 1,700,466 |
| Veneto public two-year example | 71,614 | 35,807 | 5,301 | 44,050 |

| Worker degree | West 1979-2009 | West 2001-2009 | BW 2008-2009 | Veneto 1996-2001 | Veneto two-year example |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 | 11,591,732 | 9,647,077 | 1,901,632 | 666,090 | 27,564 |
| 2 | 6,162,213 | 3,773,995 | 104,728 | 318,508 | 8,243 |
| 3 | 3,767,380 | 1,435,996 | 0 | 94,888 | 0 |
| 4 | 2,302,057 | 473,980 | 0 | 22,437 | 0 |
| 5 | 1,362,748 | 125,514 | 0 | 4,028 | 0 |
| 6 | 780,545 | 26,036 | 0 | 468 | 0 |
| Above 6 | 948,534 | 4,776 | 0 | 0 | 0 |

For cutoff k, retain H workers whose degree exceeds k. The hybrid has J+H
vertices. Each eliminated degree-d worker contributes d(d-1)/2 weighted firm
pairs; equal pairs are merged by summing weights. Each retained worker contributes
d star edges to its own auxiliary vertex. The following counts enumerate and
deduplicate the actual firm pairs; they do not assume that every clique
contribution creates a new stored edge. All counts are undirected.

| Input window | Cutoff | Retained workers H | Vertices J+H | Raw edge contributions | Merged edges |
| --- | ---: | ---: | ---: | ---: | ---: |
| West 1979-2009 | 4 | 3,091,827 | 6,453,847 | 50,486,140 | 40,657,594 |
| West 1979-2009 | 5 | 1,729,079 | 5,091,099 | 57,299,880 | 43,602,492 |
| West 1979-2009 | 6 | 948,534 | 4,310,554 | 64,324,785 | 47,496,683 |
| West 2001-2009 | 4 | 156,326 | 1,894,257 | 11,743,628 | 8,004,338 |
| West 2001-2009 | 5 | 30,812 | 1,768,743 | 12,371,198 | 8,291,326 |
| West 2001-2009 | 6 | 4,776 | 1,742,707 | 12,605,522 | 8,432,441 |
| Veneto 1996-2001 | 4 | 4,496 | 91,143 | 760,742 | 562,732 |
| Veneto 1996-2001 | 5 | 468 | 87,115 | 780,882 | 576,575 |
| Veneto 1996-2001 | 6 | 0 | 86,647 | 785,094 | 579,790 |

Vertices include all source employer IDs. Employers isolated after degree-one
elimination may be omitted or pinned separately; these are not guaranteed active
CMG dimensions. The two-year examples have maximum degree two and cannot
distinguish cutoffs four, five and six: BW gives 168,875 vertices and 66,190 merged
edges, while the public Veneto example gives 5,301 and 5,921.

Raising four to five reduces vertices by 21.12% in the long German panel, 6.63%
in the nine-year panel and 4.42% in Veneto, while increasing merged edges by
7.24%, 3.59% and 2.46%, respectively. Raising five to six reduces vertices by
15.33%, 1.47% and 0.54%, while increasing edges by 8.93%, 1.70% and 0.56%.
These observations support testing five against a four baseline and retaining
six as an experimental comparison, especially for long observation windows.
They do not identify the fastest cutoff: assembly memory, hierarchy fill,
preconditioner application and iteration counts remain unmeasured. An eventual
general policy could consider incremental merged fill rather than degree alone.

German counts use a column-only PyArrow/NumPy scan, exact deduplication of packed
worker-establishment keys, and exact enumeration/deduplication of firm pairs.
Metadata verify nonmissing IDs and years, IDs within the packing range, and
single-year row groups. Histogram totals and weighted degree sums match workers
and distinct links. Independent DuckDB queries reproduce the BW histogram and
merged pair count. The fresh SCC Veneto scan reproduces the existing six-year
degree histogram and computes the merged counts above. Its one-slot job 8061464
completed on `scc-gd4` with accounting failed=0 and exit_status=0 (18 seconds,
557,932 KiB peak RSS); this analysis duration is not a solver timing. No source data changed,
no row-level data were exported, and no model was fitted. Only aggregate evidence
is retained here; temporary analysis artifacts are deleted after recording hashes.

Source SHA-256 identities:

- German Parquet: `bcada1475fefa468d351422c3399ed8698f15b2c09a54bcc33b0b100d454f7e8`.
- Veneto six-year CSV: `e4a1928d98c1dbe7185634e14470b9361b58f8d5b33cceacc8d168ccdad4634e`.
- Veneto public two-year CSV: `93e57a413a8cfccdcb043c5d793105a67b2dc9ebd27d5d3a4f1800abf89a2241`.

Analysis-script and aggregate-output hashes are recorded in this documentation
commit. Runtime production files and artifact inputs are unchanged; the checks
for this evidence-only addition are aggregate arithmetic, source/method review
and `git diff --check`.

## Public beta packaging and local installation, 2026-10-13

Source `7ecb7974f4fbde099a07f74b3cc7ce02738886e7` changes packaging,
licensing metadata and installation documentation without changing the estimator
or scientific tolerances. The current runtime's preceding scientific evidence
remains applicable; these checks qualify the new package and its installation.

| Check | Result |
| --- | --- |
| Python tooling | 203 tests pass with 13 existing optional skips; the 12 focused packaging tests pass. |
| Dependency/source closure | Rust 1.85.1 and cargo-audit 0.22.1 report zero vulnerabilities for the exact source and lockfile. The source archive contains 53 vendored dependencies, the local Rayon fork, dependency/Rust notices and a relocatable offline Cargo configuration. All 3,077 source inventory entries match their hashes and sizes. |
| Native build | Universal x86-64/arm64 build from the extracted corresponding-source archive succeeds with an empty Cargo home and `CARGO_NET_OFFLINE=true`. Both slice audits and the host's actual-plugin ABI/selftest harness pass. The corrected packaging rebuild reuses the native build cache from the initial empty-cache build. |
| Extracted release fixtures | Stata/MP 19 on Apple Silicon passes `installation_smoke`, `feppml_installation_smoke`, `staged_plugin_binary` and `single_runtime`, including native-file failure rollback and repeated OLS/PPML use. All fixture and process return codes are zero. |
| Actual local installation | A fresh `net install` into isolated PLUS/PERSONAL/SITE/OLDPLACE directories installs all 30 declared files byte-for-byte. Native selftest reports the exact source revision. OLS agrees with `regress` within 1e-8; an absorbed PPML fit and saved-mean prediction pass; a two-group PPML witness agrees with log group means within 1e-9. |

The first actual installation exposed Stata's omission of `.txt`, `.json` and
`.sha256` files declared with lowercase `f`. Commit `7ecb7974` uses uppercase
`F` so standard installation includes licenses, the source link and metadata;
the final checks above include those files. An initial test driver requested
PPML prediction after absorption without saved effects/means and received the
documented `r(498)`; correcting the test to request `mu()` passed without an
estimator change. An earlier tooling launch placed temporary outputs inside
the source tree, which correctly failed the external-output guard; the normal
external-temporary-directory run passed.

Artifact SHA-256 values:

- Binary ZIP: `65f1bd220595316afc7510546e9952c10fce1ab1b3cda90198ab76cc05f96cbc`.
- Corresponding-source ZIP: `8b843a144b6b52db9d0e6f7fc3bb83a9d90e8b7ed0bcd9e9c9f88415c746ec4d`.
- Universal plugin: `16ac21f1184ba12e7ebc47949adce8ae75133ae63bd1afeab052145aa6133aee`.
- Release manifest: `f2d95f71ff7208636690e611954023d6bc73f33f8e381d9117455c63edcb3726`.

Independent review found no remaining packaging blocker after the installation
fix. These are local build/installation results; public HTTP installation has
not yet been tested. GitHub refused the attempted private-repository transfer
to the existing `fe-tools` organization because the authenticated account cannot
create private repositories there. Publication awaits resolution of the intended
GitHub destination. GitHub Actions remain disabled. No Linux, Windows, native
Intel-host Stata qualification or new performance claim follows from these checks.

## Mac OLS: CMG request versus auto on sparse AKM, 2026-10-13

Johannes requested a simple four-worker comparison on 16 million rows. The
frozen current Mac plugin at source `863318969195360ff06b668983fbdfafb63a3550`
ran in Stata/MP 19 on Apple M2 Ultra (24 physical cores, 192 GiB RAM), with
four requested and effective native workers, Stata processors set to four,
`memory(fast)`, `deterministic`, `tolerance(1e-8)`, default iteration budget,
unweighted OLS and unadjusted covariance. The command was
`fereg y x1-x16, absorb(g1 g2) solver(auto|cmg)` with those common options.
No saved FE or prediction outputs were requested.

The input uses the [campaign sparse AKM topology](../bench/campaign.py) and
[maintained outcome/control generator](../bench/generate.py): 3.2 million
workers, five periods each, 160,000 firms, initial firm `worker_index % firms`,
and 2% independent random destination mobility at each transition. The seed
is 739391; NumPy 2.3.5 and pandas 2.2.3 generated 16 double controls and the
OLS outcome without the campaign's unused auxiliary columns. Actual firm
changes number 256,355 (2.0027734375% of transitions). The DTA round trip is
exact. Both options retain all 16 million rows, rank 16, absorbed DoF
3,353,066 and residual DoF 12,646,918, with no singleton removals.

The fixed panel used one full-size warm-up per option and three measured
pairs in alternating order, with a 1,800-second total process limit and no
continuation. It completed in 633.82 seconds. One Stata process held the same
data throughout. The independent native command clock includes frontend,
native loading, estimation, inference and publication; data generation, DTA
loading, result export and checks are outside timing. A separate 10,000-row
preflight passed before the full panel.

| Observation | Auto seconds | CMG request seconds | Order |
| --- | ---: | ---: | --- |
| Warm-up (excluded) | 15.953673458 | 145.567799458 | auto, cmg |
| Pair 1 | 15.745327708 | 140.126737708 | auto, cmg |
| Pair 2 | 15.520258583 | 140.647376667 | cmg, auto |
| Pair 3 | 15.765887875 | 142.607612458 | auto, cmg |
| Median | 15.745327708 | 140.647376667 | |

The median CMG-request time is 8.93264 times auto's, an additional
124.90205 seconds. The paired ratios are 8.89958, 9.06218 and 9.04533. Auto
reports the MAP-owned family (code 2), 115 main iterations and 14 final
residual iterations, without fallback. **Every CMG request reports main
solver code 4 (CG), fallback=true and one warning**, with 117 main iterations;
the final residual is already certified (CMG code 7, zero iterations).
This measures the public CMG request including fallback, not a successful
CMG-only solve. The original diagnostic retained only the warning count;
the identical-input follow-up below identifies the setup rejection. Median
native-only time is
13.92084 seconds for auto and 138.82115 seconds for the CMG request; median
input-loading time is 1.21052 and 1.21755 seconds, respectively.

All eight calls pass the native rank, score, bread, covariance and overall
certificates. Auto/CMG maximum coefficient difference is `2.78961e-13`,
maximum covariance-entry difference `2.17615e-20`, and relative RSS difference
`1.02143e-12`, passing the existing campaign comparison tolerances. Within
each option all four calls have exactly equal coefficients, covariance,
fit scalars and certificate matrices. This is agreement and repeatability
evidence, not a new independent large-design oracle.

No competing builds or tests were launched by this benchmark. Desktop
activity remained, including Spotlight indexing at launch. These are
descriptive observations with three measured pairs, not quiet-host release
qualification or evidence for changing automatic routing. No production
code, artifact inputs or scientific rules changed for this comparison.

SHA-256 identities (raw data, logs and task copies removed after recording):

| Item | SHA-256 |
| --- | --- |
| Frozen plugin | `85dbf448ab1974122a47684ac9b9f17964b451109b267df409963200cdfbe9cb` |
| Numeric DTA input | `c0950ef8151bc8966cc0cb377fb4394b0c269b6cc7f25738836319c20ccb4835` |
| Frontend manifest | `370ced2082667158c5f3700e1c430e8592e1b55806d820b9e8cad109f57c6048` |
| Command-clock plugin | `31dabb8cb6e6081eb268fc48a4a0c12d9b4d64d815504e062c2b1a30cb0077bf` |
| Input-generation wrapper | `c312876aac556fe107839617a765c0eb7dd199da1f0263aa4d01575d29e70054` |
| Stata comparison do-file | `eb6840c16e6f204377868465923cf7246e939daf6d389d549f13085f8fba9533` |
| Bounded process launcher | `9fd3f01bd8c1f48c5892fd34c33655b6e34f7cda1f56f4fc4ecf9e9aed5dbb6f` |
| Result-analysis script | `a8372e26cd0d477a6eae7b21358e2fd5088b9eb6e1da51f70e454efa27ecda50` |
| Full Stata log | `d9a8c4731abedc96a0193c4485c193a4291f1040545e9c96a69f4efadee8a8ae` |
| All run matrices/scalars/times | `e71841a99f85b3f26ec8e0fe3061ea91537fdce7d1767da4f65bf3270216a440` |

The frontend manifest hashes sorted records of each file's SHA-256 and
path relative to the frozen `stata` directory. The retained current source,
generator recipe, individual observations and hashes identify the comparison;
ignored captures are not retained evidence. The documentation-only change
passes local reference checks and `git diff --check`, reusing the existing
plugin validation without rebuilding or rerunning the runtime suite.

### Follow-up: CMG memory preflight rejected setup

A single diagnostic reproduction regenerated the exact DTA hash above and
used source `863318969195360ff06b668983fbdfafb63a3550`, with only two added
trace statements at CMG setup/solve error branches. Solver mathematics,
tolerances and memory policy were unchanged. The temporary ARM64 build was
combined with the original plugin's unchanged Intel slice for the universal
binary audit; audit and licensed Stata native selftest passed. No installed
plugin was replaced. The 600-second bounded reproduction completed in
144.23 seconds including process startup and DTA loading; this is diagnostic
work, not an additional observation in the timing panel.

The captured setup error is:

> allocation: CMG requires 130776528928 incremental bytes and does not fit the joint execution/solver memory plan

The conservative CMG requirement was 121.79513 GiB. The shared usable-memory
budget was 86,094,276,199 bytes (80.18154 GiB), with 9,909,770,714 bytes
(9.22919 GiB) already reserved for execution, leaving 70.95235 GiB for the
solver. `FEREG_MEMORY_LIMIT_BYTES` was unset. These budget values are from
the reproduction; the original panel did not retain its error text. The
shared policy uses current available memory minus 20% headroom (at least
512 MiB), rather than installed RAM. `memory(fast)` does not bypass it.

The [CMG preflight](../crates/fereg-core/src/fe/cmg.rs) uses the conservative
upstream hierarchy-build estimate, 3,360,000 graph vertices, 16 million
input edges and the same 16-million upper bound on distinct edges, plus
workspace for 18 right-hand sides and the parallel runtime. Repeated
worker-firm matches make the distinct-edge upper bound conservative, but
this experiment does not establish how much memory actual CMG would need.
The [joint budget check](../crates/fereg-core/src/memory.rs) rejected setup
before graph construction. This was not a CMG iteration or convergence
failure: the long command time belongs to the subsequent CG fallback.
The reproduction again reports main requested/used codes 7/4, fallback=true,
117 iterations, all 16 million rows and a successful certified result.
The memory policy and automatic routing remain unchanged.

Diagnostic SHA-256 identities, with temporary sources, plugin, data and logs
removed after recording:

| Item | SHA-256 |
| --- | --- |
| Temporary diagnostic plugin | `a86af9c896be33cf49831a999a2a8bfb0d40c0f4e0158dc55dec7f97191027d5` |
| Instrumented CMG source | `ee83e5bae0f21a61425ca20a71b2ef744d7e8042f0b227c7e4bf6177833ab14b` |
| Captured failure/config/profile | `43570076f430a1b8c404f1d5147d6ea14cff3872f5184b2b2024253a8e2086a3` |
| Stata diagnostic do-file | `fb0098b68694a883c87f9c1de484ca102329d57880a218fe8aab2e1a77ac4438` |
| Completed Stata diagnostic log | `3fb13db670c48bb53c71ffb99f809a5388946709610c6789baf5b95ff023a4ad` |

## H9 adopted: science passes, Mac timing inconclusive, 2026-10-13

Johannes authorized adoption of the H9f strict-only design despite its failed
5% discovery-improvement gate. After reviewing the Mac and SCC timings, he
confirmed “OK, let's adopt it” on 2026-10-13, accepting integration with the
Mac timing limitation recorded below. This does not establish quiet Mac timing
qualification or a release-speed claim. The runtime now retains accepted
canonical geometry only for automatic two-FE PPML strict checkpoints within an
existing Fast reservation, with no saved-FE channels. The operator, RHS,
certification and final accepted-mean Hessian remain current; all tolerances,
memory allowances and protected model decisions remain unchanged. Ordinary
frames, explicit solver requests, Low mode and Hessian refresh keep their
existing solves. Independent source review clears the acceptance/rollback,
identity, memory and deterministic per-RHS cold-retry safeguards.

The parent is `2a7328f9f47910c51114ed8f527cc144571902cc`. Fresh universal Mac
baseline/candidate plugins have SHA-256 `cc1d1ba5c8cb7394` / `d0053c508a61984d`;
the [retained evidence](../bench/results/h9-20261013-integration.json) contains
full identities, all structured checks, task harness sources and every timing
observation. The [case-specific numerical scope](../tests/replay/scopes/2026-10-13-h9-strict-macos.json)
was frozen from input identities and baseline presence before candidate replay.

The independent diagnostic consolidation subsequently entered main as
`25015664d506f31e23960dbdf1ae0b74b53ed6b8`. All eight H9 source hashes, its staged
changes apart from the coordinated inventory, and its measured plugins remain
unchanged. The combined complexity inventory and full runtime suite pass
against that new parent: 1,765 Rust test invocations in 64 groups and 207 Python
tests, with the same 13 optional Python skips and Mac GNU ELF audit skip. The
measurements here retain their original artifact identities; they do not
establish timing of the combined diagnostic/H9 artifact.

The combined universal Mac plugin, SHA-256 `85dbf448ab1974122`, passes binary
audit and native ARM64 load/ABI/selftest checks. Its 73-case replay (219 calls)
is scientifically exact against the earlier H9 candidate and across repeats,
with all independent references passing. Six further actual-reuse witness calls
at one and sixteen workers also match that candidate exactly and pass the
known-optimum/dummy-covariance checks. Four Stata/MP 19 fixtures pass for
installation, the single runtime, public PPML scope and hidden PPML transactions.
These composition checks have independent review and separate artifact bindings
in the retained evidence. They do not resolve the Mac timing limitation below.

| Check | Result |
| --- | --- |
| Full runtime suite | Pass: 1,759 Rust test invocations; 207 Python tests (200 in the main suite), 13 existing skips; formatting, Clippy, allocation audits, portable/static, complexity, mock-SPI and numerical guards. Mac skips the GNU ELF audit. |
| Native Mac build | Both universal builds pass binary audit and native ARM64 load/ABI checks. No new Linux or Windows qualification claim. |
| Native replay | All 73 cases pass, including independent references, fixed-worker repeats and scientific/protected-decision checks at 2/4/8/16 workers: 803 calls. |
| Licensed Stata | All 17 PPML science, transaction, allocation and validator fixtures, plus private/public PPML Break rollback checks, pass. |
| Actual reuse | Isolated full-fit Rust witness and 15 native calls at 1/2/4/8/16 workers pass; all ten candidate/repeat fits consume certified accepted geometry. |
| Timing science | All 20 standard baseline/candidate pairs are exact, including binary output columns. All 320 later timing calls reproduce those fingerprints. |
| Mac nonregression | 19/20 cells meet the confidence bound. OLS K=20 at 16 workers is inconclusive; background syncing, indexing and backups resumed. No clean Mac qualification or release-speed claim. |

The native paired-score witness has analytically known coefficients and means,
plus independent explicit-dummy covariance. Maximum coefficient error is
`1.53e-16`, maximum scaled mean error `1.36e-15`; baseline/candidate coefficients
are exact and the largest absolute covariance difference is `8.47e-21`.
Protected sample, missingness, rank/omission, separation, outer/strict, fallback
and acceptance decisions remain exact. Fixed-worker scientific repeats are
exact; cross-worker science and protected decisions agree. Reuse consumes
generation-1 geometry at generation 5 with 27 iterations, 36 operator actions
and 27 inverse actions. The 132,120-byte cache and 232-byte ledger fit within
338,752 bytes of reported workspace and the existing 1,048,576-byte reservation.
Neither standard PPML timing case attempts reuse; the separate witness and
earlier SCC experiment establish actual reuse coverage.

The frozen timing panel uses one warm-up plus seven alternating pairs per cell,
finishing all 320 calls in 650.64 of 2,400 seconds. The inconclusive cell has
baseline/candidate medians 0.392249/0.401017 seconds and paired-median ratio
1.020701. Its 95% bootstrap interval is [1.014167, 1.079446], exceeding the 1.05
upper allowance. Retain every observation; no continuation was run. Host
sampling records renewed FileProvider/Spotlight activity and later backups.
The earlier SCC improvement failure remains unchanged and is not converted
into a performance pass by the adoption instruction.

Setup failures remain recorded: a parent build invoked from the wrong working
directory was discarded before fits; the licensed driver rejected a filename
before Stata ran; a bounded input reader and redundant `separation(none)` wire
helper were corrected, with one baseline request rejected before estimation.
After six valid native witness calls, an overly strict trace reader rejected
only the order of concurrent RHS diagnostics. Independent review confirmed all
fields and science exact. The repaired reader sorts only contiguous RHS blocks,
preserving attempt boundaries and multiplicity; it verifies the six preserved
outputs/events and runs only the nine originally unrun calls. No estimator,
input, tolerance or fit budget changed in that repair.

## H9 strict-only fits: partial performance gain, 2026-10-13

The authorized [strict-only complete-fit test](fable_performance.md#h9f-authorized-strict-only-complete-fit-prototype)
finds useful savings but fails its predeclared discovery gate. Dense 4M total
native time falls by 9.26% at four workers and 3.41% at sixteen workers; both
sixteen-worker pairs miss the required 5% improvement. All scientific,
repeatability, provenance, reuse and nonregression gates pass. This experiment
left production unchanged. Johannes later explicitly authorized the
[strict-only integration](fable_performance.md#h9g-authorized-production-integration)
despite the failed improvement gate; ordinary-frame reuse and A2 remain outside
that authorization.

| Frozen input | Workers | Baseline median s | Candidate median s | Median paired reduction |
| --- | ---: | ---: | ---: | ---: |
| Dense 1M | 1 | 9.890 | 9.464 | 4.31% |
| Dense 1M | 4 | 4.569 | 4.321 | 5.41% |
| Dense 1M | 16 | 3.046 | 3.019 | 0.90% |
| Dense 4M | 1 | 46.387 | 42.522 | 8.33% |
| Dense 4M | 4 | 20.924 | 18.987 | 9.26% |
| Dense 4M | 16 | 13.046 | 12.600 | 3.41% |

These are complete native fits, including producer retention and discarded
work. Each cell has two alternating baseline/candidate pairs. Dense 4M's
four-worker ratios are 0.907086/0.907753 and its sixteen-worker ratios are
0.970733/0.961038. Percentages use the median paired ratio, not the ratio of
arm medians. All twelve cells meet the max(5%, 1 ms) nonregression allowance;
the largest median increase is 2.76% for AKM at sixteen workers. The small
SCC panel is descriptive and does not establish a Mac release speed claim.

The strict phase explains the gain: dense 4M median strict time falls from
5.109 to 3.189 seconds at four workers, and from 2.497 to 2.118 seconds at
sixteen. Corresponding complete command medians fall from 22.525 to 20.589
and 14.651 to 14.223 seconds. Earlier cache-retention costs remain inside
whole-fit timing. A successful strict solve alone would not pass this gate.

All 48 target fits and 48 initialization fits finish, as do the two smoke fits
and their two initializations: 100 native calls, with no fit retries. All 24
paired scientific comparisons, fixed-worker same-arm repeats and cross-worker
checks pass. Published coefficients match exactly; the largest absolute
covariance difference is `8.47e-22`. Sample, rank/omission, separation,
missingness, outer/strict decisions and existing fallback routes remain exact.
AKM and sparse comparisons are exact on the unaffected scope. Native
certificates pass at unchanged tolerances. The benchmark request adds no
saved-FE or fitted-mean export; known-optimum, accepted-Hessian and independent
oracle checks supply separate witnesses for those mathematical obligations.

All twelve dense candidate fits consume accepted generation-1 geometry at the
generation-5 checkpoint. Normalized weight ratios remain 22.07/36.84, with
four/eight PCG iterations per RHS and 68/136 total inverse actions. Maximum
true current relative residual is `3.72e-15`, below the unchanged `1e-14` inner
target. Retained caches are 18.0/72.0 MB, and inclusive current workspace is
42.7/169.3 MB within the existing 160/640 MB reservations (decimal MB). No
memory budget or Fast/Low selection changes. Dense 4M observed estimator
process RSS rises by about 121–123 MB; this observation is distinct from
modeled solver workspace and scheduler virtual-memory accounting. The small
smoke exercises a bounded cold retry and still publishes identical results.

Rust 1.85.1 checks pass on Mac and SCC: 29 canonical tests, the isolated full-fit
reuse witness, 435 PPML tests, 19 session tests and ten independent oracle tests.
The PPML/session groups each retain one pre-existing ignored test. Linux native
build/load/ABI and input/output guards pass. Independent Astra review approves
the repaired prototype and harness, then independently reproduces the final
failed-gate decision. The task-only patch reproduces all eight tested source
hashes exactly; the experiment itself did not integrate it into the runtime.

Build jobs `8051255`/`8051343` have `failed=0`, `exit_status=0`. Smoke wrapper
`8051353` exits 1 after successful fits because the first reader used the
incomplete auxiliary trace file. Complete stderr traces already existed in
`process.log`; hash-bound reanalysis passes unchanged comparisons. A reviewed
reader repair preserves those fits and runs only the original unrun targets.
No numerical rule, solver artifact, timing gate or fit budget changes. There
are 47 passing analyzer tests after the repair. Target job `8051451` runs on
isolated `scc-wc4` with 28 physical cores, `failed=0`, `exit_status=0`, 971
scheduler seconds and `maxvmem=6.771G`. Its panel uses 958.773 of 2,400 allowed
seconds; the longest launcher uses 56.74 of 120 seconds.

The [compact evidence](../bench/results/h9-20261013-strict.json) retains every
record losslessly, all paired observations, hashes, schemas, executable harness
sources, accounting summaries and the failed attempt. Its embedded decoder
reconstructs the complete records and reproduces the frozen analysis. The
[task-only patch](../bench/results/h9-20261013-strict.patch) is against `86eaf5ef`.
At the end of this experiment production identity matched that revision and
`3d06d0466587` across 179 runtime modules and 58 artifact inputs. Production
integration follows Johannes's later explicit instruction, not a pass of the
failed discovery gate. No ordinary extension, resampling or retuning follows.

## H9 stale-preconditioner quality: promising absorption savings, 2026-10-13

Johannes authorized the [bounded quality test](fable_performance.md#h9e-authorized-quality-test-beyond-the-inventory-drift-bound)
after the inventory below. The actual generation-1 preconditioners work on
generation-5 strict systems despite normalized weight ratios of 22.07 and
36.84. All six cells pass, and every measured pair is faster with reuse.
These are canonical absorption observations, not complete-fit or Mac release
performance claims. Production remains identical to `3d06d0466587` across
179 runtime modules and 58 artifact inputs.

| Frozen input | Workers | Cold median s | Reuse median s | Median paired time reduction |
| --- | ---: | ---: | ---: | ---: |
| Dense 1M | 1 | 1.662 | 1.184 | 28.8% |
| Dense 1M | 4 | 0.567 | 0.362 | 34.8% |
| Dense 1M | 16 | 0.269 | 0.213 | 20.7% |
| Dense 4M | 1 | 8.567 | 5.084 | 40.7% |
| Dense 4M | 4 | 3.060 | 1.687 | 45.0% |
| Dense 4M | 16 | 1.090 | 0.844 | 23.2% |

Each cell has one excluded warm-up pair and three alternating measured pairs.
The percentage is one minus the median paired ratio, which can differ from
the ratio of separately reported arm medians. Dense 1M at sixteen workers is
the noisiest cell: its three ratios are 0.823, 0.659 and 0.793. These small
panels are descriptive; no confidence-bound qualification is claimed.

Both captures and replays ran on isolated `scc-zj4` allocations of 28 physical
cores, using the frozen 1/4/16-worker masks. The cold one-worker replay matches
each captured geometry, representation and reconstructed-output signature
exactly. Higher-worker candidates are compared with their own cold arm, and
every repeated same-arm signature is exact at fixed worker count. All 24
candidate comparisons pass: maximum column error is `7.50e-15` relative to
`max(1, cold reconstructed column norm)`, and maximum normalized Gram entry error is
`4.45e-16`. Native and additional original-input certificates pass at the
unchanged `1e-12` request; no recurrence or historical matrix certifies a result.

Dense 1M uses the old direct factor and takes four PCG iterations per RHS.
Dense 4M uses one fixed symmetric Gauss–Seidel inverse action and takes eight,
versus six cold explicit-CG iterations. Across 17 RHSs, the candidates perform
68/136 preconditioner actions and 85/153 current-operator applications,
including one diagnostic true-residual application per RHS. Maximum true
relative residuals are `2.30e-15` and `3.72e-15`, below the unchanged inner
target of `1e-14`. Avoiding assembly therefore outweighs the added solve work
in these observations; a reduction in iteration count is not the explanation.

Timing includes retention copying, current geometry, solving, the native
certificate and complete `write_dense` reconstruction. Extra independent
screening and comparisons are outside arm timings but inside the panel's
220.540-second deadline usage. Capture-only hashing and file output are disabled
in replay. Modeled solver workspace rises from 12.1/48.5 MB to 42.7/169.3 MB,
including retained caches of 18.0/72.0 MB, within the captured 160/640 MB
allowances. These are decimal MB and include small diagnostic report storage.
The replay harness's scheduler `maxvmem=4.317G` includes fixtures and preparation;
it is not an estimator memory claim. Full estimator lifetimes, rollback and
memory-limited fallback still require integration checks.

All eight native estimation calls finish: two smoke fits, two target captures
and four launcher initializations. Smoke coefficient/covariance tables, sample
hashes and all 26 non-timing metadata fields match. Provenance verifies accepted
producer and committed strict consumer, sample/topology/basis identities,
coordinates and hashes. All 48 absorption calls finish without decline,
timeout, retry or retuning. Rust 1.85.1 builds and 26 focused canonical tests
pass on Mac and SCC; 26 provenance-reader witnesses, native Linux ABI/load and
input/output guard checks pass. Independent Astra reviews clear the design and
the final observations.

SCC jobs `8050485`, `8050486` and `8050487` have `failed=0`, `exit_status=0`,
with 785/80/228 scheduler seconds. Initial build `8050478` exits 101 before
compilation because its wrapper used the wrong working directory; held
dependents `8050479`/`8050480` were canceled without execution. The narrow
directory repair changes no solver source or fit budget. All failed evidence
is retained by hash and summary.

The [compact evidence](../bench/results/h9-20261013-quality.json) retains every
pair, RHS work count, scientific observation, capture/artifact hash, frozen
plan and analysis programs. The [task-only patch](../bench/results/h9-20261013-quality.patch)
reproduces the experiment against the baseline. Patch applicability, local
references and `git diff --check` pass. Existing production runtime evidence
is reused; no new full runtime suite or platform qualification is claimed.
Large captures and task build copies are deleted at closure.

This result supports a strict-only complete-fit prototype with a newly frozen
validation and cost budget. It does not supply a new weight-ratio cutoff or
pass the earlier full-fit 5% discovery gate. At sixteen workers the observed
dense 4M absorption saving is about 0.25 seconds; it must not be presented as
a 23% whole-fit gain. Ordinary-frame reuse, A2 and production integration
remain outside this completed diagnostic.

## H9 preconditioner inventory: stopped at drift gate, 2026-10-13

Johannes approved the [H9 design](fable_performance.md#7-h9-reuse-preconditioners-at-exact-current-weights)
and narrow SPEC section 13 exception for historical preconditioners. The first
evidence gate fails: all twelve frozen PPML cells exceed the fixed cache-build
weight-ratio bound of 2. No runtime prototype or subsequent fit discovery was
started. Production identity remains exact against execution baseline
`3d06d0466587001a732efc2e3c4c175c89e4c485` across 179 runtime modules and 58
artifact inputs; scientific tolerances are unchanged.

The actual accepted canonical producer is from generation 1, rather than the
last ordinary IRLS frame. Intermediate ordinary frames update weights without
rebuilding canonical geometry. The table shows the first strict attempt;
ratios are rounded across the 1/4/16-worker observations.

| Frozen PPML input | Producer representation | Producer → strict weight generation | Normalized cache-build ratio |
| --- | --- | --- | ---: |
| AKM 1M | Implicit | 1 → 5 | 14.84236 |
| Dense 1M | Direct Cholesky | 1 → 5 | 22.06557 |
| Dense 4M | Explicit iterative matrix | 1 → 5 | 36.84239 |
| Sparse 1M | Implicit | 1 → 8 | 239.75427 |

All compared source codes, generated columns, components, anchors, free
coordinates, sample/topology generations and basis identities match exactly.
The drift failure alone blocks the dense consumers; implicit producers are
also excluded by this design. Sparse canonical construction declines at
generations 8 and 11, while the enclosing strict fallback frames commit
successfully. The first decline clears simulated optional cache state, leaving
no accepted producer for the second strict canonical attempt. There is no
observed strict rollback. Overall, none of the fifteen target strict canonical
attempts is eligible before memory admission. Cache payload lower bounds are
retained, but complete overlap admission, inverse-action quality and PCG
convergence remain unmeasured.

SCC build job `8050148` and inventory job `8050149` both have `failed=0` and
`exit_status=0` (488 and 239 scheduler seconds). Both Linux plugins pass native
ABI/load and input/output guard checks; all 21 focused canonical tests pass.
Two 1,200-row baseline/diagnostic checks agree exactly in coefficients,
covariance, sample hash and selected scalar metadata. Twelve target fits finish
in 228.416 seconds within the declared 900-second budget and 120-second per-fit
cap. Including two smoke and fourteen launcher initialization fits, all 28
native calls complete without retry. Frozen source, plugins, all seven input/
clock artifacts and capture hashes are verified. The panel used `scc-zj4`,
28 allocated physical cores, with the declared 1/4/16-worker masks.

Diagnostic clocks include snapshot overhead and establish no speed claim.
For context, dense 4M's sixteen-worker strict phase is 2.295 seconds, including
0.492 seconds of reduced assembly and 0.007 seconds of matrix packing; the
snapshot itself takes 0.150 seconds. Total native time is 11.606 seconds.
These single instrumented observations cannot be pooled with the older A3 or
H3 panels or used as clean qualification timings.

Independent Astra review found and repaired analyzer generation, invalidation
and snapshot-integrity gaps before analysis. The repaired reader passes 24
synthetic witnesses and verifies every captured file's manifest membership,
size, SHA-256 and binary layout. The held SCC analysis job `8050157` remained
queued after release and was canceled without execution; the same reviewed
analyzer completed on Mac in 0.764 seconds after collection. Two earlier local
diagnostic compilation setup failures (missing Cargo on PATH, then an incorrect
generation accessor) were repaired before deployment. The smoke launcher's
two deletion-count keys were absent from its metadata schema; a retained
post-capture audit verifies all 26 actual non-timing fields exactly, including
`num_singletons` and `separated`, without rerunning a fit. No failed fit is omitted.
Final independent review confirms the stop decision and the successful sparse
fallback interpretation.

The [compact evidence](../bench/results/h9-20261013-inventory.json) retains every
observation, scientific table, normalized H9 event, comparison, snapshot hash,
artifact identity and analysis program. The [diagnostic patch](../bench/results/h9-20261013-inventory.patch)
reproduces the observation hooks against the frozen baseline; it is not a
production change. Local references, patch applicability and `git diff --check`
pass. Existing baseline runtime evidence is reused under the documentation-only
rule; no new full Mac runtime suite, Mac timing qualification or Windows check
is claimed. Task captures and build copies are removed at closure.

The inventory stops this bounded design. Preconditioner quality at the observed
larger drift, a paid later producer refresh, or another general admission rule
would need a revised proposal and budget. Ordinary-frame reuse and A2 cannot
advance on the failed strict prerequisite. Certificate fusion remains separate:
the canonical certificate is already fused, while general FE projection sums
depend on prior norm scales and some group means require a scaled fallback.
No certificate implementation changed in this inventory.

## Adaptive two-FE OLS: accepted tradeoff and Mac qualification, 2026-10-13

Johannes accepted the adaptive version after reviewing the accuracy and work
tradeoffs. Ordinary automatic two-FE OLS now targets half the requested tolerance
first and attempts one tighter residual correction only after a failed requested
certificate. Original RHS normalization, cumulative PCG/MAP limits and retained
FE reconstruction are preserved. Explicit solvers, PPML and final-residual targets
are unchanged. Independent Astra review found no implementation blocker.

The initial candidate failed the retained H8 default projection witness:
9.843576231904905e-9 maximum absolute error against 5e-9, despite passing the FE
certificate. The first focused run passed 92/93 tests; the first full suite stopped
at that same failure after 1,235 passing core tests. Johannes explicitly accepted
this disclosed tradeoff on October 13. The default witness now permits 1e-8 and
also requires exact agreement with the passing half-request route. The strict
1e-14 request retains its 5e-9 bound (observed error 3.615219235086897e-11); the
separate 5e-11 projection witness and all other scientific bounds are unchanged.

Final validation passes: all 93 focused tests and `./scripts/run_tests.sh`
(1,749 Rust test executions across 64 invocations, tooling, allocation audits,
portable and bridge checks). The unchanged candidate artifact reuses its passing
universal Mac build/load evidence, 128 independent oracle tests, 73-case parent
replay with zero exact or scoped differences, all 292 worker checks and all 17
mapped licensed OLS/PPML fixtures. Linux-only ELF checks are skipped on Mac; no
new Linux or Windows qualification follows. Baseline runtime is `b4b4f6d3`,
with subsequent prose-only parent `dd6329d6`; artifact hashes and individual
observations are in the [compact evidence](../bench/results/h8-adaptive-20261012.json).

All four Manu input files match the retained H8 SHA-256 values. The bounded Mac
panel completes 48 licensed fits: two alternating pairs per input at 1/4/16
workers, robust VCE and 16 controls. All 24 paired numerical/discrete checks,
24 fixed-worker repeat checks and 16 cross-worker comparisons pass. Native changes
below are paired geometric-mean percentages; tracing is enabled and two pairs
are descriptive, not confidence-qualified speedup claims.

| Frozen input | 1 worker | 4 workers | 16 workers | Main/final iterations, H8 → adaptive |
| --- | ---: | ---: | ---: | --- |
| AKM 1M | -6.27% | -8.38% | -6.50% | 18/0 → 14/0 |
| Dense 1M | +0.14% | +3.40% | -1.66% | 1/0 → 1/0 |
| Dense 4M | +0.05% | +3.10% | -0.58% | 1/0 → 1/0 |
| Sparse 1M | -10.86% | -11.12% | -10.43% | 120/0 → 92/11 |

The stable candidate then passes all twenty frozen quiet Mac timing workloads:
one warmup and seven alternating pairs, 320 native calls, 629.483 seconds, no
retry or continuation. Every paired-median bootstrap upper bound lies within
max(5%, 1 ms); the largest ratio upper bound is 1.028886. Every plugin repeats
its scientific results and solver signatures exactly. Cross-plugin protected
matrix fields are exact and coefficient/covariance agreement passes; affected
row-output witnesses come from scoped replay and Manu/F1 checks. Unaffected
no-FE, three-FE and PPML timing results are exactly identical across plugins.
There are no competing builds, tests or agent compute during either panel;
Activity Monitor is temporarily paused with a recovery guard and restored.
This is incremental qualification, not a retrospective pass for the older
whole-H boundary. Workspace-capacity and allocator tests pass; no new row-by-RHS
bank is introduced, and no new observed-RSS claim is made.

The retained F1 inputs still pass ordinary full-dummy QR and protected-decision
checks; their 80-digit strict F witness remains failed. Weak changes from H8's
1.076 times the unchanged bound to 595.049 times (absolute F error about 1.44e-5);
barbell changes from 2.480 to 2.465 times. Weak main/final iterations change
140/20 to 138/137. These diagnostics, disclosed before approval, show why early
acceptance is neither a stronger accuracy certificate nor a universal saving.

## H2 follow-up: certified dense three-FE gains, 2026-10-12

The second reopened H2 candidate passes incremental Mac qualification against
`be357256` and is adopted. Automatic unit-weight, symmetric three-FE OLS can
use a geometry-qualified MAP attempt, accepted only by the unchanged requested
certificate. Exact marginal saturation avoids unnecessary sparse pair scans;
integer moments accumulate once per occupied cell. Complete geometry predicts
a sweep at the current recurrence target. Checking the certificate there saves
the fourth sweep on the measured dense inputs. Initial, configured-cadence and
fourth-sweep checks remain; a finite miss continues and a failed pilot falls
back on untouched inputs within the shared work and memory budget.

Fifteen alternating measured pairs after one warmup per cell, tracing disabled,
complete 192 licensed Stata fits in 1,041.614 seconds. The maintained 10,000-draw
paired-median bootstrap (seed 739391) uses its 95% upper bound against
max(5%, 1 ms). All six cells pass nonregression. Dense 1M native reductions are
13.05%/6.46% at 1/4 workers; dense 4M reductions are 37.38%/32.31%.
Command reductions are 12.21%/5.72% and 35.80%/29.34%, respectively.
Percentages use paired ratios, so they need not equal ratios of marginal medians.

| Frozen input | Workers | Native median seconds | Paired native ratio [95% interval] | Command median seconds | Command reduction |
| --- | ---: | --- | --- | --- | ---: |
| dense-1m | 1 | 1.915437 → 1.667007 | 0.86953 [0.86871, 0.87241] | 2.049060 → 1.799667 | 12.21% |
| dense-1m | 4 | 0.664414 → 0.625105 | 0.93542 [0.92980, 0.94204] | 0.769980 → 0.730213 | 5.72% |
| dense-4m | 1 | 11.224951 → 7.032553 | 0.62620 [0.62381, 0.62903] | 11.737197 → 7.533507 | 35.80% |
| dense-4m | 4 | 3.579673 → 2.425196 | 0.67690 [0.67539, 0.68378] | 3.982470 → 2.816806 | 29.34% |
| sparse-1m | 1 | 6.346304 → 6.345033 | 1.00016 [0.99775, 1.00229] | 6.478861 → 6.477881 | -0.02% |
| sparse-1m | 4 | 1.886580 → 1.891642 | 1.00481 [1.00041, 1.01097] | 1.992559 → 1.998368 | -0.47% |

The twenty standard workloads also pass: two fixed fifteen-pair blocks,
warmup in each, combined once into thirty pairs per cell (1,280 native calls,
2,564.605 seconds within the 3,600-second total and 1,800-second block limits).
All captured science is exact across both plugins and both blocks. The largest
observed standard cost is wide64/four workers: 1.82%, interval [0.46%, 3.28%].
Sparse 1M/four workers costs 0.48%, interval [0.04%, 1.10%]. These are retained
costs inside the allowance. No observations were dropped, fits retried or
panels continued. Frozen plugin/source/input hashes were checked. The Mac ran
without competing development builds, tests or agent tools; ordinary application
activity remained. Retained preflight snapshots are not continuous monitoring.

The numerical scope covers successful automatic three-FE results and solver
iteration diagnostics. Sample, rank, omission, separation and requested tolerance
are unchanged. Explicit solvers, weighted routes, PPML and final residual
projection retain their policies. All large-case pairs pass coefficient and
covariance agreement; each artifact repeats captured science and solver
signatures exactly at fixed workers. Sparse captured science remains exact
across artifacts. Cross-worker science and discrete solver decisions pass at
1/4 on Mac and 1/4/16 on Linux. This does not strengthen the requested certificate
or resolve the separate strong-F question recorded under H8.

`./scripts/run_tests.sh` passes: 1,746 Rust test executions in 64 successful
invocations, plus the required tooling, allocation, FFI and complexity checks.
Fourteen Rust test executions are ignored and thirteen optional Python checks
are skipped because the local Python lacks NumPy/SciPy; these are not claimed
as passes. Universal Mac build, both binary slices, actual ABI/load guards,
73-case replay with zero exact parent differences and all 292 worker checks,
and 26 mapped licensed Stata fixtures pass. New witnesses include independent
integer pair moments, eight pinned F2 geometries, generated designs with every
FE order, dense dummy-QR robust OLS, saved-FE reconstruction, Fast/Low memory,
parallel repeats, Break, allocation, consumed-work accounting and fresh norms
and coefficients after fallback. A finite predicted-checkpoint miss reaches a
fourth-sweep pass with exact residual/coefficient agreement. Independent
high-effort Astra reviews found no blocking defect. Production complexity rises
by 449 core lines and eleven dispatch candidates; the scoped ratchet records why.

Before qualification, a fixed sixteen-fit profile against candidate 1 confirms
four → three main sweeps and native reductions of 13.95%/11.73% at 1M/4M with
four workers. This trace-enabled profile is diagnostic, not pooled with release
timings. An artificial 256-row failed-pilot witness retains the added-check cost:
median 66.26 → 71.21 microseconds per attempt (five alternating pairs of 1,000
attempts in optimized CI). Geometry normally declines that input. Temporary
profiling code was removed before the final full suite and artifact freeze.

The bounded Linux check also completes: setup `8044213` on `scc-tb4` and probe
`8044217` on `scc-zj4`, both failed=0/exit=0. Linux x86-64 binary/ABI/load guards,
thirteen focused core tests and six independent oracle tests pass. The 36-fit,
two-pair-per-cell trace-enabled panel passes science and repeats. Dense paired
native reductions are 13.23%/11.48%/10.64% at 1M and 34.23%/22.82%/20.88% at
4M for 1/4/16 workers. Sparse geometry is 6.17–6.33 ms on this host. However,
sparse at sixteen workers costs 15.08% (individual paired costs 10.01% and
20.38%); its mean native times are 2.502 → 2.888 seconds. That unfavorable
observation remains unresolved: it is neither dismissed as noise nor used to
claim confidence-qualified Linux performance. No new Windows qualification or
fixest comparison follows from this work.

Candidate 1 remains **not adopted**: cheaper geometry alone gave dense 4M
native gains of 25.97%/23.17%, but dense 1M/four workers regressed 6.85%
(interval [1.05489, 1.07918]) and the standard wide129/four-worker cell remained
inconclusive. Its 54-fit SCC panel and independent checks passed. Its source
and observations are retained separately; none are relabeled as candidate 2
evidence. The original H2 29.8–30.9 ms sparse failure against 21 ms also remains
failed. This reopening supports a new incremental Mac gain, not a retrospective
pass for the earlier H-wide inconclusive cell.

Evidence: [candidate 1 record](../bench/results/h2-followup-20261012-first.json),
[its checked source patch](../bench/results/h2-followup-20261012-first.patch), and
[candidate 2 record](../bench/results/h2-followup-20261012-second.json).
Every warmup and measured timing is retained there. Raw captures, frozen source,
plugins, drivers, logs and exports are preserved on SCC, with remote hashes
verified before local cleanup:

- `/projectnb/welfgr/fereg/runs/h2-followup-20261012/mac-first-captures.tar.gz`,
  SHA-256 `9abcff312dc0b15c82c2b5d1a152c326e946564ab956027721c0077af0d041c1`.
- `/projectnb/welfgr/fereg/runs/h2-followup-20261012/second/mac-second-captures.tar.gz`,
  SHA-256 `9e30a10d742a8591a779e93498596db57c8e9639636be9407dfc6e97f3d925c4`.

Baseline Mac plugin:
`a97f0c28ec913a6b26b4f1a2388eb95688a60479f402d1bcb57d4cdcfc80d1ce`.
Adopted Mac plugin:
`545bebd0ad51354e17403f5e764b69aadd65232acd8d900eebbf5739bb1d175b`.
Frozen source archive and manifest hashes, input/driver hashes, all SCC
observations and cross-worker checks are in the compact record. Dataset bytes
remain in their original SCC campaign; standard cases are regenerated from the
pinned specification and hash-verified. No ignored local path is retained evidence.

## H complete workstream Mac boundary, 2026-10-12

The final H6 plugin was frozen after runtime/native/replay/licensed qualification
and compared with the retained pre-H3 universal Mac baseline on the twenty
regenerated, hash-verified standard workloads. The host was coordinated quiet;
no agent tools, builds or tests competed with native timing. Seven alternating
measured pairs followed one excluded warm-up pair per workload: 320 native
calls, completed in 643.298 seconds within the 1,800-second panel
and 90-second process limits. Plugin, harness and input hashes match before
and after. No continuation, retry, tuning or selected resampling occurred.

The locked Mac prevented UI coordination. A bounded process pause stopped
Activity Monitor's sampling without terminating it; its original process
resumed automatically after the panel. Post-restoration Spotlight indexing
settled before launch. Prelaunch checks and pre/post CPU deltas are retained;
the highest surviving background process averaged 5.20% of one core on this
24-core Mac. This is snapshot evidence, not continuous host monitoring.

Nineteen workloads pass nonregression; one remains inconclusive. No cell
meets the declared criterion for a material regression. The whole-H timing
gate does not pass; no overall release-performance claim follows.

| Frozen workload | Baseline ms | Final H ms | Paired ratio [95% interval] | Result |
| --- | ---: | ---: | --- | --- |
| ols-no-fe-k5-t1 | 5.406 | 5.422 | 1.0056 [0.9846, 1.0613] | nonregression |
| ols-no-fe-k5-t4 | 4.948 | 4.847 | 0.9768 [0.9537, 0.9957] | nonregression |
| ols-three-unit-k5-t1 | 597.467 | 592.861 | 0.9898 [0.9859, 0.9937] | nonregression |
| ols-three-unit-k5-t4 | 300.353 | 296.408 | 0.9888 [0.9835, 0.9899] | nonregression |
| ols-two-pw-k5-t1 | 616.578 | 620.980 | 1.0043 [1.0003, 1.0205] | nonregression |
| ols-two-pw-k5-t4 | 345.114 | 344.917 | 1.0011 [0.9916, 1.0076] | nonregression |
| ols-two-unit-k20-t1 | 1080.894 | 1073.602 | 0.9943 [0.9914, 1.0038] | nonregression |
| ols-two-unit-k20-t16 | 401.055 | 384.896 | 0.9636 [0.9578, 0.9735] | nonregression |
| ols-two-unit-k20-t4 | 515.078 | 503.198 | 0.9794 [0.9735, 0.9831] | nonregression |
| ols-two-unit-k20-t8 | 431.148 | 417.789 | 0.9663 [0.9634, 0.9763] | nonregression |
| ols-two-unit-k5-t1 | 453.701 | 441.698 | 0.9774 [0.9692, 0.9805] | nonregression |
| ols-two-unit-k5-t4 | 234.829 | 225.833 | 0.9599 [0.9568, 0.9652] | nonregression |
| ppml-two-unit-k5-t1 | 427.760 | 424.873 | 0.9942 [0.9930, 0.9974] | nonregression |
| ppml-two-unit-k5-t4 | 233.805 | 234.139 | 1.0007 [0.9990, 1.0023] | nonregression |
| ols-wide-k129-t1 | 746.397 | 759.753 | 0.9945 [0.9810, 1.0424] | nonregression |
| ols-wide-k129-t4 | 379.652 | 396.701 | 1.0386 [0.9811, 1.0593] | inconclusive |
| ols-wide-k64-t1 | 292.702 | 295.491 | 1.0112 [0.9982, 1.0247] | nonregression |
| ols-wide-k64-t4 | 162.367 | 164.951 | 1.0068 [1.0001, 1.0443] | nonregression |
| ols-wide-k65-t1 | 305.234 | 311.299 | 1.0207 [1.0096, 1.0495] | nonregression |
| ols-wide-k65-t4 | 170.262 | 171.271 | 1.0112 [1.0087, 1.0238] | nonregression |

The inconclusive cell is 129-column OLS at four workers: baseline/final
medians are 379.652/396.701 ms, with a paired ratio of 1.038577 and 95% interval
[0.981125, 1.059293]. Its upper bound exceeds the 1.05 allowance. This does
not establish a material regression or host noise. The bounded panel is
closed; a new experiment requires a new declaration, not a replacement run.

The unchanged rule uses a 10,000-draw paired-median bootstrap with seed 739391
and its 95% upper bound against max(5%, 1 ms). A lower bound above that allowance
is reported as a material regression; a straddling interval is inconclusive,
never a pass. Every measured and warm-up observation remains in the compact
record. Gains below the allowance are observations rather than failures.

Each plugin repeats science and solver signatures exactly on all twenty timing
workloads. Cross-plugin qualification uses the separate 73-case replay at
2/4/8/16 workers and the predeclared H8 recurrence scope: all 292 worker checks
pass, with exact unaffected fields and discrete decisions. This is distinct
from claiming cross-plugin bitwise identity on the twenty timing workloads.
H1 refuted the locality hypothesis and introduced no runtime change. H2 was
not integrated because sparse geometry exceeded its 21-ms cap. H4 left overlap
unchanged; H5 changes campaign placement only. H3, H8 and preserving H6 slices
are the integrated runtime work.

Baseline plugin: `9ced317e7cb995510995490e76dd492b491ee7a830a7147884ac5b76c73e38c8`.
Final plugin: `a97f0c28ec913a6b26b4f1a2388eb95688a60479f402d1bcb57d4cdcfc80d1ce`.
Evidence source commit: `da1ab207abf19e201b1c9f8ac33ab9a5fe2c61f6`;
frozen runtime source: `53337e65d625edeb8167391aa233d78dd71900f5`.
Production identity remains exact across 177 modules and 58 artifact inputs.
Recalculation from stored observations reproduces all twenty reported medians,
intervals and verdicts. The prior 1,726 Rust test executions, 73-case replay,
292 worker checks and 25 licensed transaction checks remain valid unchanged.
Raw artifacts, inputs, host snapshots and observations remain in
`/projectnb/welfgr/fereg/runs/h-20261013-boundary/boundary-validation.tar.gz`
(1,323,733,201 bytes; SHA-256 `d75bd2be4a9d4bf4c43dc67f8b5204ca123ba18695ffd5a0fd2fb8b2a6d719fe`).
The [compact record](../bench/results/h-20261013-boundary.json) retains the
panel and scoped replay. No Windows performance claim follows.

## H final Mac boundary prerequisites, 2026-10-12

The final H6 universal Mac plugin
`a97f0c28ec913a6b26b4f1a2388eb95688a60479f402d1bcb57d4cdcfc80d1ce`
passes the pre-H3 baseline
`9ced317e7cb995510995490e76dd492b491ee7a830a7147884ac5b76c73e38c8`
comparison on all 73 replay cases and 292 checks at 2/4/8/16 workers. The
pinned H8 affected scope and intentional-change policy are verified. Exact,
intentional, scientific, repeated-run and discrete difference lists are empty.
The result reuses the independently witnessed H8 declaration; it does not
replace the retained weak-graph diagnostic or claim stronger certification.

All twenty standard timing inputs match the frozen manifest. Independent
review checked the launch and reporting tools, 320-call count, artifact
identities, per-plugin repeat rule and unchanged paired-bootstrap allowance.
Before any timing, it found and repaired a missing launch check for scope and
per-case worker identities. The final plan binds those prerequisites and keeps
`quiet_host_confirmed=false` in that historical capture. Activity Monitor was
still using CPU after correctness checks. No timing observation existed at
that stage. The subsequent completed panel is recorded in
[the final result](#h-complete-workstream-mac-boundary-2026-10-12); it leaves
one cell inconclusive and establishes no whole-H timing pass.

The prepared panel remains seven alternating measured pairs plus one excluded
warm-up pair per workload, 90 seconds per process, 1,800 seconds total, with
no retry or continuation. Those exact artifacts were later restored and
hash-checked, and the completed scientific evidence was reused unchanged for
the final panel. The original pending record remains historical evidence.

Raw plugins, frozen inputs, replay captures, host snapshots, reviewed tools and
plan are retained in
`/projectnb/welfgr/fereg/runs/h-20261013-boundary/prerequisites.tar.gz`
(1,423,154,235 bytes; SHA-256
`009d6a83ae59301aa3f674a202b6940e94fb4b16586be7102b7a24b5a470f87c`).
The [compact record](../bench/results/h-20261013-boundary-prerequisites.json)
uses archive-relative identifiers and records that timing was pending at capture.

## H6 finalized FE numeric-column release, 2026-10-12

After final owned-OLS singleton processing, reencoding, rebasing and cluster
certification, redundant FE f64 columns can be released. A sealed source-indexed
bank holds raw vectors or privately proved encoded slots. No numeric values are
reconstructed and no empty slice impersonates a released source. Original
request roles and finalized bindings retain ordinary sources (including omitted
terms), slopes, interactions and clusters. The dependent outcome has its own
owner and remains intact. Raw public samples and PPML retain their contracts.

The optional conversion proves exact canonical raw/code correspondence. Failure
keeps the raw owner and original later diagnostic. Both requested and actual
slot capacity must fit while the full old owner remains live, under a ceiling
that includes metadata, loaded RHS and the full concurrent moment workspace.
The physical result must not exceed original ownership. A separate immutable
original-owner byte scalar preserves execution, output and recovery admissions,
including Fast/Low choice, FE retention and solver budgets. No scientific bits,
solver signatures, tolerance, certificate, sample, rank, omission, separation,
mandatory diagnostic, estimate acceptance or publication behavior changes.

All 37 focused executions pass: eight core release witnesses, two plugin
admission/overlap witnesses, thirteen PPML generation tests, twelve RHS tests
the moment-budget boundary and the finalized owner layout witness. They cover pointer/capacity identity, exact
savings, signed zero, malformed correspondence and diagnostic order,
exact/short/overcapacity conversion admission, singleton reencoding, protected
source roles, outcome ownership, Fast/Low, workers and finite budgets.
Independent high-effort static review approves all 18 transferred file hashes.
It assessed the concurrent finite ceiling statically; packed/unpacked overlap,
historical finite budgets and descriptor limits have separate focused witnesses.

The full suite passes 1,726 Rust executions in 62 invocations and 200 Python
tests with 13 optional dependency skips. Universal Mac native ABI/load/guard
checks, 73 exact plugin replay cases, 292 worker checks and 25 licensed
preparation, precedence, singleton, cluster, FE recovery, weight, memory and
transaction fixtures pass. Linux ELF checks are appropriately skipped on Mac;
H7 will build and load this source on Linux. No new Windows claim is made.

Seven unsuccessful attempts are preserved: empty-array access and existing sample
adapters, a private fit-field fixture, the missed raw-vector topology consumer,
a plugin fixture's core-private matrix method, an incorrect small-sample
refusal expectation, and a raw-versus-finalized struct-size assertion. The final small-sample witness uses actual byte savings
without a row-count fence. Two early checks (wrong working directory and a
shared Cargo fingerprint) do not validate the isolated edits; transferred sources
were explicitly touched before full qualification. A Python 3.9 extraction
failure occurred before writing files and was repaired with the pinned modern
interpreter. No failed observation was discarded or tolerance weakened.

Three exclusive FEs remove approximately 24N bytes of numeric capacity, less
descriptor expansion. Costs are the exact correspondence scans and temporary
slot descriptors; historical admission charges intentionally remain unchanged.
Core adds 371 production lines and three dispatch sites; plugin adds 27
lines. No new file/function size or chunk violations. Whole-fit timing is deferred
to the single final H panel, so no wall-time improvement is asserted here.

Candidate plugin SHA-256:
`a97f0c28ec913a6b26b4f1a2388eb95688a60479f402d1bcb57d4cdcfc80d1ce`;
parent plugin: `d5e240f6da750a6b104571e4fdcb13efb429613117dbd7391d852d7e1a20ae75`.
Raw source, plugins, review, failed attempts and checks remain in
`/projectnb/welfgr/fereg/runs/h-20261013-h6/release-validation.tar.gz`
(45,457,009 bytes, SHA-256
`7ab82792cab15597d7de1fb7f3440e08e108a04f8967330429df7fac4687a680`).
The [compact record](../bench/results/h-20261013-h6-fe-columns.json) binds all
qualification identities.

## H7 small-input fixed-cost check, 2026-10-12

One isolated SCC node compared final H6 source `53337e65d625`
with the retained October 10 `e29164e6` plugin through the Stata campaign
entrypoint, sharing the current frontend, frozen 100k AKM inputs and four
socket-local physical CPUs with matching memory binding. The panel used one
excluded warm-up pair and seven alternating measured pairs per OLS/PPML
model. All 28 measured fits, four warm-up fits and four 1,200-row smoke fits
completed; the existing process initialization adds 36 small fits, for 72
estimator invocations total. The declared 120-second process and 900-second
panel limits were unchanged, with no retry, continuation or tuning.

| Cell | October 10 seconds | Current seconds | Paired ratio [95% interval] | Verdict |
| --- | ---: | ---: | --- | --- |
| OLS command | 0.169399 | 0.167542 | 0.9937 [0.8375, 1.0917] | inconclusive |
| OLS runtime total | 0.151418 | 0.149957 | 0.9990 [0.8194, 1.1022] | inconclusive |
| PPML command | 0.594051 | 0.501664 | 0.8453 [0.8321, 0.8506] | nonregression |
| PPML runtime total | 0.557795 | 0.466115 | 0.8357 [0.8195, 0.8439] | nonregression |

The paired median bootstrap used 10,000 draws, seed 739391 and the declared
95% interval and max(5%, 1 ms) allowance. Individual observations, all phase
clocks, solver signatures and differences remain in the compact record. The
comparison includes all intervening changes, including H8; it cannot isolate
C1/C2/E1 or independently establish the historical host-noise cause. An
inconclusive interval is not a nonregression pass.

The existing independent smoke oracle passes all four fits. All sixteen
paired scientific comparisons, including warm-ups, pass unchanged campaign
tolerances, with exact sample/rank/omission/singleton/separation/DoF metadata.
Both plugins repeat their scientific results and clock-stripped solver
signatures exactly at fixed workers. OLS recurrence iterations increase from
13 to 16 under H8; its diagnostic residuals decrease. PPML solver signatures
are unchanged. These diagnostics do not isolate H8's timing contribution. Native Linux audits, the campaign harness
checks, runtime/input hash checks and output completeness checks pass. The harness suite ran 42 checks with one Rscript/fixest availability skip
in this Stata-only setup. The
draft harness received independent static review; its four-slot smoke CPU
selection and complete runtime hashing were corrected before launch. This is
a Linux diagnostic, not Mac performance or Windows qualification.

The original postprocessor stopped on Stata's missing optional
`time_separation_mu` clock in both PPML arms. Its retained replacement keeps
all seven values per arm explicitly unavailable; primary clocks stay mandatory
and the comparison/bootstrap function is unchanged. Ten focused analysis
checks and independent hash-bound review pass. No captured fit was repeated.
A premature accounting-log read and initial scheduler-accounting delay are
retained as collection failures; both final accounting records are complete.

Setup job `8039240` on `scc-tb4.scc.bu.edu` used four
slots for 181 seconds, maximum virtual memory
3.469G. Panel job `8039241` on
`scc-wc4.scc.bu.edu` used 28 slots for 40
seconds, maximum virtual memory 1.498G. Both have
`failed=0`, `exit_status=0`; complete placement, THP and accounting are retained.
Current plugin SHA-256: `78b9bb70ab8438597f1c9414d367cbbc828e6362ec86e952f7022b3f071fd893`;
October 10 plugin: `57cc9789efd6ecc810d6dd82c55822dac3bd537bd7be32bb7a4bcd82e2a574a1`.
Raw inputs, sources, captures, observations and accounting remain under
`/projectnb/welfgr/fereg/runs/h-20261013-h7`; the
[compact record](../bench/results/h-20261013-h7.json) binds their hashes.
No production code or scientific rule was changed by H7.

## H6 dual-order LSMR norm handoff, 2026-10-12

H6 item 7 shares each weighted residual read between the original fixed
512-row certificate leaves (merged in ascending order) and the uninterrupted
row-order dense OLS scaled norm. The two states remain separate; neither
consumer changes its arithmetic. Bounded stack column blocks use the installed
team policy. Private initial prepared-OLS provenance, Fast mode and the matching
normalized weight scale enable the handoff for retained and automatic LSMR.
Low, PPML, raw frames, final residuals and continuations retain their prior
paths; existing MAP norm handoff is unchanged.

The single-use certificate workspace lends its initially unused RHS scales,
then its dead reference-norm bank. Original RHS scale checks and group-certificate
folds retain their exact order. Only after all original coefficient allocations
and actual-capacity admissions can a passing certificate transfer that bank.
No extra heap bank or mandatory admission precedes the original checks.
Dense-only norm invalidity disables the cache and preserves the caller's old
error path. Failed LSMR certificates discard it before unchanged continuation.
Tolerances, certificates, scientific results, solver signatures, sample, rank,
omission, separation, publication and rollback remain exact.

All 13 focused executions pass. Independent retained recurrences cover widths
1/6/17/65, leaf boundaries, zero/subnormal/extreme values, nonuniform weights,
worker counts and invalid-input precedence. Other witnesses cover private frame
provenance, normalized-scale mismatch, workspace pointer/capacity, original
RHS-scale errors, complete fits, automatic/retained LSMR with and without
coefficients, finite budgets and MAP continuation. Candidate and comparator
are checked at identical budgets: automatic routing can legitimately use MAP
at or below the observed LSMR peak, or after a failed LSMR certificate.
The cache never determines that choice.

Workspace Clippy/formatting and independent high-effort architecture review
pass. The full suite passes 1,714 Rust executions in 62 invocations and
200 Python tests with 13 optional dependency skips. Universal Mac native
ABI/load/guard checks, 73 exact plugin replay cases, 292 worker checks and
twenty licensed preparation, FE, LSMR, weight, memory, saved-output and
transaction fixtures pass. The expected Linux-ELF audit skip remains on Mac;
the Linux diagnostic kernel and one focused workspace smoke pass, but they
do not establish Linux plugin qualification. There is no new Windows check.

Seven unsuccessful fixture/migration attempts remain in raw evidence: direct
call adapters and moved access, a reference-module path, then five overly
specific expectations about rejection or continuation/norm-bank behavior.
They did not cause a production solver, certificate or diagnostic-policy repair.
Collection also corrected a parser that omitted the first timing line's Rust
test prefix and a mismatched review-schema key in the record writer. The
original raw timing log and failed writer are retained; no checks or timings
were repeated to obtain a pass.
Core adds 218 production lines and two dispatch sites; plugin production
and size-violation counts are unchanged. FE numeric-column release remains
separate work.

The single declared diagnostic panel on an isolated SCC node completed 24 cells
and 72 measured pairs, checking both norm banks bitwise after every warm-up and measured pair.
Paired median fused/separate ratios range from 0.7406
(width 1, 8 workers, unit weights) to 1.1910
(width 65, 4 workers, unit weights). All cells and their three-pair ranges remain in the compact
record. Six columns at four workers cost 12.77%/12.13% more for unit/nonuniform
weights; 65 columns cost 19.10%/16.76% more at four workers and 14.98%/15.19%
more at eight. Those costs remain visible in this preserving candidate; there
is no claim that the fused kernel is faster for every shape. Three pairs
quantify observed variability only; they do not establish a release-performance
claim. Sharing a read changes the distribution of parallel work, and both arithmetic states still require their original
updates. Whole-fit nonregression remains the complete H boundary.

SCC setup job `8038695` on `scc-gr4` used four slots for 141 seconds,
maximum virtual memory 3.959G. Panel job `8038696` on `scc-yj3` used
28 slots for 46 seconds, maximum virtual memory 1.424G; the diagnostic test
itself took 31.02 seconds. Both jobs report `failed=0`, `exit_status=0`.
The panel used CPUs `0,2,4,6,8,10,12,14` and memory node/socket 0, with
THP `[always] madvise never`. Its Linux diagnostic binary SHA-256 is
`d6f10e06ca3454bc3017eeba5fc0983c4562dea509fdb9f6ec2c3419c599e3ce`.

Candidate plugin SHA-256:
`d5e240f6da750a6b104571e4fdcb13efb429613117dbd7391d852d7e1a20ae75`;
parent plugin: `e29f2e71b41f95953d7d68b398227e8cb33f9bd6e35097c25d8b9a37acc18a9b`.
Raw source, plugins, review, diagnostic panel and all checks remain in
`/projectnb/welfgr/fereg/runs/h-20261013-h6/norm-validation.tar.gz`
(52,497,401 bytes, SHA-256
`25bc603a79bc82fca42b3e07fff55be136dc598cd0cb898e6c6b9e930239a15d`).
The [compact record](../bench/results/h-20261013-h6-norms.json) binds the
source, diagnostic binary and qualification identities.

## H6 uniform candidate markers, 2026-10-12

H6 item 6b retains all-candidate inputs as a repeated true scalar. A concrete
borrowed logical view carries candidates through numeric classification,
chunked SPI transfer and PPML adjustment loading, with no materialization or
second classifier. All-candidate chunked loading keeps its existing no-mask
path. Explicit marker reservations, read/write ordering, interruption polling
and string encounter order remain intact. Repeated true or false is raw data,
not a receipt: column dimensions, metadata/source-use validation, candidate
weight domains, factor validation and exclusion precedence remain authoritative.
All arithmetic, solver signatures, tolerances, certificates and sample/rank/
omission/separation decisions stay exact.

PPML keeps its existing explicit staged original-axis mask. It swaps the entire
candidate owner and restores the prior repeated variant or explicit pointer/
capacity, together with probability-weight bits, before propagating a returned
failure. No fallible conversion is inserted inside that restoration interval.
Raw owner accounting uses actual capacities, while loading, compaction and PPML
future-mask forecasts remain unchanged. Thus the retained all-candidate input
saves N bytes without undercounting a later explicit replacement. Eliminated
allocations remove their own failure points. Logical all-true and candidate
counts take constant time for repeated values.

All 78 focused/mapped executions pass: seven new witnesses, thirteen PPML
owner tests, twenty-seven PPML numeric tests, fourteen independent numeric
sample cases, sixteen plugin candidate tests and one owned-capsule test.
Coverage includes repeated/explicit true and false inputs, malformed shapes,
excluded invalid values/weights, range/lifetime/capacity semantics, numeric and
string scalar/batched SPI paths, Break on excluded chunks, PPML provenance and
restoration, and retained versus future mask admission. Workspace compilation,
Clippy, formatting and independent high-effort architecture review pass.
The full suite passes 1,707 Rust executions in 62 invocations and 200 Python
tests with 13 optional dependency skips. Universal Mac ABI/load/guard checks,
73 exact replay cases, 292 worker checks and seventeen licensed preparation,
FE, weight, memory, output and transaction fixtures pass. The expected Linux-ELF
skip remains on Mac; there is no new Linux or Windows qualification.

Two initial test-compilation checks found missing fixture adapters (a permutation
collect, then reverse/array-view conversions). The first focused PPML all-false
fixture incorrectly unwrapped the existing insufficient-observations error;
it now compares repeated/explicit errors. These three attempts remain visible;
no production numerical or diagnostic policy was repaired. Core adds 71
production lines and plugin 23; size-violation, dispatch and chunk counts are
unchanged. Norm handoff and FE numeric-column release remain separate H6 work.
Timing remains at the complete H boundary.

Candidate plugin SHA-256:
`e29f2e71b41f95953d7d68b398227e8cb33f9bd6e35097c25d8b9a37acc18a9b`;
parent plugin: `80f89d6ae8057be42e533492d1f1bb06c0ea6e72de88daf93333eb1656a1265b`.
Raw source, plugins, review and checks remain in
`/projectnb/welfgr/fereg/runs/h-20261013-h6/candidate-validation.tar.gz`
(25,494,059 bytes, SHA-256
`d9467040a35b36253d5d075595f01a6509100e107394208dfa3533e033e124af`).
The [compact record](../bench/results/h-20261013-h6-candidate-mask.json) binds
source identities and evidence.

## H6 uniform drop reasons, 2026-10-12

H6 item 5b replaces the complete unweighted OLS sample's allocated Retained
mask with a logical repeated scalar. Classified builders keep their original
reserved mutable vectors and wrap them after classification. Logical reference
access and equality allocate nothing; empty sequences ignore the stored scalar.
Explicit storage moves without copying. Raw repeated values are not receipts:
dimensions, counts, numeric/weight checks and original row/drop consistency
still govern acceptance and diagnostic order. Arithmetic, solver signatures,
tolerances, certificates and sample/rank/omission/separation decisions stay exact.

No-drop compaction returns the implicit owner. Actual deletion materializes
fallibly after replacement value/map/weight/adjustment reservations and before
mutation. The unchanged OLS worst-case forecast includes the full original-axis
reason bank alongside both numeric banks and keep mask. PPML generation and
replacement budgets remain unchanged; production initial/final mutable ledgers
remain explicit. The legacy owner's consuming conversion is test-only; production
from_selection already supplies a Vec. Sealed publication reads logical reasons
and retains its dimensions, residual, receipt and transaction checks.

All 39 focused/mapped executions pass, including five new witnesses: logical
sequence access and empty equality, impossible materialization failure, explicit
pointer/capacity transfer, sixty malformed repeated/explicit raw-mask comparisons,
no-drop/deleting compaction and exact/one-byte-short admission. Mapped complete
transfer, numeric sample, PPML ownership/rebase, allocation, singleton, sealed
owner and publication checks pass. Workspace compilation, Clippy, formatting
and independent high-effort architecture review pass. The full suite passes
1,698 Rust executions in 62 invocations and 200 Python tests with 13 optional
dependency skips. Universal Mac ABI/load/guard checks, 73 exact plugin replay
cases, 292 worker checks and seventeen licensed preparation, FE, weight, memory,
output and transaction fixtures pass. The expected Linux-ELF skip remains on
Mac; no new Linux or Windows qualification follows.

The saving is one N-byte heap mask for complete OLS preparation. Actual deletion
still allocates that mask under the existing admission; classified and mutable
PPML ledgers keep actual-capacity accounting. Eliminated allocations remove
their own failure points. Core adds 82 production lines and one structural
logical-equality condition; plugin, chunk and size-violation counts are unchanged.
An initial lead inventory helper failed to import rust_source before writing;
correcting its Python import path restored the ratchet update. The first
collector missed an interleaved nested-test summary header; matching complete
passed/failed suffixes recovered all 62 successes without a test rerun. All
runtime checks pass without production or numerical repair. Timing remains at the H
boundary; candidate storage, norm handoff and FE-column release are separate.

Candidate plugin SHA-256:
`80f89d6ae8057be42e533492d1f1bb06c0ea6e72de88daf93333eb1656a1265b`;
parent plugin: `0c5495fce859f761d934fe65b24bd7c23bf12b05c41ebca5e017bf1b2ce06482`.
Raw source, plugins, review and checks remain in
`/projectnb/welfgr/fereg/runs/h-20261013-h6/drop-validation.tar.gz`
(25,464,434 bytes, SHA-256
`e9b5701555fdb1f022ae12499c9538132f83e2839d308b5c8554b226403dc704`).
The [compact record](../bench/results/h-20261013-h6-drop-mask.json) binds all
source identities and evidence.

## H6 identity row axis, 2026-10-12

H6 item 5a moves the existing logical identity/selected axis to the data module
and uses it for authoritative sample row maps. Complete owned and borrowed
unweighted preparation creates Identity after existing checks; classification
and singleton compaction retain Selected. OLS publication borrows immutable
sealed PreparedOlsData, retaining residual shape/finiteness, output dimensions,
receipts and transactions without rescanning membership. Public raw samples
still undergo dimensions, values, weights, counts, order/bounds and row/drop
consistency validation. Owned-load-hint provenance is unchanged. All arithmetic,
solver signatures, tolerances, certificates and sample/rank/omission/separation
decisions remain exact.

There are 54 passing focused/mapped executions: fourteen axis-filter tests,
twelve PPML model tests, thirteen numeric-sample oracle tests, fourteen
categorical sample/cluster oracles and one coefficient-shadow witness. The PPML
axis witness occurs in two filters. These cover malformed Identity inputs,
logical equality, empty and oversized maps, selected pointer/capacity transfer,
signed zero, no-drop/deleting compaction, complete/no-FE/clustered/excluded PPML
generations, rebase/pweight rollback and publication failure paths. Workspace
compilation, Clippy and independent high-effort architecture review pass.
The full suite passes 1,692 Rust executions in 62 invocations and 200 Python
tests with 13 optional dependency skips. Universal Mac ABI/load/guard checks,
73 exact plugin replay cases, 292 worker checks and seventeen licensed
preparation/FE/weight/memory/output/transaction fixtures pass. The expected
Mac Linux-ELF skip remains; there is no new Linux or Windows qualification.

Complete OLS identity axes own no heap bank, removing 8N bytes on a 64-bit
host. Selected buffers retain their actual capacities. PPML materializes an
identity fallibly only at its existing explicit-row ownership boundary; Selected
moves intact. Eliminated allocations remove their own failure points; invalid
raw-data ordering and remaining fallibility are preserved. Core production
grows 36 lines and plugin shrinks 27; size, dispatch and chunk counts stay
unchanged. Drop-reason and candidate masks remain explicit in this slice.
Timing remains at the whole-H Mac boundary.

Retained attempts include two incomplete API-migration compile failures, two
PPML fixture metadata errors (unused source, then noncontiguous IDs), a test
slice-size lint and an initial Python without tomllib. Repeated publication
access briefly exceeded the function-size limit; a local sample binding and
removal of a redundant debug membership assertion restored the limit. The first
full suite passed its runtime tests but stopped at stale inventory method
positions after a documentation line moved; refreshed positions and the final
full driver pass are retained. No numerical failure or tolerance change occurred.

Candidate plugin SHA-256:
`0c5495fce859f761d934fe65b24bd7c23bf12b05c41ebca5e017bf1b2ce06482`;
parent plugin: `65ce7a7925bc16f9a7c82e6211a35d92914e148d92196b451ca3a38326c0b6ab`.
Raw source, plugins, review and checks remain in
`/projectnb/welfgr/fereg/runs/h-20261013-h6/axis-validation.tar.gz`
(25,590,918 bytes, SHA-256
`d69729d308a0c5915898a867f742a05cc8886fbfca4846fdeca664cda106516e`).
The [compact record](../bench/results/h-20261013-h6-axis.json) binds source
identities and validation, including the removed axis module.

## H6 membership initialization, 2026-10-12

H6 item 6a initializes the membership observation bank through the existing
safe indexed-collection helper, generalized from f64 zeroes to copied scalar
values. It uses the installed team's scheduling policy and the same fallible
reservation and actual-capacity admission. Scatter destinations and per-level
observation order, numerical kernels, solver signatures, requested tolerances,
certificates and sample/rank/omission/separation decisions remain unchanged.

Ten focused checks pass: f64/usize initialization at empty, single-element and
chunk-boundary lengths outside a pool and at 1/4/16 workers preserves pointer,
capacity and bits; serial/parallel membership, optional reservation failures,
invalid codes, excess capacity and high-cardinality admission remain exact.
The full suite passes 1,687 Rust executions in 62 invocations and 200 Python
tests with 13 optional dependency skips. Universal Mac ABI/load/guard checks,
73 exact plugin replay cases, 292 worker checks and seventeen licensed
preparation/FE/weight/memory/output/transaction fixtures pass. The expected
Mac Linux-ELF audit skip remains; no Linux or Windows claim follows.

This parallelizes the serial fill but does not remove its writes: safe mutable
scatter slices require initialization, so the bank still receives 8N zero
bytes followed by 8N scatter bytes on a 64-bit host. There is no extra
observation bank, unsafe conversion or new allocation. Core production grows
four lines; dispatch and size-violation counts are unchanged. No unsuccessful
repair occurred. Candidate markers and complete-sample drop reasons remain
separate H6 changes. Timing evidence is deferred to the whole-H Mac boundary.

Candidate plugin SHA-256:
`65ce7a7925bc16f9a7c82e6211a35d92914e148d92196b451ca3a38326c0b6ab`;
parent plugin: `fc315ad83d18f6074375dd16b93e9cb0d4318852f3bd3e91b5644dbadeaa54c7`.
Raw source, plugins and checks remain in
`/projectnb/welfgr/fereg/runs/h-20261013-h6/initialization-validation.tar.gz`
(25,442,866 bytes, SHA-256
`d4a3ebdf2c7b6b9269e21e679354408758dae008af70df6be8c9a730fde8f592`).
The [compact record](../bench/results/h-20261013-h6-initialization.json)
retains source identities, validation and limitations.

## H6 owned load hints, 2026-10-12

H6 item 1 moves early OLS categorical columns into a private immutable owner
during overlapped factorization. Core restores ordered matching vacant slots
after request compilation, then runs the established loaded-input, weight,
availability and categorical-domain checks. Only the actual complete unweighted
transfer with matching source identities bypasses the hint's identity and
code/value rescans. Weighted/classified inputs, independent borrowed hints and
PPML retain the existing checked reuse. All arithmetic, signatures, requested
tolerances, certificates and sample/rank/omission/separation decisions remain.

Three focused core tests pass, covering 104 capsule/plain comparisons, missing
values, candidates, weights, clusters, singleton pruning, changed requests,
malformed slots, raw pointer/capacity preservation and positive-weight checked
encoding reuse. Plugin tests verify early/late completeness errors and unchanged
moment-cache and membership-scratch budget decisions. The overlap test exercises
both ownership modes at 2/4/16 workers, including exact-budget success,
one-byte-short refusal before any SPI read, optional factorization refusal,
late SPI interruption/panic and joined cancellation. The full suite passes
1,687 Rust executions in 62 invocations, plus 200 Python tests with 13 optional
dependency skips. Universal Mac ABI/load/guard checks, 73 exact replay cases,
292 worker checks and seventeen licensed preparation/FE/weight/memory/output/
transaction fixtures pass. Independent high-effort architectural review approves.
The Mac Linux-ELF skip remains; no new Linux or Windows qualification is claimed.

The capsule adds 56 descriptor bytes per early source on this host; its raw
buffers are moved without copying and descriptors are freed before
classification. Completed encodings plus descriptors fit the original
preparation envelope, preserving the moment-cache decision. Core production
grows 176 lines and two dispatch candidates; plugin production grows 99 lines
and three candidates. There is no new size violation. The first draft discarded
classified hints and would have added weighted refactorization; checked reuse
was restored. A function-size violation was resolved through column-preparation
and target-ownership methods, and a test-only Clippy failure through lexical
borrow scope. These attempts and initial toolchain-path failures remain recorded.
No timing gain precedes the whole-H Mac panel.

Candidate plugin SHA-256:
`fc315ad83d18f6074375dd16b93e9cb0d4318852f3bd3e91b5644dbadeaa54c7`;
parent plugin: `00a9daac0366fdb9f308406fda618cff96cdf026984a69f9a39d5afa7c33020c`.
Raw source, plugins, captures and checks remain in
`/projectnb/welfgr/fereg/runs/h-20261013-h6/loadhint-validation.tar.gz`
(25,673,546 bytes, SHA-256
`695d41c31f310158c2671d188881c3beaf7c21b3d07eb11dd9d7ddafa88c55b5`).
The [compact record](../bench/results/h-20261013-h6-load-hint.json) preserves
source identities, failed attempts, checks and review limitations.

## H6 checked initial RHS, 2026-10-12

H6 item 4 keeps the immutable `PreparedOlsRhs` owner through initial ordinary
OLS projection. Structural automatic and reusable categorical entrypoints
consume its finite-value proof; raw matrices, generic categorical builders,
PPML, final residuals and other backends retain their checks. Configuration,
dimensions, values and weights keep their existing failure order. Arithmetic,
solver signatures, certificates and sample/rank/omission/separation decisions
are unchanged, with no new heap allocation or RHS lifetime overlap.

Twelve focused RHS tests pass. They include 32 checked/raw comparisons across
zero to three effects, unit/provided weights, fresh/reused RHS construction and
one/four workers, checking exact residuals, norms, FE coefficients and complete
reports after removing clocks. Malformed axes, empty inputs, nonfinite values
and competing weight errors retain their diagnostics. The full suite passes
1,682 Rust executions in 62 invocations, plus 200 Python tests with 13 optional
dependency skips. Universal Mac ABI/load/guard checks, all 73 exact replay
cases, 292 worker checks and seventeen licensed preparation, FE, weight,
memory, saved-output and transaction fixtures pass. Independent high-effort
review finds no blocker. The expected Mac skip of the Linux ELF audit remains;
no new Linux or Windows qualification is claimed.

The first compile missed a raw conversion in the internal graph fallback,
and the added witness initially used the wrong `ThreadTeam::new` return type.
Both were corrected before validation. An inventory-update command used system
Python without `tomllib`; rerunning it with Python 3.13 repaired the tooling
failure. Core production grows from 95,189 to 95,241 lines; dispatch and size
counts are unchanged. The whole-H Mac timing panel remains outstanding.

Candidate plugin SHA-256:
`00a9daac0366fdb9f308406fda618cff96cdf026984a69f9a39d5afa7c33020c`;
parent plugin: `752d759d991524e667f68bc0053fb3b7f458a702c4e34b2efa62147f8ad889e4`.
Raw source, plugins, captures and checks remain in
`/projectnb/welfgr/fereg/runs/h-20261013-h6/finite-validation.tar.gz`
(25,411,290 bytes, SHA-256
`704fc219dafeb4404e30d389f776b1622d856ecc6ef24cbd8707474f5f06209e`).
The [compact record](../bench/results/h-20261013-h6-finite-inputs.json) retains
source identities, passing checks, failed attempts and review limitations.

## H6 membership validation reuse, 2026-10-12

H6 item 3 seals the exact codes/membership buffers checked during single-source
transfer and reuses the immutable topology owner in ordinary and categorical
absorbed DoF. A foreign owner is rejected before validation is bypassed; raw
adapters and metadata, cluster and slope checks retain their checks. Rebuilt
topology is validated at the original finish argument after source retention
and rebase, preserving its scratch lifetime and failure ordering.

All 87 focused topology tests pass, including checked/owned DoF equality for
multiple effects, interactions, duplicate recipes, nesting and slopes;
foreign-owner rejection; transfer order, pointer and alias checks; malformed
axes and reservation failures; and weighted/perturbed/NaN recovery. The final
full suite passes 1,680 Rust executions in 62 invocations, plus 200 Python
tests with 13 optional-dependency skips. Universal Mac ABI/load/guard checks,
73 exact replay cases, 292 worker checks and seventeen licensed FE, DoF,
singleton, cluster, slope, saved-output, memory and transaction fixtures pass.
Scientific results and solver signatures remain exact. No new Linux or Windows
qualification follows; the Mac skip of the Linux ELF audit remains explicit.

Independent review caught an earlier scratch-lifetime regression: rebuilt
interaction validation ran before candidate buffers were released. The final
transferred/rebuilt ownership state restores the original point. The earlier
full numerical run passed but failed the new file-size ratchet; placing the
construction state in the existing preparation-topology module and moving a
test-only import resolved the two tooling failures. All attempts remain in
the raw evidence. Core production increases from 95,165 to 95,189 lines;
dispatch and size-violation counts do not rise. No timing gain is claimed
before the whole-H twenty-workload quiet Mac panel.

Candidate plugin SHA-256:
`752d759d991524e667f68bc0053fb3b7f458a702c4e34b2efa62147f8ad889e4`;
parent plugin: `6e2fb08f91c862c5e9c45b6a3df652335cc8521ca689663a7279cca6860db55b`.
Raw source, plugins, captures and checks remain in
`/projectnb/welfgr/fereg/runs/h-20261013-h6/membership-validation.tar.gz`
(25,448,553 bytes, SHA-256
`309940f93ec426e7b816b93958902102470bc081319a9a8fd8763d803aef7e43`).
The [compact record](../bench/results/h-20261013-h6-membership.json) preserves
the source/case identities, failures and final results.

## H6 sample scans, 2026-10-12

H6 item 2 uses the installed worker team's row-chunk policy for complete-sample
validation and numeric-source finiteness, including narrow models. The complete
unweighted identity shortcut proves the old finite-value, retained-flag and row
map conditions together; a failed shortcut retains the old diagnostic order.
There is no numerical reduction or scientific-policy change.

Three focused tests cover malformed samples, boundary row identities, competing
nonfinite/axis errors, weighted and selected fallback, and signed zero at
1/4/8 workers. The full suite passes 1,680 Rust executions in 62 invocations;
200 Python tests have 13 optional-dependency skips. Universal Mac build,
ABI/load/guard checks, 73 exact plugin replay cases and 292 worker checks pass.
Solver signatures are unchanged. Ten licensed preparation, factor-availability,
weight, candidate-sample, rebase and OLS/PPML transaction fixtures pass, with
application return codes and terminal sentinels. Independent high-effort review
finds no blocker. The expected Mac skip of the Linux ELF audit remains explicit;
this item makes no new Linux or Windows qualification claim.

The candidate plugin SHA-256 is
`6e2fb08f91c862c5e9c45b6a3df652335cc8521ca689663a7279cca6860db55b`,
compared with H8's `15824d5b0ce070d56312b9ca7af31428a8086a1064cc655f14e572b90c15f139`.
Raw captures, both plugins, cases and source patch remain in
`/projectnb/welfgr/fereg/runs/h-20261013-h6/sample-validation.tar.gz`
(25,389,514 bytes, SHA-256
`ff74cb830b83c22168d54519e5ee165fa906188d5cc7154639b7ea9611293287`).
The [compact record](../bench/results/h-20261013-h6-sample-scans.json) retains
case identities and all check outcomes. An initial zero-test filter and two
test-compilation corrections are retained, not counted as passing evidence.

Core production grows 32 lines (95,133 to 95,165) and loses one dispatch
condition; the ratchet records the complete-sample proof and diagnostic-order
reason. A failed shortcut pays for its probe before ordinary validation.
The twenty-workload quiet Mac performance panel remains at the complete H
boundary; no wall-time gain is asserted here.

## H5 socket and NUMA placement, 2026-10-12

Campaign tooling now groups allowed physical cores by socket, starting with
the largest allowed socket so partial affinity restrictions cannot force a
small fit across sockets unnecessarily. SMT siblings are deduplicated.
`numactl` applies the same physical mask and memory-node binding to native and
reference arms; missing topology or binding failure has no unbound fallback.
Every raw observation contains the mask, socket/node IDs, `lscpu` and THP.
Runtime tokens and all 58 artifact inputs remain identical to `67cb34ce`.

The fixed 48-fit panel uses frozen AKM 1M inputs, OLS/iid and PPML/robust,
three alternating pairs at 1/4/8/16 workers, current accepted H8 runtime and
fixest 0.14.2. All 24 comparisons are scientifically eligible; all sixteen
three-repeat groups have exact captured science, and both tools pass the
cross-worker comparisons at unchanged campaign tolerances. The table gives
median command seconds, with speedups relative to each tool's one-worker cell.

| Model | Workers | fereg s | fereg speedup | fixest s | fixest speedup |
| --- | ---: | ---: | ---: | ---: | ---: |
| OLS | 1 | 2.291639 | 1.000 | 4.096 | 1.000 |
| OLS | 4 | 1.100400 | 2.083 | 2.115 | 1.937 |
| OLS | 8 | 0.803931 | 2.851 | 1.688 | 2.427 |
| OLS | 16 | 0.750220 | 3.055 | 1.483 | 2.762 |
| PPML | 1 | 10.851459 | 1.000 | 19.397 | 1.000 |
| PPML | 4 | 5.283432 | 2.054 | 8.147 | 2.381 |
| PPML | 8 | 4.248238 | 2.554 | 5.945 | 3.263 |
| PPML | 16 | 4.445408 | 2.441 | 4.944 | 3.923 |

Native PPML is 4.64% slower at sixteen than eight workers despite placement
control. This cost is retained; three pairs on one SCC node do not establish
a universal gain or a controlled old/new placement effect. No extra fits or
timing retries were used.

Four pre-panel 1,200-row single-FE native/reference smoke fits pass the existing
independent OLS/Poisson oracle, including covariance, predictions, sample and
PPML rank. The actual child confirms the requested affinity and `bind` policy
in `/proc/self/numa_maps`. Four placement tests cover interleaved sockets,
partial masks, SMT siblings, missing topology and identical arm commands;
all 42 SCC campaign harness tests pass. Mac `run_tests.sh` passes (1,679 Rust
executions in 62 invocations, 200 Python tests with 13 optional-dependency
skips, portable checks and native guards). The expected Mac skip of the Linux
mock-plugin audit remains explicit; the unchanged H4 Linux plugin's successful
build/audit/load evidence is reused. Five analysis checks include signed zero,
one-ulp exactness, tolerance rejection and exact sample counts. The reused
130-file comparator/clock environment matches its original frozen manifest.

Job `8035870` on `scc-tj3` passes failed=0/exit=0, 479 seconds, 3.886G maxvmem.
Its two fourteen-core Xeon E5-2680 v4 sockets use THP `[always] madvise never`;
the measured panel consumes 452.634 of 1,800 seconds.
Raw source, scripts, plugins by retained H4 identity, datasets by frozen links,
captures, checks and accounting remain under
`/projectnb/welfgr/fereg/runs/h-20261013-h5`; the
[compact record](../bench/results/h-20261013-h5.json) retains every fit and full
hashes. The full H boundary still requires the twenty quiet Mac workloads.
No new Windows qualification is claimed.

## H4 overlap check: no load inflation, 2026-10-12

Five alternating overlap-on/off pairs on frozen 1M AKM OLS at 1/4/16 workers
complete all thirty planned fits on `scc-wf2`, with identical physical masks
within each pair and tracing disabled. The diagnostic off artifact changes
only `cancellable::while_loading` to load before running the same preparation
closure on the same team; production remains unchanged. All fifteen paired
science comparisons and six five-repeat groups are exact, including solver
signatures except clocks. No fit was retried.

RRUNTIME rows 18–21 below are mean seconds (on / off); command time is also
retained in the [compact record](../bench/results/h-20261013-h4.json).

| Workers | 18: preflight | 19: input | 20: native | 21: total |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 0.000359 / 0.000358 | 0.305581 / 0.305297 | 2.257310 / 2.257538 | 2.563250 / 2.563193 |
| 4 | 0.000360 / 0.000350 | 0.299695 / 0.311857 | 0.906805 / 0.900010 | 1.206859 / 1.212218 |
| 16 | 0.000354 / 0.000350 | 0.273927 / 0.287376 | 0.464803 / 0.463340 | 0.739083 / 0.751066 |

The declared primary four-worker load-inflation rule fails: all five paired
on-minus-off differences are negative, with median −15.219 ms and ratio
0.95107. Sixteen-worker load also favors overlap (median −13.482 ms), while
one worker gives −0.049 ms and mixed signs. Mean command times on/off are
2.672148/2.671837, 1.275795/1.279567 and 0.805579/0.815720 seconds.
Small native costs at four/sixteen workers remain visible. The earlier C1
load pattern is not reproduced; this bounded check attributes it to host
variability and stops without a runtime change. These five-pair SCC
observations do not establish a confidence-qualified release performance claim.

Source comparison covers 760 files and finds only the intended diagnostic
coordinator difference. Both Linux builds, native ABI/load/guard checks and
the existing prepared-input numeric test pass, including worker counts
2/4/16, optional allocation fallback, interrupt and SPI panic handling. The
analysis is checked against synthetic inflation, mixed timings and a one-ulp
scientific mismatch. Scheduler setup `8035773` passes (failed=0/exit=0,
396 s, 3.469G); panel `8035774` passes (117 s, 2.622G). The timed panel uses
112.251 of 900 seconds. Raw archives, plugins, patch, scripts and captures
remain under `/projectnb/welfgr/fereg/runs/h-20261013-h4`.

Production identity equals `42c8248c` (171 modules and 58 artifact inputs).
This prose/evidence closure reuses valid H8 Mac runtime evidence; the twenty
quiet Mac workloads remain due once at the full H boundary. No new Windows
qualification is claimed.

## H2 dense three-FE prototype: sparse-cost gate failed, 2026-10-12

The approved prototype completed its fixed 36-fit panel and all scientific
checks, but **was not adopted**: sparse geometry took 29.785–30.855 ms,
exceeding the declared 21 ms limit in all six candidate captures. The production
runtime remains `a05ae5d7`. No cell was retried and no threshold was relaxed.
The [complete prototype patch](../bench/results/h-20261013-h2.patch) applies to
that parent and reproduces all eleven saved source/test/inventory files exactly;
the [compact record](../bench/results/h-20261013-h2.json) retains the outcomes.
This commit is an unchanged artifact with retained experimental source.

The rule was frozen in Workstream H section 6 before implementation. Unit-weight
ordinary three-intercept OLS caches exact-integer centred pair Frobenius bounds
once per session, predicts at most four symmetric sweeps at the H8 recurrence
target, and accepts only through the unchanged requested-tolerance certificate.
A missed pilot releases its candidate before LSMR uses the untouched original;
actual pilot sweeps debit the existing first-half allowance. The final MAP
reserve, explicit/weighted requests, PPML and final-residual route remain intact.
Candidate, histogram, norm workspace and preallocated canonical coefficients
are admitted; caller norm kernels are unchanged. Numerical and route scope is
recorded in section 6; sample, rank, omission and separation remain exact.

Two alternating pairs per frozen input/worker cell give these native means and
paired geometric-mean changes. Both arms use the same physical-core mask.

| Input | Workers | Parent native s | Prototype native s | Paired change |
| --- | ---: | ---: | ---: | ---: |
| dense-1m | 1 | 3.474 | 3.415 | -1.70% |
| dense-1m | 4 | 1.502 | 1.449 | -3.52% |
| dense-1m | 16 | 0.838 | 0.864 | +3.05% |
| dense-4m | 1 | 22.806 | 16.315 | -28.46% |
| dense-4m | 4 | 7.407 | 5.981 | -19.26% |
| dense-4m | 16 | 4.372 | 3.712 | -15.07% |
| sparse-1m | 1 | 13.458 | 13.449 | -0.07% |
| sparse-1m | 4 | 4.742 | 4.587 | -3.20% |
| sparse-1m | 16 | 2.766 | 2.799 | +1.31% |

All eighteen paired science/discrete checks pass, as do all eighteen fixed-worker
repeats. Maximum coefficient/VCE differences are 3.86e-14/1.84e-20. Sparse remains
on LSMR with exact captured coefficients, covariance, statistics and solver
signatures, apart from clocks. Main/final iterations are 4/1 → 4/1 at dense 1M,
10/1 → 4/1 at dense 4M, and unchanged 22/2 at sparse 1M. All dense candidates
predict three sweeps and certify at the four-sweep checkpoint; all final solves
remain LSMR. Geometry costs 37.7–40.1 ms at dense 1M and 169.2–182.1 ms at 4M.
The 1M/sixteen-worker native cost rises 3.05%; all observations are retained.

At 4M/four workers command time is 8.684 → 7.244 seconds. The prototype still
exceeds the retained fixest 4.224-second observation by 3.020 seconds (71.5%);
closing that numerical gap would require another 41.7% of its command time.
Those fixest bytes were measured on another host/source, so this is a remaining
scale comparison, not a new paired fixest claim. Two pairs with tracing are
descriptive SCC evidence; the twenty quiet Mac workloads remain at the H boundary.

Mac prototype validation passes: full `scripts/run_tests.sh` (1,691 Rust test
executions in 63 invocations, tooling/oracle/transaction checks), universal
build and actual-plugin audit/load, all 73 replay cases with zero exact parent
differences, all 292 checks at 2/4/8/16 workers, and fourteen licensed fixtures.
The added licensed eligible-MAP fixture checks saved effects, reconstruction,
predictions and robust covariance against full dummy regression in Fast/Low at
1/4 workers. Five independent oracle tests cover nonuniform and perturbed
products, extra intersections, disconnected designs, weights and explicit LSMR.
Seven core tests cover rounding, acceptance at the request, allocation refusal,
coefficient reconstruction and shared work. A subsequent focused test reproduces
all four bounds of all eight retained F2 rational-projection fixtures bitwise;
its Clippy check passes. No production tokens changed after the candidate freeze.

Independent Astra review identified and resolved an inappropriate norm handoff,
a missing fallback diagnostic, post-certificate coefficient allocation, and
geometry error/Break handling. Initial compile/test-name/import mistakes and
Clippy's decimal-precision warning are retained; exact binary64 bit constants
preserve the frozen F2 witness. A local Python cache-path refusal was resolved by
syntax-only parsing. There were no scientific fit failures or retries. These
prototype tests are retained in the patch rather than installed without their
runtime implementation. Current runtime/artifact identity equals the parent,
so its valid H8 runtime evidence is reused for this documentation-only closure.

SCC setup `8035579` on `scc-gd4` passes with failed=0/exit=0, 364 seconds and
3.967G maxvmem; its seven focused Linux tests and native guards pass. Panel
`8035584` on `scc-wf2`, 28 physical cores, passes with failed=0/exit=0, 423 seconds
and 6.209G maxvmem. Its timed panel uses 406.846 of 2,400 seconds. Source archives,
plugins, scripts, raw captures and both Mac validation archives remain under
`/projectnb/welfgr/fereg/runs/h-20261013-h2`. The final Mac archive is 25,396,960
bytes, SHA-256 `3d5dbd3a0096eef90a7b641bcbe82427907af2740d95470beba51825df8fd64a`.
The prototype would add 388 production lines and eight dispatch candidates;
this closure adds none. No new Windows qualification is claimed.

## H8 recurrence consistency with measured sparse cost, 2026-10-12

The approved ordinary automatic two-FE OLS change uses the existing automatic
LSMR target: a hundredth of the request, floored at 1e-14 and capped at the
request. Both PCG readiness checks receive that target directly. Explicit
requests, PPML's scheduled projection, the final-residual target, certificate
at the request, iteration budget, MAP reserve and acceptance/fallback rules
are unchanged. A passing certificate still publishes `Certified` after proxy
budget exhaustion; trace-only RHS reasons report the recurrence stop.

The result-changing scope names projected columns, FE coefficients, coefficients,
covariance, fit statistics, downstream saved numerical outputs, numerical
diagnostics and inner work counts. Sample, rank, omission, separation,
acceptance/stop codes, fallback and warnings remain exact. See the
[predeclared input-bound Mac scope](../tests/replay/scopes/2026-10-12-h8-two-fe-recurrence-macos.json)
and [compact evidence](../bench/results/h-20261013-h8.json).

All 48 SCC fits completed without retries: two alternating pairs for each
frozen AKM 1M, dense 1M/4M and sparse 1M cell at 1/4/16 workers. The table
shows native mean seconds and the paired geometric-mean percentage change.
All 24 coefficient/VCE and discrete comparisons pass; all 24 within-arm
fixed-worker repeats are exact after removing clocks. Maximum coefficient
and covariance changes are 3.20e-14 and 3.71e-16. Both arms use the same mask.

| Input | Workers | Parent native s | H8 native s | Paired change |
| --- | ---: | ---: | ---: | ---: |
| akm-1m | 1 | 2.060 | 2.177 | +5.71% |
| akm-1m | 4 | 0.973 | 1.063 | +9.26% |
| akm-1m | 16 | 0.523 | 0.547 | +4.68% |
| dense-1m | 1 | 1.439 | 1.417 | -1.48% |
| dense-1m | 4 | 0.597 | 0.615 | +2.88% |
| dense-1m | 16 | 0.355 | 0.373 | +5.06% |
| dense-4m | 1 | 6.548 | 6.470 | -1.20% |
| dense-4m | 4 | 2.884 | 2.564 | -11.08% |
| dense-4m | 16 | 1.463 | 1.529 | +4.54% |
| sparse-1m | 1 | 4.603 | 5.216 | +13.31% |
| sparse-1m | 4 | 2.025 | 2.272 | +12.17% |
| sparse-1m | 16 | 0.890 | 0.986 | +10.95% |

Sparse exceeds the ten-percent reporting threshold at all three worker counts;
these costs were reported to Johannes before integration. The approved rule
is retained unchanged. Main/residual iterations are 14/0 → 18/0 for AKM,
1/0 → 1/0 for both dense sizes, and 92/11 → 120/0 for sparse, identically
in both pairs and at every worker count. A zero final projection reflects the
unchanged initial certificate accepting the more accurate main result. These
two-pair, trace-enabled observations are descriptive, not confidence-qualified
release claims; the twenty frozen quiet Mac workloads remain at the H boundary.

The four retained F1 captures reproduce the exact barbell/weak request hashes.
Full-dummy QR coefficient, covariance, fitted/residual, within-TSS and null-LL
checks pass, and protected decisions are exact. The 80-digit Wald and
mixed-covariance algebra uses exact binary64 inputs. The strong-F bound remains
rtol=1e-12/atol=1e-14 and still fails: barbell is 2.465 → 2.480 times its bound;
weak is 595.049 → 1.076. Weak F error relative to the QR witness falls from
1.440e-5 to -2.603e-8. No stronger certificate or repair of this historical
failure is claimed. Four captures use 1.525 seconds of the 600-second native
budget, without retries; two unused captures are not spent.

Validation: `scripts/run_tests.sh` passes (1,679 Rust test executions across
62 invocations, tooling/oracle/audit checks). Universal Mac and both Linux
native builds pass their audits, loader and boundary guards. All 73 actual
plugin replay cases pass with **zero exact differences from the parent** on
this small corpus, including the three declared cases; all 292 checks at
2/4/8/16 workers pass science, repeat and discrete checks. Fourteen licensed
OLS fixtures pass, including transactions, saved outputs, memory modes,
weights, clusters and independent CMG controls. The focused Linux suite passes
74 tests. A derived three-vertex Schur fixture proves acceptance at the request
after one exhausted recurrence step, with no available MAP step. Exact-boundary,
helper-floor and PPML/final-residual comparisons pass. Independent Astra source
review found no production blocker. The complexity ratchet records 62 additional
production lines and no added dispatch candidates.

Failures remain recorded: a new boundary-test fixture rounded above its nominal
threshold and was replaced by exact power-of-two scaling; categorical test calls
were migrated after the API split; the task-only SCC driver had an indentation
error caught before transfer; and the licensed runner rejected a noncanonical
plugin basename before launching Stata. The corrected basename has identical
plugin bytes. The initial trace summary counted stderr and trace-file copies
twice; their event sequences were checked equal and the final summary counts
each once. Raw captures and the initial summary remain intact. No scientific
fit was retried.

Linux setup job `8035308` on `scc-gr4` has failed=0/exit=0, 406 seconds and
4.625G maxvmem. Panel job `8035310` on `scc-yk4`, 28 physical cores, has
failed=0/exit=0, 321 seconds and 4.153G maxvmem; the timed panel uses 299.584
seconds of its 2,400-second cap. Raw inputs, source archives, plugins, scripts,
logs, observations and Mac validation archive remain under
`/projectnb/welfgr/fereg/runs/h-20261013-h8`; their identities are in the compact
record. Mac plugin parent/candidate hashes are `a9818592...` / `15824d5b...`.
No new Windows qualification claim is made.

## H3 ordered parallel assembly, 2026-10-12

The canonical two-FE Schur builder now partitions packed matrix rows across
workers while preserving every cell's summation order. Each pair is evaluated
twice; each lane repeats the neighbour totals and sorting. Optional scratch
uses spare bytes inside the existing admitted phase peak and is released
before factorization. One worker or unavailable scratch uses the original
serial assembly. Solver selection, certificates, tolerances, sample, rank,
omission, separation and published stop decisions are unchanged.

Before timing, the retained A3 geometries passed raw bitwise matrix comparisons
at 1/2/4/16 workers, including restricted scratch, interleaved disconnected
components, singleton blocks and extreme weight scales. Both frozen dense
PPML inputs then passed all four current-weight matrix comparisons at sixteen
workers. Their dataset hashes equal the retained A3 hashes. The 24-fit panel
used two alternating pairs per cell, the same affinity mask for both arms,
120 seconds per process and a 2,400-second total cap, without retries.

| Rows | Workers | Baseline native s | Candidate native s | Paired reduction |
| --- | ---: | ---: | ---: | ---: |
| 1M | 1 | 9.176909 | 9.185863 | −0.10% |
| 1M | 4 | 5.303372 | 4.900105 | 7.62% |
| 1M | 16 | 3.991165 | 3.075145 | 22.95% |
| 4M | 1 | 40.915626 | 40.580177 | 0.82% |
| 4M | 4 | 24.615295 | 19.401213 | 21.18% |
| 4M | 16 | 19.540483 | 11.635927 | 40.45% |

Times are arithmetic means; reductions use geometric means of paired ratios.
All twelve pairs have exact captured coefficients, covariance, sample hashes,
scientific metadata and timing-stripped solver traces. Repeated scans do not
outweigh the observed gain at four workers on these inputs. The one-worker
changes are below the workstream allowance and are observations, not gains.
Maximum sampled process-tree RSS is 4,712.77/4,715.99 MiB for baseline/candidate.
The older A3 current/implicit native times at sixteen workers were
3.8060/2.7205 seconds at 1M and 18.4272/10.8961 at 4M; at one worker they were
9.3315/8.3492 and 39.0529/31.6992. Different hosts, sources and native harnesses
prevent pooling those observations with this Stata-side panel. A3 cost-model
calibration remains stopped; no implicit-route change was made.

Mac checks pass: `run_tests.sh` (1,678 Rust test executions and the Python,
oracle, allocation, ABI and tooling checks), universal build/load audits,
73 exact baseline replay cases, 292 additional thread/repeat checks and all
17 mapped licensed PPML scientific/transaction fixtures. Linux builds and
native audits pass; its 21 canonical-product tests pass. Independent Astra
review found no production correctness blocker. The complexity ratchet records
220 additional production lines and three resource-related dispatch candidates.
`--compare-production ac8aa81f` identifies the two changed runtime modules;
this is arithmetic-preserving work, not an unchanged artifact.

Build job `8034161` passed (520 seconds, 3.875G maxvmem); panel `8034163` passed
on `scc-zj4` (574 seconds, 6.631G), both failed=0/exit=0. The timed panel took
531.72 seconds. The first Mac full-suite invocation stopped on a new test's
Clippy iterator lint; the corrected full suite passed. The first analysis
mistakenly included clocks among scientific fields; removing only `time_*`
and trace `seconds` clocks gives the exact comparisons above. Both attempts
remain retained, as does the first compact-record size check. No fit was rerun.

The [compact record](../bench/results/h-20261013-h3.json) binds source, plugins,
inputs, observations and validation captures. Raw material remains under
`/projectnb/welfgr/fereg/runs/h-20261013-h3/`. These two-pair, trace-enabled SCC
observations are not a release performance claim. Quiet nonregression timing
on the twenty frozen Mac workloads remains due once at the H workstream
boundary; no new Windows qualification is claimed.

## H1 locality-sort confirmation: hypothesis refuted, 2026-10-12

The ten-fit diagnostic panel on `scc-zj4` rejects H1 under its predeclared
20% projection-time gate. Sorting rows is a sample-order permutation:
coefficients and covariance agree within the unchanged SPEC tolerances;
sample, rank, omission, DoF, singleton accounting, published solver decisions
and indexed per-RHS convergence/iteration decisions are identical. All five
cross-worker comparisons also pass. No H1 production change or within pin
bump was made; the next item is H3.

| Input | Workers | As-is projection s | Sorted projection s | Reduction |
| --- | ---: | ---: | ---: | ---: |
| Fixed64, 1M | 4 | 0.343621 | 0.312060 | 9.2% |
| Fixed64, 1M | 28 | 0.205888 | 0.180632 | 12.3% |
| Fixed64, 16M | 4 | 11.789560 | 12.438800 | −5.5% |
| Fixed64, 16M | 28 | 4.735098 | 4.804807 | −1.5% |

The selected dominant term is `g3`, with 64 coefficients, unsorted in the
original input and sorted in the paired arm. The 16M main-projection
`time_setup/time_solve` values are 0.844177/9.388235 seconds as-is versus
0.516879/10.470788 sorted at four workers, and 0.644289/3.065378 versus
0.377849/3.425807 at 28 workers. Across each complete as-is 16M fit, frame
gathers sum to 0.161995/0.162676 seconds, incoming permutations to
0.336065/0.330757, and outgoing permutations to 0.374020/0.394567 at 4/28
workers. These are summed call clocks with concurrent RHS work; they are not
additive elapsed-time components. Sorted arms remove frame gathers and reduce
the permutation calls to no-ops, yet do not improve the measured projection.

Every fit constructs two Solvers and runs two batches: the main projection
and final residual projection. The fixed64 main batch has six RHSs; the dense
control has eighteen. No retained `BorrowedLsmrPrepared` session is used by
these automatic fits. The 4M three-dense controls take 4.781414/1.897625
seconds in projection at 4/28 workers. The largest coefficient difference
under row sorting is 7.63e-14; covariance differences are at most 1.86e-20.

Panel `8034071` completed all ten fits in 91.63 seconds; scheduler accounting
is failed=0/exit=0, 97 seconds, 8.653G maxvmem. Two allowed fits remain unused;
there was no measurement retry. The diagnostic plugin (`3802d41b…`) builds
source `c8851b5c` with trace-only changes to within `297f32d9`. Its Linux binary
audit, actual-plugin ABI/load tests and numeric guards pass. Initial setup
`8034036` exited 101 because the offline cache lacked the within test dependency
`proptest`; after fetching pinned test dependencies, `8034061` passed two
observation and eight design tests (failed=0/exit=0). Four interleaved trace
writes retain complete phase-tagged payloads; the decoder recovers both events
and checks every expected call count. Indexed fereg traces establish RHS
decisions independently of completion order. No observation was dropped.

The [compact record](../bench/results/h-20261013-h1.json) retains the plan,
hashes, clocks, comparisons, environment and failed setup. Raw captures and
diagnostic sources remain in its SCC run directory. These single diagnostic
fits establish the stopping-rule outcome, not a confidence-bounded speed claim
or Mac qualification. Production tokens and artifact inputs remain unchanged;
this documentation/result commit reuses current runtime validation.

<a id="c2-qualification-follow-up-stopped-without-adoption-2026-10-12"></a>

## C2 adoption with unresolved timing, 2026-10-12

After reviewing the stopped qualification results below, Johannes chose to adopt
the frozen C2 implementation and move on, accepting the remaining performance
uncertainty. Main now includes the candidate without further production edits.
This is a scoped adoption decision: the inconclusive timing cells remain
inconclusive, the existing performance allowance and confidence rule are
unchanged, and uniform nonregression or a release speedup is not established.
No further timing or CPU-placement experiment was run for this integration.

The [retained qualification evidence](../bench/results/c2-20261012-qualification.json)
checks the frozen arithmetic-preserving candidate
[`3a3c5d1b`](https://github.com/johannes-schmieder/fereg/commit/3a3c5d1b63f0ca9c751e38b9baa7dd5bde1979f2)
against `d9865e57`. The candidate parallelizes bounded integer/membership
validation and uses optional, stable counting/scatter scratch after existing
memory admission.
The [earlier evidence](https://github.com/johannes-schmieder/fereg/blob/3a3c5d1b63f0ca9c751e38b9baa7dd5bde1979f2/bench/results/c2-20261011-performance.json)
retains its large-row gains, rejected policies and original inconclusive results.

This follow-up made no production edits. Original task binaries had been deleted
under the cleanup policy. Rebuilt Linux plugins have different hashes, although
the runtime source, artifact inputs, frozen case bytes and harness hashes match.
Old and fresh timing batches are therefore kept separate. No additional samples
were taken after the fixed panel, and no allowance or confidence rule was relaxed.

Mac correctness passes: `run_tests.sh`, universal binary audit and actual-plugin
ABI/load, 73 exact replay cases, 219 additional worker checks at 2/4/16 workers,
eight mapped licensed fixtures, and both OLS and public-scope PPML Break tests.
The checks cover the existing independent scientific witnesses, repeatability,
discrete decisions, memory admission and transaction/rollback boundaries.
Mac baseline/candidate plugin hashes are `b5d1f650` / `cf4769b4`; full hashes are
in the retained record. There is no new Windows qualification.

The Mac panel used all twenty hash-verified standard inputs, one excluded warmup
and fifteen alternating pairs per cell. All 640 calls preserve exact science and
solver signatures. Nineteen cells clear the existing paired-bootstrap upper
bound against max(5%, 1 ms). K=129/four workers remains inconclusive:

| Fresh batch and case | Median paired candidate/baseline ratio | Ordinary 95% bootstrap interval | Outcome |
| --- | ---: | ---: | --- |
| Mac, wide OLS K=129, four workers | 1.02620 | [0.96849, 1.05702] | Nonregression not established |
| SCC, no-FE OLS K=5, four workers | 0.97783 | [0.95123, 0.99317] | Ordinary bound clears |
| SCC, three-FE OLS K=5, one worker | 0.99658 | [0.97447, 1.01189] | Ordinary bound clears |
| SCC, probability-weighted two-FE OLS K=5, four workers | 0.96019 | [0.92581, 0.96910] | Ordinary bound clears |
| SCC, AKM OLS 1M/K=16, four workers, RRUNTIME total | 1.02393 | [0.97922, 1.16442] | Nonregression not established |

The Mac boundary cell's separate time medians are 0.370598 and 0.373725 seconds
(+0.84%); that different statistic cannot replace the predeclared paired result.
Other Mac observations include paired reductions of 3.6% for weighted two-FE/four
workers, 1.7% and 2.0% for K=20 at four/sixteen workers, and 1.3% for PPML/four
workers. Single-worker point estimates stay within 1%. These are bounded
observations, not uniform gains or a completed release-performance claim.

Johannes made the Mac available. No competing qualification builds, tests,
transfers or agent tool workloads ran during timing; monitoring only read the
existing process output. The 24-core Mac's bookend snapshots still show desktop
activity: Activity Monitor at 59.6%/35.1% of one core and sysmond at 21.3%/9.9%.
These observations neither establish continuous idleness nor identify the cause
of the inconclusive cell. Every pair remains in the analysis.

SCC job `8033696` reserved all 28 cores of `scc-wo4`, the original timing host,
with affinity permitting all 28 CPUs and library thread pools capped at one.
The three previously unresolved standard configurations received one warmup plus
fifteen pairs (96 exact calls). Their ordinary intervals clear, but the
predeclared conservative selection/repeated-look sensitivity uses an exact
order-statistic upper bound at tail `0.025/(28*2)`. The corresponding bounds are
1.04324, 1.05981 and 1.05030: only the no-FE cell clears that sensitivity.
The new targeted observations do not retroactively pass the original SCC panel.
Ordinary per-cell intervals do not constitute a familywise confidence claim.

The separate SCC AKM follow-up used the same 1M-row input and four workers,
with one excluded warmup followed by fifteen pairs. All 32 calls remain exact.
Total-time medians are 2.052897 and 2.363395 seconds, a ratio of 1.15125 (+15.1%).
The median of within-pair ratios is instead 1.02393, as reported above; the wide
interval and this difference expose substantial variability, not a reliably
established slowdown magnitude. Native-only time has paired ratio 1.02327 and
interval [0.97515, 1.20610]. Loading has paired ratio 1.00513.

Four separate traced pairs consistently reduce the recorded OLS preparation
phase from about 138 ms to 103 ms (25%). Later fitting, score certification and
covariance times vary in both arms and can more than offset that saving.
These diagnostic clocks are nested, and separate phase medians are not an
additive explanation of the untraced total. Four separate ABBA `perf stat` runs
have nearly equal whole-process instruction counts, but include substantial
loader/input/output work outside native timing. They cannot establish unchanged
native instruction cost or prove a frequency, cache, scheduling or NUMA cause.
User-only software-event zero counts do not establish absent migrations.

The first setup job `8033663` passed both Linux builds/load/numeric guards, then
failed because the input generator rejected an unsupported option (accounting
0/2). Focused job `8033677` reused those builds, corrected the invocation and
verified the original input/harness hashes (0/0). Timing job `8033696` completed
with accounting 0/0, 1,081 scheduler seconds and 1.029 GB maximum virtual memory.
The retained record also names two Mac launcher failures before their affected
checks: a required plugin-basename guard and SIGPIPE in the pre-timing process
snapshot. No timing observations were discarded or replaced for these repairs.

Independent design and evidence review recommended keeping C2 unmerged under the
existing gate before Johannes's adoption decision above. The proposed diagnosis
compares broad affinity with four fixed physical cores sharing a NUMA node/LLC, using
unchanged binaries and a consistent memory-placement policy. It would test a
placement hypothesis, not select favorable timings to waive qualification.
No such experiment has run; the bounded qualification experiment remains stopped.

The original follow-up commit changed only retained evidence and prose. Its
production-identity, retained-record consistency, relative-link and whitespace
checks passed; temporary local and SCC captures were deleted after the record
and hashes were committed. Integration preserves the reviewed candidate's
runtime and artifact inputs, so its Mac scientific, transaction, build/load and
timing evidence remains applicable. `check_complexity.py --compare-production
3a3c5d1b` passes for all 170 runtime modules and 58 artifact inputs, and source,
test and tooling files are byte-identical to that candidate. The full
`./scripts/run_tests.sh` integration gate passes on Mac, including Clippy,
scientific witnesses, allocation audits, complexity and native bridge/numeric
guards (log SHA-256
`3a6a5591cdb7e6e0c8475cad8cba7a8c224ea1cd011ccebd8c4383325a4f7726`).
The Linux-only mock audit is skipped on Mac as intended; prior Linux evidence
remains separate. Both retained C2 records are unchanged from their source
commits. Documentation links, the prior section anchor and whitespace checks
pass. Integration task outputs are removed after committing this summary.

## SCC optimized campaign rerun, 2026-10-12

The [benchmark comparison](fixest_benchmarks.md#optimized-rerun-october-12)
retains the completed rerun of source `d9865e57` (qualified B, C/D and E1),
using the exact 36 October 10 datasets. All 56 configurations and 579 planned
fit positions are represented; default and newly added strict OLS fixest arms
remain separate. Linux plugin SHA-256 is
`75faa55005e95787d8b1a711af738340c36bcf97205c6c8bde1d977aaa41ae23`.

Build `8028839`, independent smoke `8028840` and production array `8028841`
tasks 1–4 have complete successful accounting and application sentinels.
Frozen runtime and decoded dataset identities pass, as do Linux build/ABI/load
and numeric guards, 42 benchmark tooling tests and all 11 native smoke oracles.
All 168 native fits complete; exact repeatability passes on 56 configurations,
and cross-worker checks pass. The 165 completed historical native counterparts
have unchanged sample/discrete fields and agree within the coefficient,
covariance and objective tolerances. Historical saved fitted arrays were not
compared; each campaign's own output checks remain binding.

The largest fixest gaps persist. Four-million-row dense two-FE PPML is 26.767
versus 15.068 seconds; dense three-FE OLS is 8.253 versus 4.224. Fereg wins
26/35 eligible default comparisons, with one former loss becoming a win and
one small new observed loss. Reference failures, skipped fits and timeouts
remain explicit: 154 eligible arm cells, 32 descriptive-discrepancy cells and
seven with no completed timing. All corresponding configurations changed hosts
between campaigns; paired within-node ratios describe relative performance,
but their changes do not isolate the optimizations' causal effect. These are
bounded Linux observations, not release performance qualification.

Tier: retained benchmark evidence and documentation; no runtime, frontend,
scientific tolerance or artifact input changed. The original scheduler entrypoints
were wrapped only to copy and verify frozen datasets and require successful
predecessor accounting. Local copy/decoded-identity/tamper guards and shell syntax
passed before submission; their launcher hashes are retained. A login-Python
submission syntax error was corrected before any job was launched. No scientific
retry occurred. Evidence identities, CSV reconstruction, local links and
whitespace are checked at this documentation boundary; existing qualified
runtime evidence is reused. Raw captures remain at the report's SCC location.

## Performance-plan qualification

### C2 parallel preparation candidate, 2026-10-11–12

The [retained C2 evidence](../bench/results/c2-20261011-performance.json)
compares the isolated candidate with `d9865e57`. This is arithmetic-preserving
work: integer range checks and membership validation use the installed worker
team, and an optional counting/scatter constructor preserves the original
ascending observation order within every canonical level. Ordered error joins
preserve first-error precedence. Factorization, singleton semantics, sample,
rank, omission, separation, floating-point reductions and solver policy are
unchanged. Independent source factorization and a general topology rewrite
remain outside this candidate.

The counting/scatter path serves eligible unweighted reusable membership.
Optional membership scratch is admitted only after the original Fast/Auto/Low
route and existing cache decisions. Low mode receives none; insufficient spare
memory or optional allocation failure falls back to the original constructor.
The offer is capped at min(8N bytes, 128 MiB), source constructions are sequential,
and actual scratch capacities are checked. PPML consumes the offer only in
generation zero; later solver/cache admission excludes this already-freed
scratch. At 16M rows and 160,000 firm levels, twelve useful blocks require
46,080,288 transient bytes on the measured 64-bit hosts. Retained preparation
bytes are unchanged.

The final general scheduling rule requires at least four useful blocks and
limits block-count × level-count to N/8. A broader N/4/two-block prototype
regressed 1M-row membership construction by 43–51% at N/8 levels. Raising the
minimum to four still regressed the N/16-level boundary by 13–30% on a second
host. The final N/8 work cap declines those cases at every size, including large
cases where the broader policy had helped. Its one bounded final density
screen found no median increase beyond max(5%, 1 ms). All three policies'
observations remain separate in the evidence; no further constant tuning or
sampling continuation was used.

Final preparation diagnostics on SCC `scc-ed3` used two alternating process
pairs, each with one warmup and two measured preparations. Generation, cloning
and comparison were outside timing; the gated stage clocks were enabled.
Complete-result fingerprints and retained bytes agree across arms.

| AKM preparation, four controls | Workers | Baseline s | Candidate s | Reduction |
| --- | ---: | ---: | ---: | ---: |
| 1M rows | 16 | 0.052202 | 0.036342 | 30.4% |
| 16M rows | 16 | 0.930398 | 0.555805 | 40.3% |

These are preparation observations, not whole-fit or Mac speed claims. Earlier
three-way diagnostics separate the validation and membership changes; the final
artifact has its own measurements. At 16M rows, unchanged source factorization
still takes about 0.147 s in this probe, so substantial serial work remains.
The probe supplies no overlapped-load hint; complete native fits retain C1's
existing overlap and must be measured separately.

The final native panel reserved all 28 cores of SCC `scc-wo4` (two Xeon
E5-2680 v4 sockets), with tracing disabled. The twenty frozen standard workloads
used one warmup and seven alternating pairs: all 320 calls retain exact science
and solver signatures. Seventeen cases pass the prescribed paired-bootstrap
nonregression bound. No-FE K=5/four workers, three-FE K=5/one worker, and weighted
two-FE K=5/four workers remain inconclusive; their point ratios are 0.9989,
1.0347 and 0.9605, but upper bounds are 1.0666, 1.0569 and 1.0782 against 1.05.
Every point estimate is within its allowance. There was no continuation or
resampling. Thus this panel does not establish full standard nonregression.

Separate campaign-shaped AKM inputs used seed 739391, unit weights, robust
inference and three alternating pairs without a dedicated warmup. The table
uses median RRUNTIME **total** seconds, including OLS loading, rather than the
OLS native-only field used by the driver's default summary. These are
descriptive observations, not confidence-qualified performance passes.

| Native transaction | Workers | Baseline s | Candidate s | Change |
| --- | ---: | ---: | ---: | ---: |
| OLS 1M, K=16 | 1 | 3.481277 | 3.516486 | +1.0% |
| OLS 1M, K=16 | 4 | 2.050752 | 2.185939 | +6.6% |
| OLS 1M, K=16 | 16 | 1.350190 | 1.296027 | −4.0% |
| PPML 1M, K=16 | 1 | 17.742044 | 17.762970 | +0.1% |
| PPML 1M, K=16 | 4 | 8.844992 | 8.584369 | −2.9% |
| PPML 1M, K=16 | 16 | 6.125746 | 5.978354 | −2.4% |
| OLS 16M, K=4 | 4 | 21.586503 | 19.842369 | −8.1% |
| OLS 16M, K=4 | 16 | 12.824662 | 11.467164 | −10.6% |

The four-worker 1M OLS slowdown remains unresolved. Existing load and solver
clocks do not isolate its cause, and no extra samples were taken to erase it.
At sixteen workers, PPML's complete preparation phase changes from 0.921854 to
0.868597 seconds (5.8% lower), much less than the isolated unit-weighted OLS
preparation probe. Faster preparation therefore does not imply a corresponding
whole-fit gain or monotonic improvement with worker count.

The 16M panel's original 900-second budget includes replay transport and result
validation. It stopped during the ninth call, after eight successful calls.
One fixed continuation on the same host was limited to the four missing
originally planned calls and 600 additional seconds. The original timeout and
every successful observation are retained separately. No completed call was
repeated, and the intended design remains three pairs per cell. This operational
continuation makes no confidence-qualified claim and does not revisit any
standard-panel cell.

All 48 successful campaign-shaped calls retain exact baseline/candidate science
and solver signatures. Across worker counts, the retained headers preserve the
existing discrete-decision policy and the complete streamed columns have equal
hashes; coefficients and covariance agree within the existing replay tolerances.
The continuation finishes with scheduler accounting 0/0. The original timing
job's 0/1 exit remains explicit because of its budget stop. The larger OLS case
supports useful C2 gains, while the full record still falls short of uniform
nonregression or Mac qualification.

Independent review covered stable ordering, fallible allocation, original
admission boundaries and PPML scratch lifetimes. Witnesses include an independent
row-filter membership reference, malformed membership/error ordering, worker
counts 1/2/3/4/16, exact budget and density boundaries, injected reservation
failures, OLS moment-cache admission and PPML generation/rollback behavior.
The complexity ratchet records scoped increases of 231 core and 38 plugin
production lines, with no increase in oversized file/function counts.

The final source passes `./scripts/run_tests.sh` on SCC, including the
independent scientific oracles, allocator checks, Clippy, formatting and the
complexity gate. The Linux release artifact passes the glibc/dependency/export
audit, actual-plugin ABI/load/selftest and both UBSan numeric guards. Native
replay has 73/73 exact comparisons and 219 additional worker-count checks at
2/4/16 workers, with scientific agreement, fixed-worker repeatability and stable
discrete decisions. All eight mapped Stata fixtures and both OLS/PPML Break
checks pass. The evidence retains failed setup/launcher attempts and identifies
the completed components reused by their focused continuations.

Eighteen Linux-regenerated standard inputs differed from the pinned Mac hashes;
none were timed. Existing Mac generators recreated all twenty exact frozen
files in 9.7 seconds, then transferred them to SCC. This was data generation
only: no Mac build, estimation test or timing was run during this initial
experiment. At that point the candidate remained separate from main without
Mac qualification. The later Mac checks and adoption decision are recorded
[above](#c2-adoption-with-unresolved-timing-2026-10-12); neither experiment
establishes a release performance claim.

### A2 retained-guess prototype stopped at discovery, 2026-10-11

The [prototype record](../bench/results/a2-20261011-prototype.json) preserves
all twenty-four results, twelve baseline-bound scopes and recomputed comparisons,
phase rows, ordered traces, artifact identities and failed attempts. Every
scientific comparison passes, including exact sample, rank, omission, separation,
solver/fallback decisions and outer counts across workers. Six canonical calls
accept a guess. Total cost fails the predeclared benefit condition:

| Frozen PPML case | Workers | Baseline native s | Candidate native s |
| --- | ---: | ---: | ---: |
| AKM 1M | 1 | 9.604 | 9.483 |
| AKM 1M | 4 | 4.822 | 4.996 |
| AKM 1M | 16 | 3.832 | 3.757 |
| Dense 1M | 1 | 9.333 | 8.973 |
| Dense 1M | 4 | 5.082 | 5.058 |
| Dense 1M | 16 | 3.831 | 3.811 |
| Dense 4M | 1 | 38.958 | 38.843 |
| Dense 4M | 4 | 23.319 | 23.254 |
| Dense 4M | 16 | 18.999 | 19.025 |
| Sparse 1M | 1 | 35.673 | 35.215 |
| Sparse 1M | 4 | 17.458 | 21.671 |
| Sparse 1M | 16 | 20.116 | 20.537 |

Native totals are 191.026 versus 194.623 seconds, a 1.883% increase; RRUNTIME
totals are 191.013 versus 194.610 seconds. These are single traced, baseline-first
pairs on shared SCC host `scc-gd4`, not repeatable quiet-Mac performance evidence.
Candidate tracing and all setup, conversion, initial residual products, retries
and certification are included. The result stops discovery; it does not isolate
the cause of the observed difference. The plan separately explains why the
iteration counter alone does not establish saved operator work.

The [disposable reproducer](../bench/results/a2-20261011-prototype.patch)
binds base `e975bf3d`, eight runtime/test files and portable research tooling.
All twenty-one resulting file hashes match an exact-base application using
`git apply --unidiff-zero --whitespace=error`. The failed near-roundoff variant
is preserved as encoded patch bytes with a checked decoder. Two substantive repairs address a
nonimproving initial residual and retained-bank cleanup on early errors.
Sixteen focused runtime witnesses and thirty-four driver guards pass; all
seventeen test invocations and their failures/costs are retained (379.524 seconds).

Build 8028183 on `scc-ei3` passes Linux binary/ABI/load and numeric guards,
with accounting 0/0, 113 scheduler seconds and 4.230 GB maximum virtual memory.
Initial panel 8028199 rejects valid output metadata before fitting: zero fits,
10.680 application seconds, accounting 0/1. The narrow header-decoder correction
preserves this failure and the frozen runtime/input bytes. Corrected panel
8028338 completes twenty-four fits in 598.391 application seconds, accounting
0/1, 611 scheduler seconds and 6.226 GB maximum virtual memory. Exit 1 preserves
the failed cost gate; every terminal identity check passes. The discovery book,
including four eligibility fits, totals twenty-eight fits and 736.220 seconds.

Tier: decision and retained research evidence, unchanged production artifact.
The prototype has a declared numerical and inner-work scope but is not integrated.
No dependent full integration suite, mapped strict/dummy/covariance qualification
or quiet-Mac timing was run after the failed discovery prerequisite. Production
remains qualified E1, with no tolerance, sample/rank/omission/separation, route,
certificate, fresh strict PPML, memory-control or SPI change. Existing full-suite
evidence for identical production inputs is reused; retained-data integrity,
reproducer hashes, local links, production identity and whitespace are checked
at this documentation boundary.

### F2 geometry pricing and the missing total-cost model, 2026-10-11

All six frozen Auto fits retain exact public science with the task-only
geometry observer. The [pricing record](../bench/results/f2-20261011-geometry.json)
binds inputs, artifacts, both observations per fit and ten independent feature
checks. The [disposable reproducer](../bench/results/f2-20261011-geometry.patch)
preserves the checked contingency calculation and tooling against base
`67ba321a`. It changes no solver or numerical acceptance policy.

The dense 1M/4M four-sweep bounds are 1.916e-16 and 1.752e-17; sparse pair
bounds saturate at one and give a vacuous bound of 1393. Bounds match across
workers and primary/residual calls. They concern exact arithmetic with true
unit weights and one common constant; they are not floating-point convergence
certificates. Scratch capacities are 496, 792 and 80,000 bytes respectively.

| Frozen OLS case | Workers | Primary geometry ms | Residual geometry ms | Diagnostic allowance ms |
| --- | ---: | ---: | ---: | ---: |
| Dense 1M | 1 | 32.52 | 32.76 | 143.57 |
| Dense 1M | 16 | 33.97 | 36.25 | 42.30 |
| Dense 4M | 1 | 142.56 | 154.84 | 782.95 |
| Dense 4M | 16 | 146.02 | 162.27 | 177.26 |
| Sparse 1M | 1 | 21.06 | 21.00 | 595.94 |
| Sparse 1M | 16 | 20.94 | 21.50 | 178.00 |

Each feature computation includes three counting and three clearing passes,
allocation, marginal checks and trace cost. The allowance is max(5%, 1 ms)
against current-host native time minus both observer closing clocks. This
subtraction is an approximate diagnostic reference: it cannot remove cache
effects or supply an uninstrumented confidence interval. A hypothetical cache
charges the primary computation once; sparse overhead is 0.18%/0.59%, while
uncached dense sixteen-worker overhead is 8.30%/8.70%. No cache was implemented.
Earlier MAP/checkpoint timings on another host were not pooled with these costs.

The feature passes this discovery cost gate, but a total-cost selector remains
unidentified. N, levels, preconditioner and observed LSMR iterations are
confounded; rollback-copy and failed-attempt costs were not measured. The
plan names the missing controlled calibration and conservative comparison
against unavoidable Auto setup. No coefficients, route prototype or holdout
were produced. A passing geometric prediction would still require the ordinary
independent numerical certificate and the unchanged shared fallback budget.

Pricing job 8028143 used 127.343 application seconds on `scc-gd4`; all six
captures and terminal identity checks passed, without retry. Its isolated
build, job 8028142 on `scc-tb4`, passed Linux binary/ABI/load and numeric guards
in 170 scheduler seconds with accounting 0/0. Pricing accounting is also 0/0,
with 154 scheduler seconds and 5.125 GB maximum virtual memory. This is bounded
Linux diagnostic evidence, not quiet-Mac qualification.

Tier: decision and diagnostic tooling, unchanged production artifact. Independent
contingency/projection witnesses and exact retained outputs support the feature
observations only. No tolerance, sample, rank, omission, separation, route,
certificate, fresh strict PPML, memory control or SPI ownership changed.
Existing full repository evidence for identical E1 production inputs is reused;
retained-data integrity, local links and production identity are checked at
this documentation boundary.

### A3 calibration stopped at the sparse work limit, 2026-10-11

The [calibration record](../bench/results/a3-20261011-calibration.json) retains
all 62 attempted kernel calls and their ordered trace events. The first 36
passed the independent paired-dyadic projection and direct-Gram witnesses.
The additional panel completed twenty calls, refused five under unchanged
admission gates and timed out on its twenty-sixth call. That case has 16,384
rows, 2,048 levels on each side, degree four and seventeen RHS columns. Explicit
iterative geometry completed eleven RHS solves of 962–963 iterations before
the unchanged 30-second subprocess limit. It has no final certificate or
completed timing ratio. The remaining 154 calls were not attempted; there was
no retry, fitted rule, holdout or production selector.

The completed small kernels show both sides of the setup/solve tradeoff. On
the first host, degree-four one-worker direct solving took about 0.0238 seconds
versus 0.0430 for implicit; degree-64 sixteen-worker implicit took about 0.00895
versus 0.0144 for direct. These are two traced kernel repetitions, not quiet-Mac
qualification. The first and additional panels ran on different SCC hosts;
their wall times were not pooled. The frozen starting-book hash and final
artifact identities match exactly. Additional raw spectral features explicitly
remain uncertified diagnostics, not floating-point iteration guarantees.

The first kernel panel used 4.787 application seconds; the additional panel
used 34.136 seconds including validation. Together with the earlier eight
whole fits, the shared A3 book totals 253.263 seconds. Corrected plugin-FMA
builds cost 563 and 429 scheduler seconds, both with accounting 0/0. Kernel
jobs 8028122 and 8028140 took 17 and 46 scheduler seconds, respectively;
the latter reports `failed=0`, `exit_status=1`, preserving the timeout.
No unused call or time allowance authorizes continuing a stopped panel.

The [disposable reproducer](../bench/results/a3-20261011-calibration.patch)
binds base `57c5cd1e`, the diagnostic source, both frozen kernel generators and
drivers. It installs the unchanged runtime-selected plugin FMA table while
the generic core is compiled without global FMA. It is retained research
material, not an instruction to rerun the stopped campaign.

Tier: decision and diagnostic tooling, unchanged production artifact. The
independent witnesses above support completed kernel observations only.
Tolerances, sample, rank, omission, separation, default routes, certificates,
fresh strict PPML, memory controls and SPI ownership remain unchanged. Existing
full repository checks for identical E1 production inputs are reused; local
links, retained-data integrity and production identity are checked at this
documentation boundary. No new platform or release-performance claim follows.

### A2/A3/F2 first diagnostic panels, 2026-10-11

The [retained discovery record](../bench/results/fable-20261011-discovery.json)
binds twenty-four traced SCC calls to the qualified E1 numerical program and
the committed diagnostic patch. Production defaults remain unchanged. This
record contains opportunity and rejection evidence, not performance qualification.
It packs the complete scientific matrices without changing any bits and keeps
the input identities, comparisons, phase costs and failed attempts.

All four A2 preflight scientific fingerprints exactly match retained E1 Linux.
Initial/strict anchor tuples match on every case. AKM has a compatible iterative
strict consumer whose reduced solve costs 0.959 seconds; dense 4M has only
0.056 seconds of such work, and dense 1M uses direct solving. Sparse has 445
matching component anchors but both strict canonical attempts decline. These
results support a bounded A2 prototype, chiefly for AKM, while requiring sparse
fallback decisions to remain exact. No guess was retained or warm solve run.
The four calls took 127.149 seconds including capture and validation.

All eight A3 whole-fit calls completed, and all four scoped scientific
comparisons passed with four outer iterations in every fit:

| Frozen dense PPML | Workers | Current native s | Forced implicit native s |
| --- | ---: | ---: | ---: |
| 1M | 1 | 9.332 | 8.349 |
| 1M | 16 | 3.806 | 2.721 |
| 4M | 1 | 39.053 | 31.699 |
| 4M | 16 | 18.427 | 10.896 |

Implicit reduced solving costs more in every cell. Avoiding serial pair
assembly saves about 1.08 seconds over two canonical calls at 1M and 7.9 seconds
at 4M, explaining most of the total differences. Planned canonical storage
increases from 12.05 to 24.46 MB and 48.51 to 96.92 MB respectively. No pair-work
admission rejection was exercised, so these results do not support broadening
that gate. This is one traced pair per cell on a shared host; neither a general
iteration predictor nor a quiet-Mac performance claim follows. The stage used
214.340 seconds of its shared 720-second first-subset budget.

F2 completed all twelve checkpoint calls in 229.751 seconds: ten successful
captures and two sparse MAP iteration-exhaustion failures. The four dense
comparisons pass the unchanged coefficient, covariance and strong fit-statistic
bounds. Sparse's four MAP sweeps plus checkpoint certificate cost 1.725 seconds
at one worker, against a 0.614-second allowance from same-host Auto. Even granting
free final-residual work leaves at least 11.30% modeled overhead before copying,
setup and the initial certificate. This rejects the general four-sweep pilot.
At sixteen workers, numerical work alone is 0.286 seconds against 0.138 seconds;
the hypothetical free-residual bound is inconclusive. No failed fit receives a
completed timing ratio, and no conditional geometry rule has been qualified.

Bundle job 8028069 reports scheduler `failed=0`, `exit_status=1`, 600 seconds and
6.253 GB maximum virtual memory; the nonzero exit preserves the two expected MAP
failures. Frozen input reconstruction first failed its complete-file identity
gate because new templates changed OLS VCE and PPML separation metadata. The
original extractor was recovered, and all seven original complete-file hashes
then matched: corrected job 8028057 reports `failed=0`, `exit_status=0`, 45 seconds
and 2.401 GB. No fit used the failed extraction. A generic core calibration build
was canceled before measurements because it did not install the plugin FMA table;
its 283 seconds remain a cost, not calibration evidence. Corrected calibration
and all prototype/holdout results are outside this record.

Tier: decision and diagnostic tooling, unchanged production artifact. No
tolerance, sample, rank, omission, separation, default route, independent
certificate, fresh strict PPML requirement, memory control or SPI ownership
changed. The recorded full repository checks for the identical E1 production
inputs remain applicable; these observations add no platform qualification.

### F1 numerical diagnosis: accepted projected controls, 2026-10-11

The [retained diagnosis](../bench/results/f1-20261011-diagnosis.json) isolates
the material weak-control Auto robust-F discrepancy in its initial accepted
within-X bank. Against independent FE QR, the largest within-X error is
2.57e-6 for Auto and 3.27e-13 for LSMR. An independent inverse of the same
accepted score design differs from Auto's bread by only 1.03e-18. The
80-digit F decomposition attributes +1.4398833e-5 to the accepted score design,
+1.08e-11 to bread inversion and -1.95e-11 to remaining covariance arithmetic;
final residual and Wald effects are also small. This diagnoses the error source
on the captured weak control; it does not declare a production repair.

Eight one-worker Mac captures used qualified E1 source `d3fed590`: Auto at
1e-8, diagnostic Auto at 1e-10, LSMR and valid CMG on the hash-matched barbell
and weak controls. All coefficient, covariance, fitted/residual and within-statistic
witnesses passed. Four strong-F checks passed and four failed at the unchanged
rtol=1e-12/atol=1e-14 bound. Tighter Auto still failed both controls, by factors
2.8194 and 1.1763; scalar tightening is not an established repair. Valid CMG
and LSMR passed these two independent F witnesses. The original sixty-fit
panel's twenty invalid requests and eight comparison failures remain failed.

Two follow-up Auto/LSMR captures used a
[task-only diagnostic patch](../bench/results/fable-20261011-diagnostic.patch).
Every public scientific bit, solver signature and external observation byte
matched its uninstrumented arm. Both capture lengths and terminal markers were
exact. The record binds qualified/probe/harness artifacts, all scientific
matrix banks, projection/bread/scale mappings, and independent QR and 80-digit
algebra. Full-data QR remains binary64; compensated Gram/meat product rounding
and independent same-score QR cross-checks are recorded explicitly.

The eight-call stage took 4.876 seconds within 120 seconds per call and 600
seconds total; the two-capture stage, including decomposition, took 4.081
seconds within 60 seconds per call and 120 seconds total. Neither stage retried.
These are diagnostic task costs, not performance qualification: the probe's
8 KiB capture writer runs inside existing native/certificate clocks. No
production runtime, tolerance, model decision or default route changed, and
no handoff threshold follows from these captures.

The unchanged-artifact check against `57c5cd1e` matches all 170 runtime modules
and 58 artifact inputs. `./scripts/run_tests.sh`, local documentation links and
whitespace checks pass; the test transcript digest is
`fa1fc1a468aa4f4c571225c908833e57569c9375cf268cac07e202315128a2f9`.
The transcript is not retained, and these checks make no new timing claim.

### Performance-plan F: comparative failures and route decisions, 2026-10-11

The automatic routes remain unchanged. The
[retained F1/F2 comparisons](../bench/results/f-20261011-comparisons.json)
use qualified E1 source `d3fed590` and its Linux native artifact
`1f6de670a17c6bc08d2fa3e482c4cdec80e1903cadb5313fc27531c80502cf9f`.
They are bounded diagnostic comparisons of existing explicit requests, with
phase/solver tracing and two cold, reversed-order repetitions on a shared SCC
host. They do not qualify a route change or a performance claim. Frozen input
and body hashes, every fit status, exact-repeat checks, phase/solver/memory rows,
certificate gates and scheduler accounting are retained. Large campaign cases
have cross-route checks and independent runtime certificates; the three smaller
graph controls also have frozen full-dummy QR/HC1 witnesses.

F1 job 8027562 attempted all sixty fits: 32 passed and 28 failed. Twenty CMG
requests failed syntax because the task driver retained `preconditioner=auto`,
which correctly requires `off` for explicit CMG. These are request-rejection
costs, not CMG solve observations. Eight LSMR comparisons on barbell and weak
mobility failed the unchanged stronger robust-F bound (rtol=1e-12, atol=1e-14),
by factors 1.63 and 595.04. Both routes passed independent coefficient,
covariance, fitted/residual and within-statistic checks. An additional
independent QR/HC1 Wald calculation puts Auto farther from that reference than
LSMR on those controls; it does not override the failed comparison, establish a
production bug by itself, or authorize a relaxed bound.

F1's raw one-worker native costs include about 0.175 seconds for Auto versus
0.114 for LSMR on the chain, but 1.77 versus 7.29 on frozen 1M AKM and 1.18
versus 3.75 on dense two-FE 1M. No failed cell receives an accepted ratio.
The panel took 221.734 process seconds (224 scheduler seconds); accounting
reports `failed=0`, `exit_status=1`, as expected for a failed comparison.
There was no retry. The observed chain reports 127 iterations; the historical
4,246-iteration reproducer was unavailable, and ordinary PCG's iteration-eight
progress was not observed. A handoff threshold cannot be calibrated from this
panel.

Before F2's first fit, the invalid MAP request construction was corrected to
`preconditioner=off`, guarded by an exactly-one-original-auto check. All other
arguments and body bytes, tolerances and the predeclared limits remained
unchanged. The original held job 8027564 was cancelled without running; new
job 8027931 used separately hashed tooling and qualification bindings.

F2 attempted all 24 fits: twenty passed, and all four sparse MAP fits timed out
at 120 seconds, at both one and sixteen workers. The 1,200-second panel budget
was not extended: elapsed process time was 931.087 seconds, scheduler time
932 seconds, and maximum virtual memory 5.517 GB. Accounting reports
`failed=0`, `exit_status=1`; there is no success sentinel. Sparse Auto completed
in 12.18–12.32 native seconds at one worker and 2.95–2.98 at sixteen. Timeout
durations are whole-subprocess limits, not completed native timings.

Dense median native seconds, diagnostic only:

| Frozen case | Workers | Auto | Explicit MAP |
| --- | ---: | ---: | ---: |
| 1M three-FE | 1 | 3.140 | 2.724 |
| 1M three-FE | 16 | 1.035 | 0.911 |
| 4M three-FE | 1 | 16.803 | 12.919 |
| 4M three-FE | 16 | 4.814 | 3.519 |

The 4M Auto sixteen-worker observations range from 4.324 to 5.303 seconds;
two shared-host repetitions cannot establish a confidence bound. Auto used
Diagonal LSMR with four primary iterations on dense 1M, Schwarz LSMR with ten
on dense 4M, and Schwarz LSMR with 22 on sparse 1M. Dense MAP took four primary
iterations. Costs include different setup, projection and final certificate
work, not just the primary iteration count. For example, one-worker modeled
native peaks were 1.125/0.554 GB (Auto/MAP) on dense 1M and 5.461/2.216 GB on
dense 4M. These planner estimates are not observed RSS. Timeout fits have no
completed solver/certificate/memory packet and receive no diagnostic ratio.

The [F1 and F2 follow-ups](fable_performance.md#f1-authorized-diagnosis-resolve-the-comparison-before-handoff)
recommend retaining Auto. F1 needs bounded numerical/request/progress diagnosis;
F2 needs a defensible geometry-and-total-cost model and a separately authorized
holdout before any route prototype. Johannes subsequently authorized their
bounded follow-up, including separate A2/A3 discovery and costing, as recorded
in the plan; that authorization changes none of these failed outcomes. No default
route, tolerance, model semantics, certificate, fresh strict PPML solve, memory
policy or SPI ownership changed in this comparison work. Production identity
against E1 remains exact across 170 runtime modules and 58 artifact inputs;
its qualified runtime evidence is reused for this documentation-only change.

### Performance-plan E1: qualified declared numerical scope, 2026-10-11

E1 passes against qualified C/D (`aeb6ee64`). The
[retained observations, scopes and artifact identities](../bench/results/e1-20261011-performance.json)
include every Mac timing pair, Linux instruction counts, independent numerical
witnesses and failed attempts. Fixed 512-row leaves and ordered merges make the
norm and independent PPML finite-score reductions independent of worker count.
They reuse idle, already-admitted scratch and preserve the single-leaf arithmetic.
No tolerance, sample, rank, omission, separation, solver route, inner-work counter,
certificate decision or outer iteration changed. Strict/canonical PPML solving
and final-Hessian checks remain fresh. No worker touches SPI.

Scaled norm summaries retain their largest rounded weighted element, preserving
the D1 rejection proof. Score leaves retain both compensated components: the
first attempted implementation rounded each leaf to a single total and lost a
residual of one across `1e30`, `1`, and `-1e30`. Independent review rejected it
before qualification. The repair has signed-cancellation and exact dyadic
witnesses. Norm witnesses cover fixed leaves, dynamic scales, errors, scratch
capacities and 1/4/16 workers; independent full-dummy weighted QR covers the three
32,768-row chain/barbell/weak-mobility controls and their robust covariance.

The first large comparison also failed its scope: OLS Gaussian null likelihood
depends on the already-scoped within-TSS. The original manifest and failed
comparison remain failed in the retained record. Before fresh candidate calls,
the corrected manifest explicitly named that leaf and added independent FE-only
weighted-QR witnesses at the existing rtol=1e-12/atol=1e-14 statistic bounds.
No runtime, tolerance, decision or work-counter exception repaired this failure.
Other recorded repairs concern external-column validation, constant visibility,
Clippy style and a superseded scalar-order test assertion.

The full repository gate passed: 1,176 core tests, 143 non-FFI plugin tests,
173 FFI plugin tests, ten isolated allocator audits, portable checks, independent
oracles, transaction mocks and complexity. All 73 small replay cases remain
exact against C/D; additional 4/16-worker checks retain exact repeats and
protected decisions. All 67 licensed fixtures and public OLS/PPML Break probes
passed. These small cases do not certify the new multileaf arithmetic by
themselves: the separate Mac 36-fit graph panel, SCC 84-fit campaign/graph panel
and Mac 40-fit timing-input science preflight all passed their explicit scopes,
protected decisions and numerical witnesses. The final source matches the frozen
measured artifact inputs across 170 runtime modules and 58 other inputs.

SCC job 8027197 completed 84 discovery fits at 1/4/16 workers in 1,278 scheduler
seconds (6.543 GB maximum virtual memory), with successful accounting. Profiling
job 8027203 completed eight native witnesses and sixteen instrumented fits in
482 scheduler seconds. Native/instrumented science and repeats agree exactly.
Two instruction-count repetitions show dense PPML +1.179%, sparse PPML +1.046%,
AKM PPML +0.215%, and dense three-FE OLS -0.269%. Library-FMA instruction cost
is zero in every profile. These are costs as well as gains; prior timing was
not reused because the positive changes exceed the observed repeat ranges.

Fresh Mac timing completed all 640 fits across twenty standard cells in
1,269.685 seconds of the 1,800-second budget. Every adjusted upper bound passes
max(5%, 1 ms), with no continuation. The exact two-look rule is declared in the
[plan](fable_performance.md#e-declaration-before-implementation-2026-10-11).
Observed K=20 median reductions are 2.53%, 2.16% and 4.39% at 4/8/16 workers; single-worker
K=20 costs 0.27%. Weighted two-FE OLS at four workers is 1.48% lower, and unit-weight
K=5 is 2.82% lower. Single-worker PPML costs 1.36%; four-worker PPML is near zero.
No-FE four-worker OLS costs 5.47% (about 0.26 ms), within the unchanged 1 ms
allowance. Other cells are mixed and mostly near zero. The largest upper bound
among the 5%-allowance cells is 1.049008, so this is a nonregression qualification,
not a claim of a general speedup. Per-cell intervals and all observations remain
in the retained record. Background desktop applications stayed open; there were
no competing builds/tests/agent workloads or recorded thermal warnings.

E2 and E3 are deferred for lack of compatible work, as explained in the
[coverage decision](fable_performance.md#e2e3-coverage-decision-2026-10-11).
All observed ordinary PPML frames used the joint basis, bypassing E2's proposed
direct projection. E3 cannot subtract an FE fit twice from an already-within
residual, and the raw-residual paths lack the required retained bank. No new
route or storage was introduced to manufacture eligibility.

### Performance-plan C/D: qualified combined workstream, 2026-10-11

C/D passes arithmetic-preserving qualification. The
[compact observations and artifact identities](../bench/results/cd-20261011-performance.json)
retain the Mac pairs, confidence bounds, Linux instruction counts, campaign
phase rows and installation follow-up. No tolerance, sample, rank, omission,
separation, strict PPML solve or independent certificate changed.

The preserving candidate overlaps optional factorization of Rust-owned
categorical inputs with caller-thread numeric loading, for OLS and PPML.
Full classification, pruning, topology, DoF and generation ownership remain
authoritative after loading. A capsule can replace encodings only when original
rows, source identities and canonical values agree; unsupported layouts or
insufficient admitted storage decline the optimization. PPML consumes it once
in generation zero and charges its complete transient capacity. No worker calls
SPI. Errors, Break and panics join workers before rollback. Cancellation during
factorization can wait for the current source's factorization; the polling
interval is not a bound on that inner kernel.

C3 schedules residual reconstruction and completeness scans by rows without
changing each numerical recurrence. C4 safely initializes already-reserved
observation banks on the installed team, retaining allocation order and capacity
checks. D1 can reject the zero-fit shortcut from the first group's unchanged
certificate arithmetic only inside the independently reviewed finite-error
bounds; accepting shortcuts and final certificates still run fully. D2 reuses
certified norms only for the identical matrix and normalization. D3 tiles Schur
and Gram updates while preserving each element's observation order. The norm
and score reductions that would reassociate arithmetic remain in E. The C2
diagnosis below did not support speculative topology changes.

Costs include validation of reused encodings, an extra raw-weight minimum scan
for the bounded weighted screen, and zero initialization before parallel
residual reconstruction. Parallel initialization can expose fewer lanes than
requested when binary splitting encounters awkward sizes. These costs must be
judged with the combined measurements, not omitted from the performance claim.

The full repository gate passed, including 1,167 active core tests, 173 active
FFI plugin tests and all ten separately launched allocator audits, independent
numerical oracles and transaction mocks. The first gate stopped at two Clippy
style findings (an equivalent boolean form and a test struct initializer);
neither repair changed arithmetic or scientific policy. Independent review
found no blocking ownership, error-precedence or certificate-proof issue.
The universal Mac build passed ABI, dependency and actual-plugin load checks.
All 73 replay cases passed with zero exact differences against qualified B,
unchanged solver signatures, scientific witnesses and fixed-worker repeats.
All 67 licensed fixtures and OLS/PPML public Break probes passed.
Before commit, all 170 production module token streams and 58 other artifact
inputs were checked against the frozen measured source and remained identical.

Predeclared measurement bounds: SCC uses four frozen 50k shapes,
two repetitions per plugin, one worker, 600 seconds per instrumented fit and
2,700 seconds total; the seven full frozen campaign cases use 1/4 workers,
two alternating pairs, 120 seconds per fit and 2,400 seconds total. The C2/C4
AKM installation follow-up uses 1/4/8/16 workers, one warmup and three measured
pairs, 120 seconds per fit and 900 seconds total. Linux phase and solver traces
are diagnostic. The quiet Mac uses all twenty regenerated, hash-verified
standard workloads, including K=20 at eight and sixteen workers, with tracing
disabled and no competing measured-host work.

Mac timing has two predeclared looks: fifteen paired measurements initially
(1,800-second budget, 90 seconds per fit), then at most fifteen additional pairs
for cells whose point estimate passes but upper bound is inconclusive, in the
same sorted case order (900-second total continuation budget). An initial point
estimate above its allowance stops all continuation. Each look uses the larger
of the 98.75th-percentile paired-median bootstrap endpoint (10,000 draws, seed
739391) and an order-statistic median upper bound with binomial tail at most
0.0125. The latter uses rank 13 of 15 and rank 22 of 30; it avoids relying on
nominal small-sample bootstrap coverage. All valid pairs are retained and pooled,
and the ordinary max(5%, 1 ms) allowance is recomputed from the pooled baseline.
Artifact/input identities and science fingerprints must agree across blocks.
Incomplete blocks cannot qualify, and there is no third look. The median-ratio
inference assumes stationary independent pairs; the estimated-baseline allowance
retains the existing protocol's limitation. These are per-cell bounds, not
simultaneous intervals; overall acceptance requires every cell to pass.

Candidate/check SHA-256 identities: source archive
`bd9a50e3a0c39458abf13f4185b3576a9600a01eb91f268281e8d8072c8c11e5`,
source patch `237262a329cc496a4d0b85180ffe95f7a079cf448df0c5b1324886c45cba6e94`,
full gate `0d673bf089eb0235903f7116eadb6d3b19a5f4c1b834701b73e7d04242472f1d`,
initial style failure
`16476f2a994366016edf1a5cf2c3fd6b229c7b6618820272535229bad3aa13b8`,
Mac plugin `4e06dd13bd5d363a197c4d9cd0a0f8cfac99a7f46f815da1b137e0e4666d0d83`,
Mac exact-replay summary
`de03f92fe00bbca41c576dce5a8eca1191a1297a495b2565d4f60ef6798f7114`,
and timing protocol
`4e411fac43fe84201dba8aad34df2367edf3a6e453dacc1b4003bdb4a6097b1e`.

While SCC queued, a bounded Mac phase probe ran one traced baseline/candidate
pair on each frozen K=20 OLS case at 1/4/8/16 workers. All eight fits agreed
exactly. Native seconds were 1.19236/1.02968, 0.56890/0.53278,
0.46653/0.45228 and 0.44441/0.41507 (baseline/candidate). These single pairs
are diagnostics, not qualification. The probe summary hash is
`9fdf772cb0d1a86f96f6c66ee0e6164025c51c71ccb08e3afba481a9f426f378`.

SCC instruction job 8026782 stopped after 16 seconds because its copied driver
still required an executed `__fma_fma3` symbol in the comparator. Qualified B
already removed those calls. The corrected driver retains the exact libm build
ID, matching debug-file hash, validated Valgrind identity, full instruction
totals and native/scientific comparisons; it allows the expected empty FMA count.
No runtime artifact changed. The failed summary hash is
`fa7d0755d9cccf35a7d7a048dc28ccf8dade526d0787fd9afa2177536775b1dd`;
its log hash is
`ae3ec5a177430a44a8ae5487d6aa0a81a555efb5d87b552412bab5766953edc1`.
The bounded retry is job 8026800.

The quiet Mac panel completed all 640 fits in 1,280.54 seconds, below its
1,800-second bound. Every cell passed the predeclared upper bound; no
continuation was needed. The host was Mac14,14, Apple M2 Ultra, with 24 logical
CPUs. There were no competing builds, tests or agent tool workloads. Ordinary
desktop activity remained visible, including Activity Monitor and sysmond;
the qualification is not a claim of a bare operating system. All measured
outputs and solver signatures were exact across baseline, candidate and repeats.

Paired-median native-time changes (negative is faster):

| Frozen Mac family | Workers | Change |
| --- | --- | --- |
| OLS, no FE, K=5 | 1 / 4 | +0.10% / −0.21% |
| OLS, three FE, K=5 | 1 / 4 | −0.04% / −0.72% |
| OLS, two FE, pweights, K=5 | 1 / 4 | −9.77% / −4.80% |
| OLS, two FE, unit weights, K=5 | 1 / 4 | −10.56% / −6.86% |
| OLS, two FE, K=20 | 1 / 4 / 8 / 16 | −13.35% / −8.45% / −5.60% / −5.13% |
| OLS, wide K=64 | 1 / 4 | −5.36% / −4.24% |
| OLS, wide K=65 | 1 / 4 | −4.26% / −5.95% |
| OLS, wide K=129 | 1 / 4 | −3.19% / −2.53% |
| PPML, two FE, K=5 | 1 / 4 | +0.26% / −0.40% |

These point estimates do not establish every apparent gain. In particular,
the K=129 four-worker interval includes no improvement. The largest acceptance
upper bound among cells with the 5% allowance was 1.012652, below 1.05.
Small gains and the small PPML/no-FE costs remain observations, not failures.
Mac summary and assessment hashes are respectively
`82c4fcc86a14e13772b2ec6a5d190ca8e81fda8ad05d5534aacf4715295e2dc3`
and `e54085e4a599f141aac5263a99d51dd2ad65a35ecdf4a432001f4c020f892249`.

SCC build job 8026744 passed native/profiling ABI and load checks, all eight
hardware witnesses and the 73-case exact replay. Jobs 8026800, 8026783 and
8026784 passed with `failed=0`, `exit_status=0`, and scheduler wall times
472, 1,181 and 315 seconds. They shared `scc-ei3`, Xeon Gold 6226R at 2.90 GHz,
and overlapped, with 1/4/16 slots and observed affinity exposing 32 logical CPUs.
These are bounded Linux diagnostics, not quiet timing or Linux qualification.

The four 50k instruction cells had 16 instrumented fits and eight native
witness fits. Both repetitions and both plugins were exact; executed library
FMA instruction totals were zero under the validated symbol/tool bindings.
Mean instruction changes were −8.0583% (AKM PPML), −0.4261% (dense three-FE
OLS), **+0.0294%** (dense PPML) and **+0.0159%** (sparse PPML). The latter two
are small measured costs, not reductions or values erased as noise. The
retained counts include both repetitions; two samples do not establish a
precise sampling distribution. The panel took 468.56 seconds of its 2,700-second
budget.

The full campaign panel's 56 fits were exact at 1/4 workers and took 1,168.99
seconds of 2,400. OLS AKM improved in the observed pairs. Most PPML total times
were close; dense PPML at four workers had approximately 2.7% (1M) and 2.3%
(4M) positive costs. Two pairs on a shared host do not support a Linux speedup
or nonregression claim. All PPML phase rows and OLS rows 18–21 are retained in
the compact results, including these costs.

The 32-fit installation follow-up compared C/D with the earlier preparation
snapshot and remained exact. Its medians were:

| Workers | Preparation, before → after (s) | Installation, before → after (s) | Candidate observation bank |
| ---: | ---: | ---: | ---: |
| 1 | 0.67691 → 0.67561 | 0.11581 → 0.11586 | 8 MB |
| 4 | 0.53475 → 0.50528 | 0.12975 → 0.11197 | 32 MB |
| 8 | 0.50843 → 0.47562 | 0.13927 → 0.11497 | 64 MB |
| 16 | 0.51789 → 0.47624 | 0.15640 → 0.12048 | 128 MB |

This supports the preparation/installation hypothesis at higher worker counts,
but the comparison combines C1, D1 and C4 and is not an isolated component
ablation. Whole-fit PPML changes were much smaller than the subphase reductions.
Sixteen-worker involuntary context switches varied substantially; all three
measured observations and their ranges remain in the compact results. The
panel took 312.38 seconds of its 900-second budget.

Linux hashes: native plugin
`35b9b214faf160349a4dab2b2cffb31b025bb1c59169d555719a161b17c4d5a5`,
profiling plugin
`e2b9b9df0ad1a3b3867e8aa63227c82337a41c0136ef52bdfa646103c6adb297`,
exact replay
`7d805fc4e15debfc1ae643d7c0c5b5b8b871fe5419b8e071462484208ac42c07`,
instruction summary
`8295fc9aa5e4eb6cf0a2a393420d574c46b20de37e42012af7cca5fe360ed8dd`,
full campaign summary
`386ee98ff14ce729d4a4266ff2f931aa62346980c4fda910dd271e05a0aebd15`,
and installation summary
`dfd93521cb08bf016b284367c9a9bbc240448c5edb05faba895ec8042d95286d`.

### Performance-plan C/D: preparation diagnosis, 2026-10-11

The first C/D diagnostic snapshot contained C3 residual/sample scheduling,
D2 norm handoff, D3 ordered tiles and gated preparation clocks. It preceded
C1 overlap, D1's initial-certificate screen and C4 workspace first-touch.
Its 73 Mac replay cases had zero exact differences against qualified B,
including solver signatures, independent science checks and fixed-worker
repeats. The universal Mac build passed ABI, dependency and actual-plugin
load checks. The initial core suite passed 1,154 tests (three ignored), and
Clippy passed; these are snapshot checks, not final C/D qualification.

C2 first measured the preparation interval instead of assuming that the
plan's earlier worker-count growth persisted. SCC job 8026546 used the frozen
campaign inputs for AKM, dense and sparse two-FE PPML at one million rows,
1/4/8/16 workers, one B comparator, one candidate warmup and three candidate
measurements per cell. Every one of the 60 fits passed its certificates and
exact comparison, including solver signatures. Both phase and solver tracing
were enabled. The 120-second per-fit and 1,800-second total bounds held:
the panel took 980.68 seconds. The shared host was `scc-gd4`, Xeon Gold 6242
at 2.80 GHz, with 16 SGE slots and observed affinity exposing 32 logical CPUs.
Build job 8026545 and panel job 8026546 both had `failed=0` and
`exit_status=0`. These are descriptive Linux phase measurements, not quiet-Mac
qualification.

Median preparation seconds (RRUNTIME row 28), with the observed range across
three measured repetitions in parentheses:

| Frozen PPML case | 1 worker | 4 workers | 8 workers | 16 workers |
| --- | ---: | ---: | ---: | ---: |
| AKM 1M | 0.67560 (0.67530–0.67726) | 0.53218 (0.53154–0.53607) | 0.50815 (0.50425–0.51582) | 0.52153 (0.51199–0.54837) |
| Dense 1M | 0.62779 (0.62698–0.62916) | 0.48013 (0.47073–0.48105) | 0.44747 (0.44391–0.44766) | 0.45815 (0.45552–0.46162) |
| Sparse 1M | 0.64470 (0.64408–0.64761) | 0.50394 (0.48610–0.50459) | 0.47362 (0.47257–0.47670) | 0.48691 (0.48370–0.49475) |

Preparation spans input classification, initial model-generation construction
and IRLS setup; it is not just factorization/topology. On AKM, their respective
one-worker medians were 0.06481, 0.14424 and 0.46657 seconds. The nested
`shared_model` clock includes materialization, factorization, memberships,
topology and cluster binding; it fell from 0.13251 to 0.10719 seconds at sixteen
workers. IRLS workspace setup fell from 0.17291 to 0.05624 seconds and the
initial independent score from 0.08309 to 0.01724 seconds. Session installation
instead rose from 0.11657 to 0.15697 seconds; dense and sparse installation
also rose, from 0.08479 to 0.11610 and 0.10944 to 0.15101 seconds. Thus the
broad one-to-sixteen growth hypothesis was not reproduced. The modest
eight-to-sixteen increase points to installation, not factorization, as the
next bounded experiment. Three samples and shared-host scheduling do not
support a precise speedup claim. Sparse total fit time also rose at sixteen
workers despite the relatively small preparation change; that cost is not
explained away by this diagnosis.

Inspection found a distinct serial initialization of an N-row observation
scratch bank for each admitted coefficient worker. At N=1M and seventeen
right-hand sides its nominal size rises from 8 MB to 128 MB between one and
sixteen workers. This is a hypothesis for part of installation's growth,
not proof of causality. The next first-touch comparison must report actual
workspace admission, installation time and total preparation time, while
retaining reservation order, capacities and exact scientific outputs.

The snapshot's gated clocks do not take timestamps when phase tracing is
disabled. Its four diagnostic lines separate inclusive and nested intervals;
the initial-generation envelope must not be added to its own subphases.
Hashes (SHA-256): source archive
`f2c31d5d19a6660619c922566835814ece632dbe3c780f2ac350d0108135510c`,
source patch `cedb853403ca72b269fec8384847ccd0f44a0c5470550f2421c78fcac6b5be68`,
Mac plugin `fac5196b442e82d67a98a45316603053c6f8423d5b7980bd4f648cdd36dd9850`,
Linux plugin `ee5b2a8bc6f8a725c82fcaddbf563d806117eaf133e9f61d8dcdca3407ed125f`,
Mac exact-replay summary
`114ae77ae2982b5986449624d0ed8635d06c7366561911adbaaa31160c9d92a6`,
and completed SCC panel summary
`f2d08c8c3c4ea3db77e56a750fd9aff7945eb1de6f8aa08fe6c18574c0e61162`.
The Linux build job 8026545 also ran all eight hardware-FMA witnesses,
including the new ordered upper-triangle tiles.

During witness development, the first core compile required an explicit `f64`
annotation in a test vector; no runtime repair was needed. A standalone plugin
test invocation also omitted the prescribed serial test runner and failed seven
process-global allocator assertions. Repeating it with `--test-threads=1` passed
all 171 active tests (ten isolated audits ignored at this focused stage). The
full repository gate remains responsible for running those audits in separate
processes. These invocation failures are not scientific passes or hidden
tolerance changes. The concurrent-run log hash is
`982f5c138b444975de40f186662c102ef821d9b98b27ec62a2a0d7e96efa9b1f`;
the serial focused-run hash is
`d391c4665088fab438addbb93f84fefa54b0584c62aa491bdf14a2a9ce9b8c5c`.

The new PPML encoding-reuse witnesses also exposed test-fixture defects before
qualification. The exact-plan comparison initially gave only one arm a clone,
which changed its measured spare capacity; both arms now clone identically.
The direct-core allocation witness initially omitted the existing allocator
observation baseline, so the observed peak included the fake SPI map and other
preexisting test owners. Beginning the observation before each fit fixes that
boundary; the required reduction of at least two allocation events per cached
source then passed. Ordinary Rust equality also rejected identical NaNs in
omitted standard errors and z statistics; these fields require IEEE-bit
comparison. None of these repairs changes runtime arithmetic or a tolerance.
The separation comparison also excludes observed method durations, as the
replay contract requires, while comparing every decision and certificate.
The failed core capacity and inference runs have hashes
`1c4e423aa199c5e4e1b6788c600fd4db1cd71dcc0ca94382de40fe9960da93c1`
and `284bd858c6a47abb9869cdc93866fe6632e379ad8ee02fdcef401163279bb305`;
the failed plugin baseline run has hash
`45f1244d946b2d6fc589b6435908bd8468b01c03f7967f7fd8c2fcb010fcee7a`.

### Performance-plan A1: failed frame-adoption experiment, 2026-10-11

A1 did not meet its conditional acceptance criteria and is not in the runtime.
The experiment used source `68efe752` plus fresh canonical provenance, one-use
strict adoption, exact response/weight/scale/generation and canonical-basis
checks, and saved pre-acceptance movement. Both accepted convergence passes,
independent certificates, exact accepted-Hessian validation, objective replay,
eta reconstruction, rollback and all existing tolerances remained required.
This was an unqualified declared-scope experiment, not an accepted ulp-scope
change; SPEC was not amended and no affected-scope exception was installed.

The conservative first variant excluded profiled curves and never attempted
reuse on the target two-FE AKM case. Removing that exclusion while retaining
exact candidate identity exposed two unchanged core failures: the small
DynamicRange Low fixture had eta error `1.89640009941443824e-9` against its
`1e-10` bound, and the disconnected-slope fixture observed one accepted canonical
product where zero was required. Neither bound nor expectation was changed.
The third variant restricted precomputation to the existing admitted fast
categorical-product family; all 1,148 core tests passed (three ignored), including
freshness, response/weight/scale/tolerance invalidation and same-rank basis
rollback witnesses. This family restriction introduced no workload-size rule.

The universal Mac build passed ABI, dependency and actual-plugin load checks.
All 73 independent replay science checks and fixed-worker repeat checks passed,
but exact comparison with B failed in seven PPML fixtures: `ppml-09`, both bias
modes, and the FE/IR/Mu/simplex separation fixtures. The other 66 were exact;
all 38 PPML outer counts and discrete views were exact. No scientific tolerance
was loosened and these differences were not silently exempted. All 67 licensed
fixtures and both OLS and public-scope PPML Break probes passed. The additional
named coefficient/covariance/explicit-dummy qualification panels and quiet-Mac
performance gate were not run after the savings hypothesis failed.

An exploratory, traced Mac one-worker AKM-shaped one-million-row pair cost
`5.400968 -> 5.782367` seconds. Outer iterations stayed five, canonical reduced
calls rose from two to three, and no frame was adopted. The saved movement was
`2.94154580693373191e-8`, above the unchanged `1e-8` bound; accepted eta also
failed exact identity. This oracle-RNG input is distinct from the frozen campaign.

The SCC diagnostic used the frozen campaign
`20261010T192805Z-ols-repair-e29164e6`, four PPML cases, 1/4/16 workers, two
alternating pairs, both phase and solver tracing, a 180-second fit limit and a
3,000-second total budget. All 48 fits completed in 1,331.76 seconds on shared
`scc-ei3` (Xeon Gold 6226R, 16 SGE slots; observed process affinity exposed 32
logical CPUs). Build job 8026507 and panel job 8026509 both had `failed=0` and
`exit_status=0`. These two-pair medians are descriptive Linux observations,
with scheduler and sampling uncertainty; they are not quiet-Mac qualification.

| Frozen PPML case | Workers | B total s | A1 total s | Total cost | B strict s | A1 strict s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| AKM 1M | 1 | 10.35863 | 11.29985 | +9.09% | 2.33377 | 2.34964 |
| AKM 1M | 4 | 4.61770 | 4.89156 | +5.93% | 0.89525 | 0.89400 |
| AKM 1M | 16 | 3.97067 | 3.96606 | -0.12% | 0.57766 | 0.56633 |
| Dense two-FE 1M | 1 | 10.00108 | 10.67410 | +6.73% | 2.34171 | 2.25299 |
| Dense two-FE 1M | 4 | 5.14177 | 5.62802 | +9.46% | 1.30114 | 1.20351 |
| Dense two-FE 1M | 16 | 4.16419 | 4.72266 | +13.41% | 1.05971 | 1.02974 |
| Dense two-FE 4M | 1 | 52.87482 | 56.03334 | +5.97% | 14.09082 | 12.71264 |
| Dense two-FE 4M | 4 | 28.74721 | 32.02216 | +11.39% | 8.94523 | 7.79166 |
| Dense two-FE 4M | 16 | 22.96235 | 29.07915 | +26.64% | 7.04041 | 7.10605 |
| Sparse two-FE 1M | 1 | 40.31158 | 43.49182 | +7.89% | 17.37118 | 8.92324 |
| Sparse two-FE 1M | 4 | 15.46146 | 16.08025 | +4.00% | 6.38076 | 3.63414 |
| Sparse two-FE 1M | 16 | 30.52704 | 25.02154 | -18.03% | 3.75873 | 2.62285 |

Every candidate run had zero adoptions. AKM and both dense cases retained four
outer iterations and 3 ordinary + 2 checkpoint frames; the sparse case retained
nine and 8 + 3 frames. Canonical reduced calls rose 2 -> 3 on AKM/dense and
stayed one on sparse. Actual solves were not saved. AKM strict time did not
fall by a frame's cost. Sparse strict time fell while ordinary projection time
rose; that phase movement alone does not pass A1. The apparent sparse 16-worker
total gain is retained above, alongside all costs, without a qualification claim.
All original-weight and reconstruction certificates passed, fixed-worker repeats
were exact, and sample, rank/status and DoF outputs were exact across artifacts.
Float/solver differences remain unqualified. The SCC record retains RRUNTIME
rows 28-40, solver routes, actual frame counts, trace counts and exact-difference
paths; the table reports the principal acceptance quantities.

Exact eta identity failed in all tested attempted adoptions. Saved movement was
about `2.022e-6` on frozen AKM, `1.070e-8` on dense 1M, `7.032e-10` on dense 4M,
and `3.613e-4` on sparse; even the dense 4M case below the movement bound failed
eta identity. No eligibility condition was relaxed to manufacture adoption.
A separate review found that catching a failed fresh staging solve would need a
cold, recertified recovery path: fallible in-place reprojection can invalidate its
old frame. The experiment propagated that error atomically; such recovery was
not implemented or claimed. A1 was removed before independent C/D work. A2
remains the separate decision in the performance plan.

Johannes requested a compact reproducer when deferring A1. The retained
[unqualified patch](../bench/results/a1-20261011-reproducer.patch) is a 23 KB
experiment fixture against `68efe752`, excluded from every production build.
Its zero-context form reproduces all seven original changed source files exactly.
Apply it only to a disposable source copy. From the repository root, the small
AKM-shaped reproducer is:

```sh
task=target/tmp/a1-reproduction
mkdir -p "$task/source"
git archive 68efe752 | tar -x -C "$task/source"
git apply --unidiff-zero --directory="$task/source" bench/results/a1-20261011-reproducer.patch
(
  cd "$task/source"
  scripts/build_macos.sh
  bench/build_timing_harness.sh ../timing-harness
  cargo run --locked -p fereg-oracle --example replay_large --profile ci -- \
    --output ../akm.case --rows 1000 --levels 200,10 --regressors 16 \
    --weights unit --vce robust --graph akm-campaign --solver auto --seed 739391 \
    --ppml --candidate-all --non-deterministic --no-observation-outputs
  FEREG_PHASE_TIMING=1 FEREG_SOLVER_TRACE=1 ../timing-harness \
    dist/fereg_macos.plugin --replay ../akm.case ../result.json
)
```

The retained small input hash is
`e5f58c14bb9bdc655d8868ba8d0773c404bbc7ac5938f99f8d494d02a2b87306`.
Its fresh ordinary attempt fails exact eta identity and adopts no frame. The
one-million-row form uses `--rows 1000000 --levels 200000,10000`; the frozen SCC
panel still requires the retained campaign inputs, not regeneration. Use the
usual task cleanup after inspecting the reproducer; it is not a shipping option.

Artifact and witness SHA-256 identities:

| Artifact | SHA-256 |
| --- | --- |
| First conservative patch | `0a579e92e22ae412ed97aab1ba22565c3235c9198a6385916d8baecd9b2ec3c1` |
| Failed unrestricted curve patch | `b1bd209358533d7452e1710ff9ef302938b7606403cb5ae0af201ed91083cca6` |
| Failed curve core run | `51fc0bcdc9cac573a09546ffc7975532d3ac2179f0c32fb9e165dd45a07d4d9d` |
| Third candidate patch | `551f3ccc06eaa7a50c7c9926d20fadf95fec6fedfa20d241bf1febeae65d2cd5` |
| Retained zero-context reproducer | `38e61f5c7aa45c34c3620a4c464836324022af67d1226ab47d97bbcd4f8fb874` |
| Third candidate source archive | `a6b17228d2ee9a8421de34ff9bd31b5835e0f0a6d4d2d21cefc515201fca84b8` |
| Third candidate core run | `aebd16ebee27dfe8a1f2033887e5de8f8975d3d4d9d0af5685a77395216e9c69` |
| Third candidate universal Mac plugin | `b75b5165c8a8feda53427783e6733984b690c8b5fa84c4ec1d0bac402200a148` |
| Third candidate Linux plugin | `959b72336d219729e2b00d3a0dfa81b1b9ace92ff9dd39ae9855c7cc451d5601` |
| Exact replay failure/science summary | `6bdc5f5468daa637f54659aedc5e9a560e9dcb818bb34642dae92c144f31f196` |
| Discrete/outer replay check | `8b2f8c7ff84b5ea2b164da6547458c60884e76f48be5f884b34a684fd9d72550` |
| Licensed 67-fixture summary | `9397e27d95a4515abac5be37f451eaeb22436fe690b213a016707403e1becbf3` |
| OLS Break probe | `6e34ae90d9ae35896ca63ce58bd1dd3b893325aa47dcfbe3cf9dfe2f763e3238` |
| PPML public Break probe | `f5aada3ea8df93ebc394b615bb07031d17c06bbe03831bcbf2f1ade2b1a2f3fb` |
| Mac traced pair summary | `aff7895d5fbafeb3b9b5de2154830de7727f67a555d486f75fe94145b4de98ef` |
| SCC 48-fit summary | `7dd95b5aa3d6422050c12733c4bfafcfe28657e7fbdb90b1146978309c843c0d` |

### Performance-plan B: ordered FMA coverage, 2026-10-11

**B qualifies with cached-dispatch source patch `4aa37b80`, Mac plugin
`68c21a90` and Linux plugin `447fe148`.** All twenty Mac bounds pass; all
73-case Mac/Linux replays and 56 full-size Linux fits remain exact. The final
instruction panel has zero library FMA on every measured case. Earlier failed
candidates and their costs are retained below.

The initial candidate `2c6d1832` is arithmetic-preserving, **not an accepted B
workstream**. It extends the existing detected FMA/AVX table from five to twenty entries.
The six areas named in the plan now dispatch ordered dot/projection, joint
iteration, two-FE PCG and canonical upper-Gram loops. Core bodies remain safe
and shared with the portable path; the plugin owns target-feature wrappers.
Per-element arithmetic, reduction order, padding and inactive-RHS behavior are
preserved. The intentionally non-fused reduced-matrix product is unchanged.
There are no tolerance, sample, rank, omission, separation, route, SPI ownership,
certificate or strict-PPML-solve changes. Some fallback/reconstruction,
reusable-certificate and smaller canonical-product loops remain scalar;
profiling must determine whether more coverage is needed.

An independent high-effort Astra review found no correctness issue. Its request
for explicit padding/inactive sentinels and the active `-1.0` sentinel was
implemented. The complexity ratchet records +231 core and +252 plugin production
lines for the shared bodies and typed wrappers, with no new oversized function
or file. The existing joint iteration gains eleven lines from explicit typed
call arguments. Dispatch-call cost is included in the Mac measurements below.

Mac checks pass: `scripts/run_tests.sh`, universal native build/audit/load,
73/73 exact replay cases against the previous plugin (zero differences and
unchanged solver signatures), all 67 licensed full-profile fixtures, and both
OLS and public-scope PPML PTY Break checks. The replay's independent scientific
witnesses pass in all 73 cases. These are working-tree artifact checks bound by
the source-patch and plugin hashes below; the plugin's embedded source label
predates the runtime patch.

The x86 Mac slice compiles all twenty entries. Static disassembly of each of
the fifteen added target-feature bodies contains no library-FMA call; fourteen
contain hardware FMA instructions and PCG direction uses the equivalent
multiply/add form for its unit multiplier. This is instruction-selection
evidence, not an executed instruction count. Rosetta exposes neither FMA nor
AVX here: all five hardware tests skip, despite their test-runner `ok` labels.
SCC access was restored through interactive SSH; the bounded Linux checks below
supersede the earlier authentication blocker. Linux executed all five hardware
tests. Cachegrind found residual library FMA calls, so the initial candidate
fails B's disappearance criterion. B stays unqualified and subsequent
workstreams have not begun. Johannes
conditionally approved A1 on 2026-10-11 under the [plan's eligibility and
validation requirements](fable_performance.md#4-johanness-decisions-and-remaining-gates).
That decision does not waive B qualification. SPEC §13 is unchanged.

The quiet Mac run used the twenty hash-verified frozen workloads, one warmup
and seven alternating pairs per cell, a 90-second process limit and an 1800-second
budget. It completed in 643.59 seconds, with all 320 fits exact and all twenty
paired-median 95% bootstrap upper bounds within max(5%, 1 ms). No builds or
agents ran during timing; ordinary desktop background processes were present.
The largest upper bound is 1.035739. Paired medians range from 0.966264 to
1.009575; costs as well as gains are retained below. These small observations
do not establish a speedup or a release-performance claim. Times are seconds;
ratios are medians of paired ratios, not ratios of the displayed medians.

| Frozen workload | Baseline median | Candidate median | Paired ratio | 95% bootstrap interval |
| --- | ---: | ---: | ---: | --- |
| `ols-no-fe-k5-t1` | 0.005282 | 0.005280 | 0.999621 | [0.991737, 1.005359] |
| `ols-no-fe-k5-t4` | 0.004686 | 0.004704 | 1.001930 | [0.999575, 1.007042] |
| `ols-three-unit-k5-t1` | 0.585763 | 0.585808 | 1.000077 | [0.996792, 1.001798] |
| `ols-three-unit-k5-t4` | 0.299557 | 0.299698 | 1.000087 | [0.997239, 1.001967] |
| `ols-two-pw-k5-t1` | 0.653873 | 0.656214 | 1.000259 | [0.997755, 1.011564] |
| `ols-two-pw-k5-t4` | 0.372277 | 0.372096 | 1.001750 | [0.989284, 1.005846] |
| `ols-two-unit-k20-t1` | 1.203934 | 1.205990 | 1.005488 | [0.994109, 1.010733] |
| `ols-two-unit-k20-t16` | 0.445681 | 0.442693 | 0.999367 | [0.987844, 1.008383] |
| `ols-two-unit-k20-t4` | 0.574316 | 0.567609 | 0.990337 | [0.982508, 0.999854] |
| `ols-two-unit-k20-t8` | 0.471380 | 0.474140 | 0.997908 | [0.978926, 1.034033] |
| `ols-two-unit-k5-t1` | 0.478811 | 0.480858 | 1.001146 | [0.987009, 1.004367] |
| `ols-two-unit-k5-t4` | 0.263885 | 0.263871 | 0.995747 | [0.994644, 1.005684] |
| `ols-wide-k129-t1` | 0.763857 | 0.752693 | 0.974630 | [0.970251, 1.005925] |
| `ols-wide-k129-t4` | 0.398176 | 0.401504 | 0.990082 | [0.965212, 1.035739] |
| `ols-wide-k64-t1` | 0.295673 | 0.295657 | 0.993140 | [0.988308, 1.023617] |
| `ols-wide-k64-t4` | 0.168455 | 0.163700 | 0.966264 | [0.959143, 1.002210] |
| `ols-wide-k65-t1` | 0.312483 | 0.311014 | 0.996615 | [0.978531, 1.011322] |
| `ols-wide-k65-t4` | 0.172959 | 0.176653 | 1.009575 | [0.957675, 1.029921] |
| `ppml-two-unit-k5-t1` | 0.405354 | 0.404991 | 1.000059 | [0.993464, 1.004761] |
| `ppml-two-unit-k5-t4` | 0.229143 | 0.228294 | 1.001377 | [0.991459, 1.003422] |

A separate instrumented Mac diagnostic used locally generated campaign-shaped
AKM inputs: one million rows, 16 controls, 200,000 worker and 10,000 firm
levels, seed 739391, one worker, OLS IID and PPML robust inference, unrestricted
candidate, nondeterministic worker policy, and only the required sample output.
These inputs are not the frozen SCC campaign inputs and cannot substitute for
them. Two alternating pairs per model, with a 90-second fit limit and 600-second
budget, give eight successful fits with exact science and solver signatures.
PPML has four outer iterations in every fit and keeps its fresh strict solve.
The following means are descriptive; no timing acceptance is inferred from
two pairs with tracing enabled. Unobserved phases stay missing.

| Diagnostic runtime field | Baseline mean | Candidate mean |
| --- | ---: | ---: |
| OLS harness native call | 0.884388 | 0.885726 |
| OLS preflight (row 18) | 0.000202 | 0.000200 |
| OLS input (row 19) | 0.133730 | 0.134039 |
| OLS native (row 20) | 0.748780 | 0.749800 |
| OLS total (row 21) | 0.882712 | 0.884039 |
| PPML harness native call | 4.773539 | 4.703071 |
| PPML total (row 1) | 4.771736 | 4.701227 |
| PPML preparation (row 28) | 0.289071 | 0.288944 |
| PPML separation FE (row 29) | 0.004087 | 0.004130 |
| PPML separation simplex (row 30) | missing | missing |
| PPML separation IR (row 31) | missing | missing |
| PPML separation mu (row 32) | missing | missing |
| PPML weight/response refresh (row 33) | 0.060927 | 0.061665 |
| PPML FE projection (row 34) | 1.963144 | 1.927000 |
| PPML finite solve (row 35) | 0.560771 | 0.560114 |
| PPML objective/line search (row 36) | 0.279796 | 0.280245 |
| PPML strict final solve (row 37) | 1.165864 | 1.133371 |
| PPML inference (row 38) | 0.085691 | 0.083245 |
| PPML FE recovery (row 39) | 0.000001 | 0.000001 |
| PPML publication (row 40) | 0.121995 | 0.122015 |

Failed attempts: initial Clippy checks rejected complex function-pointer types
and a macro placed after tests; named types and placement fixed both. The plan's
literal `output_sample=0` recipe failed before estimation because both wire
parsers require a positive sample output. The corrected case keeps sample
column 21 and disables only optional outputs; the plan recipe is corrected.
A generator retry refused to overwrite its existing template; reusing that
unchanged template resolved setup. These failures do not count as successful
scientific or timing observations. No estimator repair or tolerance change was
made in response.

SHA-256 records (raw task captures are disposable; these summaries and hashes
are the retained record). The source patch is relative to `38fd8296` and includes
the new ordered-kernel file and complexity inventory. Its production code is
identical to the timed build; only the later sentinel test assertions differ.

| Artifact / observation | SHA-256 |
| --- | --- |
| Previous universal plugin | `f59d2a2989dc672045fdda86dd88fa2a4191ab0e15a42d833f7bd0711c4d8ca4` |
| Candidate universal plugin | `b04e08a2a085e249f8f72eb6df8c6979e431a7062d210864c48e5e79c205da75` |
| Runtime source patch | `15f29992707e954323d93e8ea86b9e9c4f63357b4114dfb382fafa38470b41e2` |
| Full gate | `439bafa7ff5bba99524e15021d19ed2bdcc7320f2e6dece573719f9c81a3dbf5` |
| Exact replay summary | `519c063ef73872ae8b27d9a8a1d639e900ccf12aca0a42efa8e42c01797c1760` |
| Licensed fixture summary | `9397e27d95a4515abac5be37f451eaeb22436fe690b213a016707403e1becbf3` |
| OLS Break | `6e34ae90d9ae35896ca63ce58bd1dd3b893325aa47dcfbe3cf9dfe2f763e3238` |
| PPML Break | `f5aada3ea8df93ebc394b615bb07031d17c06bbe03831bcbf2f1ade2b1a2f3fb` |
| Mac timing summary | `43286b38795729eff6a9005a4d0e6c3c5a11e411a95eb5301928f0640e131bad` |
| Local phase input manifest | `42e783208ab7e61d84bacf38afa0d3e3d0ad734b27be9dc41f80a51767497322` |
| Rejected zero-sample-output probe | `bea85baec7aa5e5a97d1535d63173a4e745da91cd675af6399a47f39792c207c` |
| Successful phase diagnostic | `c106aa856c2b2f3c6459ceed8ec6cacbb03339379292923940f996ed0d003fb9` |
| Static x86 instruction summary | `19a6e94524b6afb5195ca9da9a482bed30ada4f2f80bbea9ee247baebdcad4d9` |
| x86 tests (hardware skipped) | `a340e4da6363a29dddcbea72ebbb1decb133b3c7239c4e7238a5201ad7d93552` |
| Candidate build/audit/load log | `b14b52bccc47b8986c2e2deeb05c48c34e3c653b758332b5e9c8779e199bc96e` |
| Local OLS AKM case | `6503640e23c0955ba2b711b02f82917085503ccd16f2c58011bc64b74f04fd3e` |
| Local PPML AKM case | `029fae2a6399992230044ff0847d11cdac4e05758704c3d6eef17538c8c1404e` |

### B Linux observations and extension, 2026-10-11

The initial twenty-entry candidate was built from `eb838c85` (production equal
to `2c6d1832`) against `38fd8296`, with Rust 1.85.1, GCC 12.2 and glibc 2.28.
SCC job 8023715 completed release/profiling build, binary audit/load, numeric
buffer guards, all five actual FMA/AVX hardware tests, and 73/73 exact replay
cases with unchanged solver signatures and independent witnesses.

Inputs are binary64 extractions from the seven frozen campaign DTAs in the
retained `20261010T192805Z-ols-repair-e29164e6` campaign, with hashes checked
before and after use; they were not regenerated. Calls retain the required
sample output, unrestricted candidates and campaign solver/separation policy.
The following native means use two alternating pairs, phase/solver tracing,
1/4 workers and 120-second per-fit/1500-second total limits. All 56 fits are
exact across plugins and repeat runs at each fixed worker count, with both
certificates passing. Dense/AKM PPML keeps four outer iterations; sparse PPML
keeps nine, two strict cycles and one refinement. This does not assert
cross-worker bit identity. All OLS rows 18–21 and PPML rows 1, 28–40 were
captured, preserving missing phases.

Job 8023727 completed in 1245.23 measurement seconds on Xeon Gold 6242
(scc-gr4). The requested four-CPU binding did not constrain the reported
32-CPU affinity. The node was shared and tracing was enabled; these are
phase diagnostics, not quiet timing or a Linux speedup claim. Strict-phase
means are PPML row 37. Both costs and gains are shown.

| Frozen case | Workers | Base native s | Candidate native s | Base strict s | Candidate strict s |
| --- | ---: | ---: | ---: | ---: | ---: |
| ols-akm-1m | 1 | 2.069848 | 1.982841 | — | — |
| ols-akm-1m | 4 | 1.096788 | 1.093844 | — | — |
| ppml-akm-1m | 1 | 11.185748 | 10.736614 | 2.718806 | 2.549752 |
| ppml-akm-1m | 4 | 5.519300 | 5.254834 | 1.119544 | 1.052400 |
| ppml-dense-1m | 1 | 10.631114 | 9.862782 | 2.567979 | 2.245452 |
| ppml-dense-1m | 4 | 5.580860 | 5.432845 | 1.405876 | 1.312754 |
| ols-three-dense-1m | 1 | 3.239322 | 3.245102 | — | — |
| ols-three-dense-1m | 4 | 1.633578 | 1.652328 | — | — |
| ppml-sparse-1m | 1 | 42.871195 | 39.525806 | 19.554498 | 16.715510 |
| ppml-sparse-1m | 4 | 21.045159 | 18.929957 | 7.646044 | 7.244020 |
| ppml-dense-4m | 1 | 45.032531 | 42.844466 | 11.922271 | 10.963437 |
| ppml-dense-4m | 4 | 25.499336 | 24.711165 | 7.301107 | 6.964740 |
| ols-three-dense-4m | 1 | 17.688177 | 17.553592 | — | — |
| ols-three-dense-4m | 4 | 7.487824 | 7.605072 | — | — |

Cachegrind 3.22 job 8023764 measured only the substantive native call, using
profiling builds, one worker and the first 50,000 rows of four frozen inputs.
Two alternating pairs per case completed in 980.09 seconds under the declared
600-second per-fit/2700-second total bounds. All sixteen fits were exact.
Hardware tests also ran under Valgrind without skipping. Cache and branch
simulation were disabled. Complete symbol totals, rather than top-function
absence, determine library FMA counts. Matching signed AlmaLinux glibc debug
symbols identify `__fma_fma3`; debug and loaded-library build IDs were checked.

| Frozen prefix | Base instructions | Candidate instructions | Change | Base library FMA instructions | Candidate library FMA instructions |
| --- | ---: | ---: | ---: | ---: | ---: |
| ppml-dense-1m | 4,167,300,758.5 | 3,928,120,662.0 | -5.7394% | 175,977,738 | 60,566,754 |
| ols-three-dense-1m | 1,368,018,067.0 | 1,368,017,711.5 | -0.0000% | 4,814,409 | 4,814,409 |
| ppml-sparse-1m | 11,443,414,520.0 | 8,679,314,606.5 | -24.1545% | 930,283,374 | 135,790,188 |
| ppml-akm-1m | 66,683,371,106.0 | 43,995,700,951.5 | -34.0230% | 5,855,481,021 | 1,010,663,925 |

Between-pair instruction differences were at most 0.0006%; the three-FE
change is noise. Valgrind wall times are not performance evidence. A separate
Callgrind caller diagnostic (8023768) found remaining work in group basis
operators, retained residual reconstruction, numeric coefficient updates,
canonical cross products, certificate folds and triangular solves. It was
whole-process caller attribution, not the native instruction measurement.

Failed measurement attempts remain nonpassing: 8023728 exhausted its 180-second
first-fit bound on the AKM prefix; the completed run used the larger declared
bound. Probe 8023745 had exact fits but missing libm symbols, so its unresolved
counts cannot support FMA attribution. Job 8023755 failed the attribution
guard after an incorrect debug-symbol prefix; correcting the task-local lookup
and verifying the build ID enabled 8023764. No scientific tolerance or solver
policy changed in response.

The follow-up extends twenty entries to thirty-three, targeting those observed
callers. It keeps every per-element operation and validation order, allocations,
rank/sample/omission/separation decisions, worker policy, independent
certificates and fresh strict PPML solve. Small certificate folds retain their
interleaved checks and use a scalar detected entry; its cost must be measured.
The first extension passed 1,146 active core tests. The subsequent certificate
extraction failed two existing tests by applying zero-width chunks to an empty
group; restoring the original empty-group behavior and adding explicit coverage
made all 1,146 active tests pass (three ignored). Two independent high-effort
Astra reviews found no remaining correctness issue, while stressing parent
replay and measured dispatch cost. The seven x86 Mac tests compile but skip
hardware execution under Rosetta. The current full `scripts/run_tests.sh` gate
passes, including allocator audits, transaction mocks and independent numerical
witnesses. The ratchet adds 288 core and 238 plugin production lines for the
thirteen bodies and wrappers, with no additional oversized file/function.
A Mac comparator build launched from the wrong working directory was stopped
before producing comparison evidence and restarted inside the baseline checkout.
The thirty-three-entry candidate passed the Mac universal build/audit/load,
73/73 exact replay cases with unchanged solver signatures, all 67 licensed
Stata fixtures, and both public-scope OLS/PPML Break checks. The fixture runner
first rejected a noncanonical plugin filename before running Stata; copying the
identical artifact to its required basename resolved that tooling error.
SCC build job 8023807 passed the native audit/load, all seven executed FMA/AVX
tests and 73/73 exact replays with independent witnesses.

This candidate **does not qualify**. The standard Mac panel completed all 320
fits exactly in 644.00 seconds (one warmup, seven alternating pairs per case,
90-second per-fit and 1800-second total bounds). Nineteen timing bounds passed.
The K=129, four-worker OLS upper bootstrap bound was 1.051078661 against 1.05,
although its paired median ratio was 0.977195353. This is a nonpassing bound,
not evidence of a demonstrated regression or permission to relax the allowance.
No agents, builds or tests competed during the run; ambient desktop processes
were present. The following table retains costs as well as gains.

| Standard case | Base median s | Candidate median s | Paired median ratio | 95% interval | Bound passes |
| --- | ---: | ---: | ---: | --- | --- |
| ols-no-fe-k5-t1 | 0.005275 | 0.005261 | 1.002286 | [0.968808, 1.009217] | yes |
| ols-no-fe-k5-t4 | 0.004616 | 0.004725 | 1.018100 | [1.011482, 1.066769] | yes |
| ols-three-unit-k5-t1 | 0.585446 | 0.585340 | 1.000311 | [0.996862, 1.000776] | yes |
| ols-three-unit-k5-t4 | 0.301226 | 0.301576 | 1.001162 | [0.998409, 1.012663] | yes |
| ols-two-pw-k5-t1 | 0.654002 | 0.656754 | 1.000448 | [0.994806, 1.004548] | yes |
| ols-two-pw-k5-t4 | 0.374342 | 0.374774 | 0.995349 | [0.980985, 1.013267] | yes |
| ols-two-unit-k20-t1 | 1.192590 | 1.192593 | 0.996402 | [0.992877, 0.999713] | yes |
| ols-two-unit-k20-t16 | 0.444602 | 0.443497 | 0.994737 | [0.993586, 0.999665] | yes |
| ols-two-unit-k20-t4 | 0.569982 | 0.576994 | 1.014324 | [1.003129, 1.024588] | yes |
| ols-two-unit-k20-t8 | 0.468603 | 0.474596 | 1.015585 | [0.984932, 1.025868] | yes |
| ols-two-unit-k5-t1 | 0.481379 | 0.477421 | 0.992746 | [0.978231, 1.001479] | yes |
| ols-two-unit-k5-t4 | 0.264745 | 0.264925 | 1.004851 | [0.992034, 1.014969] | yes |
| ols-wide-k129-t1 | 0.754905 | 0.757999 | 1.015816 | [0.977636, 1.021017] | yes |
| ols-wide-k129-t4 | 0.400861 | 0.398980 | 0.977195 | [0.949603, 1.051079] | **no** |
| ols-wide-k64-t1 | 0.302252 | 0.297055 | 0.981568 | [0.966294, 0.995895] | yes |
| ols-wide-k64-t4 | 0.169353 | 0.167513 | 0.985753 | [0.973005, 1.015971] | yes |
| ols-wide-k65-t1 | 0.315579 | 0.314946 | 1.000732 | [0.989827, 1.007048] | yes |
| ols-wide-k65-t4 | 0.172801 | 0.175230 | 1.020985 | [1.012910, 1.048891] | yes |
| ppml-two-unit-k5-t1 | 0.407041 | 0.402097 | 0.987854 | [0.982752, 0.992735] | yes |
| ppml-two-unit-k5-t4 | 0.229931 | 0.229954 | 0.994263 | [0.991941, 1.001507] | yes |

Linux native-call instruction job 8024253 completed sixteen exact fits in
980.55 seconds with the same frozen prefixes, limits and attribution rules.
Library FMA remained on every case, so the literal disappearance criterion
also failed. These instruction changes do not establish quiet wall-time gains.

| Frozen prefix | Base instructions | Candidate instructions | Change | Candidate library FMA instructions |
| --- | ---: | ---: | ---: | ---: |
| ppml-dense-1m | 4,167,357,006.0 | 3,728,351,548.0 | -10.5344% | 14,400,006 |
| ols-three-dense-1m | 1,368,016,815.5 | 1,350,884,057.0 | -1.2524% | 15 |
| ppml-sparse-1m | 11,443,432,959.5 | 8,404,214,660.0 | -26.5586% | 38,251,782 |
| ppml-akm-1m | 66,683,364,069.5 | 43,721,041,900.0 | -34.4349% | 913,804,857 |

The corresponding 56-fit Linux native panel (8024271) completed in
1222.12 seconds, with all fixed-worker science and signatures exact.
All certificates pass; dense/AKM PPML retains four outer iterations and sparse
PPML retains nine. Host scc-gd4 is shared; tracing remains enabled,
so these phase observations establish neither Mac qualification nor a Linux
speedup claim. Means include both alternating pairs.

| Frozen case | Workers | Base native s | Candidate native s | Base strict s | Candidate strict s |
| --- | ---: | ---: | ---: | ---: | ---: |
| ols-akm-1m | 1 | 2.054525 | 1.929889 | — | — |
| ols-akm-1m | 4 | 1.075328 | 1.087040 | — | — |
| ppml-akm-1m | 1 | 11.134865 | 10.017541 | 2.722945 | 2.366631 |
| ppml-akm-1m | 4 | 5.438297 | 5.021609 | 1.104971 | 0.996896 |
| ppml-dense-1m | 1 | 10.304446 | 9.244334 | 2.431706 | 2.082191 |
| ppml-dense-1m | 4 | 5.540954 | 5.258065 | 1.346294 | 1.278112 |
| ols-three-dense-1m | 1 | 3.221288 | 3.169217 | — | — |
| ols-three-dense-1m | 4 | 1.629021 | 1.593238 | — | — |
| ppml-sparse-1m | 1 | 42.801137 | 37.569458 | 19.502466 | 16.243620 |
| ppml-sparse-1m | 4 | 20.854550 | 18.088248 | 7.732623 | 7.137208 |
| ppml-dense-4m | 1 | 44.647278 | 39.293597 | 11.869542 | 9.865655 |
| ppml-dense-4m | 4 | 25.357428 | 23.529659 | 7.290157 | 6.639839 |
| ols-three-dense-4m | 1 | 17.555877 | 17.354827 | — | — |
| ols-three-dense-4m | 4 | 7.444361 | 7.180352 | — | — |

The dense caller probe (8024275) attributes 4,800,000 remaining calls to the
row-major joint residual/score loops and two to the final score certificate.
The next candidate adds detected entries for those ordered row loops and uses
the existing scalar entry for the certificate. Its 1,146 active core tests and
Clippy checks pass; this source is not yet qualified. The AKM caller diagnostic
(8024807) completed in 371.80 seconds within its 540-second bound. It accounts
for all 304,601,619 remaining library calls: 152,783,966 in incidence center/
scatter, 139,243,955 in reduced CG updates, 12,573,696 in the joint row loops
and two in the final score certificate. Whole-process Callgrind is caller
attribution only; native-call Cachegrind remains the instruction acceptance test.

The resulting thirty-seven-entry candidate adds ordered incidence and CG
step/energy bodies and reuses the existing affine direction entry. Original
nonfused curvature, initial energy and speculative matrix products remain
unchanged. Scales, iteration limits, tolerances, finite checks, allocations,
solver routes and scientific decisions are unchanged. Independent dense
elimination and known-solution CG witnesses remain in the core suite. All
1,146 active core tests and Clippy pass; the independent high-effort review
found no issue. Hardware tests cover empty and repeated incidence coordinates,
CG tails, subnormals and signed zero. The final ratchet adds 366 core and 338
plugin production lines over `ded7f627`, without another oversized file or
function. The full repository gate passes, including allocator audits, transaction mocks,
portable checks and independent numerical witnesses. The Mac universal build,
ABI audit/load, 73/73 exact replays with unchanged signatures, all 67 licensed
fixtures, and OLS/PPML Break checks pass. Linux instruction and hardware
qualification and the final Mac timing panel remain pending. The final Mac
panel is predeclared at fifteen alternating pairs for every standard workload,
one warmup, 90 seconds per fit and 1800 seconds total, with the same
max(5%, 1 ms) allowance. This does not retroactively accept the failed
thirty-three-entry panel.


The thirty-seven-entry Linux build (8025080) passed native release/profiling
load/audit, all eight hardware tests and 73 exact replay cases. The initial
measurement launches did not run fits: native job 8025103 omitted its required
setup binding/mode, and instruction job 8025102 stopped at the hardware guard.
Under Valgrind 3.22, the normalized selected-score signed-zero test returned
positive zero where native hardware preserved negative zero. No assertion was
relaxed and no production repair was made for this instrumentation failure.

Independent C probe 8025110 calls libm FMA, scalar FMA and packed FMA on four
lanes: `a=[1,1,-1,-1]`, `b=[-0,+0,+0,-0]`, `c=[-0,-0,-0,+0]`. All native
implementations return the IEEE results `[-0,+0,-0,+0]`; all three under
Valgrind 3.22 return `[+0,+0,+0,+0]`. Upstream's
[release notes](https://valgrind.org/docs/manual/dist.news.html) document AMD64
FMA-emulation accuracy improvements in 3.23. The older instruction/caller
observations above remain diagnostic and nonqualifying.

A task-local build of the official Valgrind 3.27.1 release is bounded at four
cores, 8 GB and twenty minutes, with 180/780/120-second configure/build/install
limits. Its source archive matches the published MD5/SHA-1 and the SHA-256
record below. The first queued build (8025117) was canceled before execution
to correct the probe source path; 8025123 is the corrected build. Acceptance
requires all eight unchanged hardware tests and the independent signed-zero
probe to pass under the new tool. The ensuing instruction panel also compares
every instrumented result with a native run of both plugins on that exact
prefix. Corrected native phase job 8025111 retains the original fit/time bounds.


Valgrind 3.27.1 build 8025123 passed all eight unchanged hardware tests and the
four-lane signed-zero probe. Native-call instruction panel 8025135 completed
sixteen fits in 578.55 seconds. Each of the eight native prefix runs
matched every corresponding instrumented result exactly, including solver
signatures and observation-output hashes. The complete function totals were:

| Frozen prefix | Base instructions | Candidate instructions | Change | Candidate library FMA instructions |
| --- | ---: | ---: | ---: | ---: |
| ppml-dense-1m | 4,167,331,794.5 | 3,693,815,352.0 | -11.3626% | 0 |
| ols-three-dense-1m | 1,368,017,720.0 | 1,350,883,636.0 | -1.2525% | 9 |
| ppml-sparse-1m | 11,443,434,202.0 | 8,312,554,042.0 | -27.3596% | 0 |
| ppml-akm-1m | 66,683,392,949.0 | 41,747,983,075.5 | -37.3937% | 0 |

This still did not complete coverage: the three-FE OLS row retained three
scalar library calls (nine instructions). Caller probe 8025143 completed in
23.86 seconds and located two in `weighted_sum_of_squares` and one in
`certify_statistic_identities`. The Mac fifteen-pair panel was deliberately
interrupted after 251 exact guarded fits and seven completed passing workload
bounds, before changing these sites. That interrupted panel is not a timing
pass. Its records and hash are retained separately from the prior failed
seven-pair bound.

The completed-coverage source keeps the thirty-seven-entry table and dispatches
the two remaining scalar source expressions through its existing FMA entry.
The norm square is called twice; the RMSE square is called once on this case.
Operands, positive-zero addends, evaluation order and interleaved checks are
unchanged. The independent statistic-identity witnesses and full repository
gate pass. The ratchet now adds 368 core and 338 plugin production lines over
`ded7f627`, with no additional oversized file/function. Final native builds passed their ABI/load checks. Mac replay remained exact
on all 73 cases; all 67 licensed fixtures and OLS/PPML Break checks passed.
Linux build job 8025154 passed all eight executed hardware witnesses and all
73 replay cases. Valgrind 3.27.1 job 8025206 completed sixteen fits in
622.41 seconds, with eight native preflights agreeing exactly with the
instrumented runs. Library-FMA instruction totals are now zero in all eight
candidate fits. This completes measured FMA coverage, not B qualification.

| Frozen prefix | Base instructions, median | Candidate instructions, median | Change | Candidate library FMA instructions |
| --- | ---: | ---: | ---: | ---: |
| ppml-dense-1m-50k | 4,167,361,710.5 | 3,693,852,366.5 | -11.36% | 0 |
| ols-three-dense-1m-50k | 1,368,018,900.0 | 1,350,884,951.5 | -1.25% | 0 |
| ppml-sparse-1m-50k | 11,443,432,587.5 | 8,312,522,589.0 | -27.36% | 0 |
| ppml-akm-1m-50k | 66,683,391,134.0 | 41,748,005,545.5 | -37.39% | 0 |

The largest within-arm repeat spread is 32,960 instructions. These are
bounded single-worker Linux instruction observations, not Mac timing gains.
Native Linux job 8025207 hit its declared 1500-second total budget after
47 of 56 successful guarded fits. Every completed comparison was exact;
the interrupted four-million-row dense PPML/four-worker fit and the remaining
fits are unavailable. Its slower shared host does not establish a wall-time
regression or a pass. The earlier complete 56-fit panel remains attributed to
its earlier artifact.

The final Mac panel also **does not qualify**. All 640 fits (twenty workloads,
one warmup and fifteen alternating pairs) were exact in 1281.42 seconds, but
K=129/four-worker OLS again failed the unchanged bound: baseline/candidate
medians 0.395245/0.398175 seconds, paired median ratio 1.010543, 95% interval
[0.992008, 1.057668], limit 1.05. The other nineteen bounds passed. Costs remain
visible: K=20/eight-worker OLS had paired median ratio 1.016910 (upper 1.022419),
and four-worker PPML 1.016870 (upper 1.019245). The failed bound is retained;
it is not replaced by an unchanged-artifact rerun.

A bounded independent review identified repeated `OnceLock` table reads in
scalar certificate loops as a concrete overhead hypothesis. For the wide
case, roughly 130 RHS across 5,500 FE levels can cause about 715,000 table reads
per complete certificate. The next arithmetic-preserving candidate caches the
table outside certificate/coupling folds and residual row traversal. Public
installation semantics, operands, check positions, reduction order, solver
policy and tolerances remain unchanged. This is a source-level hypothesis;
the timing variation alone does not establish causality. The complexity
ratchet adds 391 core and 338 plugin production lines over `ded7f627`, including
the previous coverage extension. New qualification is required.

The cached-dispatch candidate passes the full repository gate, universal Mac
ABI/load, 73/73 exact Mac replays, all 67 licensed fixtures and both public
Break checks. Bounded independent review found no changed operands, check
positions, reduction order, public installation semantics or unsafe code in
the lookup-hoisting delta. Linux build job 8025383 passed native audit/load,
all eight hardware witnesses and 73/73 exact replays.

Mac qualification used the same twenty frozen workloads, one warmup and fifteen
alternating pairs under a predeclared 1800-second total/90-second per-fit bound.
All 640 fits were exact and all bounds passed in 1283.54 seconds. No local
builds or agents ran during timing. The Mac14,14 host had 24 logical CPUs;
load averages at start/end were 1.71/2.31 (one minute), with no thermal or
performance warning. Activity Monitor and ordinary desktop processes remained
open; their observed load and the failed earlier panels are retained. These
results establish the workload nonregression check, not a release-speed claim
or proof that lookup overhead alone caused the previous failures.

| Frozen Mac workload | Base median s | Candidate median s | Paired median ratio | 95% upper ratio bound | Pass |
| --- | ---: | ---: | ---: | ---: | :---: |
| ols-no-fe-k5-t1 | 0.005265 | 0.005269 | 1.001899 | 1.010703 | yes |
| ols-no-fe-k5-t4 | 0.004673 | 0.004662 | 0.998091 | 1.014960 | yes |
| ols-three-unit-k5-t1 | 0.586072 | 0.586364 | 1.000768 | 1.007410 | yes |
| ols-three-unit-k5-t4 | 0.300342 | 0.301070 | 1.001049 | 1.002883 | yes |
| ols-two-pw-k5-t1 | 0.655622 | 0.656263 | 1.000818 | 1.003743 | yes |
| ols-two-pw-k5-t4 | 0.373327 | 0.372299 | 0.997883 | 1.004250 | yes |
| ols-two-unit-k20-t1 | 1.192626 | 1.190379 | 0.997721 | 1.001369 | yes |
| ols-two-unit-k20-t16 | 0.444991 | 0.443226 | 1.000321 | 1.003870 | yes |
| ols-two-unit-k20-t4 | 0.566583 | 0.572692 | 1.008211 | 1.016746 | yes |
| ols-two-unit-k20-t8 | 0.471166 | 0.469543 | 0.995826 | 1.009364 | yes |
| ols-two-unit-k5-t1 | 0.487497 | 0.490294 | 1.007776 | 1.013533 | yes |
| ols-two-unit-k5-t4 | 0.264627 | 0.264720 | 1.001893 | 1.009534 | yes |
| ols-wide-k129-t1 | 0.762863 | 0.752804 | 0.988493 | 1.020497 | yes |
| ols-wide-k129-t4 | 0.384350 | 0.391891 | 1.006384 | 1.017147 | yes |
| ols-wide-k64-t1 | 0.298219 | 0.297289 | 1.005429 | 1.012541 | yes |
| ols-wide-k64-t4 | 0.166151 | 0.166675 | 1.012796 | 1.022620 | yes |
| ols-wide-k65-t1 | 0.313229 | 0.311255 | 0.991602 | 0.997856 | yes |
| ols-wide-k65-t4 | 0.172737 | 0.174030 | 1.006231 | 1.028476 | yes |
| ppml-two-unit-k5-t1 | 0.405339 | 0.405013 | 0.999573 | 1.001791 | yes |
| ppml-two-unit-k5-t4 | 0.229393 | 0.234549 | 1.019294 | 1.027793 | yes |

Costs as well as gains remain in that table: four-worker PPML has a 1.93%
paired median increase; K=64/four-worker OLS has a 1.28% increase. Both are
within the unchanged allowance. The former K=129/four-worker failure now
has interval [0.954403, 1.017147], with paired median ratio 1.006384.

Linux Valgrind 3.27.1 job 8025456 completed sixteen instrumented fits in
581.92 seconds. Eight native preflights matched the instrumented science
exactly; every paired/repeated result and solver signature was exact. All
candidate library-FMA instruction totals are zero. Native hardware tests and
the independent signed-zero witness retain the corrected instrumentation
prerequisite; no rounding assertion or tolerance was relaxed.

| Frozen prefix | Base instructions, median | Candidate instructions, median | Change | Candidate library FMA instructions |
| --- | ---: | ---: | ---: | ---: |
| ppml-dense-1m-50k | 4,167,348,700.5 | 3,694,004,287.5 | -11.36% | 0 |
| ols-three-dense-1m-50k | 1,368,018,265.5 | 1,350,338,462.0 | -1.29% | 0 |
| ppml-sparse-1m-50k | 11,443,407,213.0 | 8,313,466,961.0 | -27.35% | 0 |
| ppml-akm-1m-50k | 66,683,350,571.5 | 41,749,824,121.0 | -37.39% | 0 |

The largest within-arm repeat spread is 70,273 instructions.
The separate full-size Linux native panel (8025457) completed all 56 fits
in 1890.32 seconds under the predeclared 2400-second total and
120-second per-fit limits. All completed base/candidate pairs and repetitions
have exact science and solver signatures; all certificates pass. Dense/AKM
PPML retains four outer iterations and sparse PPML nine. Build and both
measurement jobs have scheduler failed=0/exit=0. The shared Xeon host and
phase/solver instrumentation make these descriptive cost observations, not
quiet Linux or Mac timing qualification. Means include both alternating pairs.

| Frozen case | Workers | Base native s | Candidate native s | Change | Base strict s | Candidate strict s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| ols-akm-1m | 1 | 3.641185 | 3.556964 | -2.31% | — | — |
| ols-akm-1m | 4 | 2.230758 | 2.214248 | -0.74% | — | — |
| ppml-akm-1m | 1 | 18.879920 | 18.073212 | -4.27% | 4.227770 | 3.709725 |
| ppml-akm-1m | 4 | 9.928838 | 9.073932 | -8.61% | 1.950056 | 1.613718 |
| ppml-dense-1m | 1 | 19.015679 | 19.548936 | +2.80% | 4.190574 | 4.817517 |
| ppml-dense-1m | 4 | 11.544038 | 10.518159 | -8.89% | 2.537457 | 2.472142 |
| ols-three-dense-1m | 1 | 4.973539 | 4.942548 | -0.62% | — | — |
| ols-three-dense-1m | 4 | 2.891701 | 2.902296 | +0.37% | — | — |
| ppml-sparse-1m | 1 | 62.953963 | 60.134788 | -4.48% | 25.867078 | 25.064503 |
| ppml-sparse-1m | 4 | 33.993516 | 28.996312 | -14.70% | 13.161102 | 10.548958 |
| ppml-dense-4m | 1 | 74.896901 | 72.574617 | -3.10% | 18.288112 | 17.400576 |
| ppml-dense-4m | 4 | 45.419287 | 41.870218 | -7.81% | 11.649183 | 10.912688 |
| ols-three-dense-4m | 1 | 22.388316 | 22.218360 | -0.76% | — | — |
| ols-three-dense-4m | 4 | 12.118458 | 12.675189 | +4.59% | — | — |

B is qualified at this artifact: zero exact replay differences and unchanged
solver signatures, independent numerical witnesses, zero measured library-FMA
instructions, and all standard Mac nonregression bounds passing. No tolerance,
sample, rank, omission, separation, solver-family policy or SPI ownership
changed. The fresh strict PPML solve and independent certificates remain.


Retained SHA-256 records:

| Artifact / observation | SHA-256 |
| --- | --- |
| Linux baseline release | `166a23b18ab00a8f1929c0e44b6f57f5febb0e577d15229284b9cd2320d3807c` |
| Linux initial candidate release | `8fb51f4669f8e4f4c1d0d992bbb37bcd1ea8bb174177e50ac3de799d341b3cb9` |
| Linux baseline profile | `3fc36476c55de77facfcf43d228370b89392b3333a6bc85f36e976973dca6c48` |
| Linux initial candidate profile | `e6942e88afb6b22454488dcd262df5df25d9c0d54a83af812648805441605492` |
| Initial Linux hardware tests | `5803d7860cd60719bd08fc809035fb495bfbe9aeb422d9c01a386adc813822d4` |
| Initial Linux replay log | `b48cf722ac01c4a37543b299c0b6b281861d0ffc931ffd2e662e66b7946e1be4` |
| Loaded libm | `eff3c73980ea0c10daeeb029e998500d1ba643a9a8ad49a43f8978b4e013312a` |
| Matching libm debug symbols | `c718009c8ff3f040e0a941f17212fd87db079824205435a38d9b7e4263941ed9` |
| Follow-up source patch against ded7f627 | `980d5a34a0f5fb0d4e096f07aabaf2f084fc0e9fd86c4b1da0f9a5d24ba1d283` |
| Follow-up full gate | `945dadf648a2f20f2d774aec3ffd72a32f82e10593cbe4deb9d0e7fad4e34d8b` |
| Rejected zero-width certificate test run | `51a5a424944083943c94957a87b8c20d97102cd41af16593ff36768b6280ed62` |
| Corrected core test run | `8120fea89fee81095fc4efa961e333b1cf9503e8c7b04198a6230b6b1e11f33d` |
| Rejected comparator build | `4aa882d1e7e56eb53057c5b845ce1de864eb4a6e54885b896e316fd27464cf36` |
| Follow-up SCC source snapshot | `a731b48c9c29eaed8dcf623cba4e5a35db2e842a8ab8571d7fad21a02d82b345` |
| Mac comparator rebuild | `5ab89f3834e33eec571cda4ea85d008b3ae36dabb794cbb868df5b67a053671d` |
| Mac thirty-three-entry candidate | `d927004c347261c5349635f0567fce791b2772be90000cac9d39f6cc02848340` |
| Mac follow-up replay summary | `b2d8fe6b9ca0a3b248299e490538f7a59d3afc5775515d58cd520b609b6dfea9` |
| Mac follow-up licensed fixtures | `9397e27d95a4515abac5be37f451eaeb22436fe690b213a016707403e1becbf3` |
| Mac follow-up OLS Break | `6e34ae90d9ae35896ca63ce58bd1dd3b893325aa47dcfbe3cf9dfe2f763e3238` |
| Mac follow-up PPML Break | `f5aada3ea8df93ebc394b615bb07031d17c06bbe03831bcbf2f1ade2b1a2f3fb` |
| Rejected fixture filename | `28a351d62776a1b9edd78ebf82660b3d82f91a32a0e09a6e3cc65b3f331b192e` |
| Mac follow-up nonpassing timing panel | `b650f8439453d34a2c5359a680b9680df0bfcb089df404d11063f9b9adef0a99` |
| Linux follow-up instruction summary | `9d355710c03fd4f2f2c248da24c2b70384c7c631747ab8d3da4e60f147aa9f7b` |
| Linux follow-up native summary | `505b0733d3dfc45c4e3bf3599e9ab3c25c89812ab4fe4541c794219ff5637bc2` |
| Linux follow-up accounting | `f323f1b208033555dcc8bd467a254fadbf66c7e66653426580c9e8bd06a0bd48` |
| AKM follow-up caller attribution | `c89307ef5bd3dc979c14fb0df68dd0d8f72c88a42ab208d3deb6e7515bc2f12e` |
| Thirty-seven-entry source patch against ded7f627 | `157e9ec6d30144d426b663e900f63f345423a2bb626a1aec8ad206ce3ec2a9d2` |
| Thirty-seven-entry SCC source snapshot | `86c9abf823a7aed3da5aeb59ed6b09588342ea1e64776a7bcae2b3a43ded3924` |
| Thirty-seven-entry full gate | `02688a398e33aa2bb517c2595050d13c616ce48263edd3e321e56f2d346f18ff` |
| Thirty-seven-entry Mac plugin | `ac74d34954b1ff3fdf6613d6f6ab80c9381ad0762efcdc7ccfaece1276becfba` |
| Thirty-seven-entry Mac replay | `a8710ac1effa10b8e0b5af46022ecd97854877d4c6802e10eae96cf8d7f6849e` |
| Thirty-seven-entry Mac licensed fixtures | `9397e27d95a4515abac5be37f451eaeb22436fe690b213a016707403e1becbf3` |
| Thirty-seven-entry OLS Break | `6e34ae90d9ae35896ca63ce58bd1dd3b893325aa47dcfbe3cf9dfe2f763e3238` |
| Thirty-seven-entry PPML Break | `f5aada3ea8df93ebc394b615bb07031d17c06bbe03831bcbf2f1ade2b1a2f3fb` |
| Rejected Valgrind 3.22 hardware tests | `57354524dd832b2cf2b482d7888dafd6e771302bff7c9bf49ce3915d5e91a9ca` |
| Independent FMA zero probe source | `de1aa78f2bbec19101f4a56553e59bbab236030607235a08b4cb0ae31a7a99c4` |
| Native FMA zero results | `3dbca8400c03dad9461538a71f7fa2ddd986bc31c3d64eb2a28cb84c72b7774a` |
| Valgrind 3.22 FMA zero results | `88c16ef6aca7be918b76e7a82bc0c4380fc4e1c6eb2ba1b31b61a7e68c32a5ea` |
| Official Valgrind 3.27.1 source archive | `5d589152eb8071c02feab8ce6ab719e431a1fbc3e2b1700f5432632a8b9264dc` |
| Valgrind 3.27.1 hardware tests | `98340e74a02c2a36d287fd07465e233f9d8523656d8a5b250365d5cccddbd47e` |
| Valgrind 3.27.1 tool binding | `311da0ccd9ac0bff8ff10861260d179c01f20b4893f06ff2b5bae1f124d6670d` |
| Valgrind 3.27.1 zero probe results | `3dbca8400c03dad9461538a71f7fa2ddd986bc31c3d64eb2a28cb84c72b7774a` |
| Thirty-seven-entry symbolized native instruction panel | `179291d0f93858a77f55426b3eb89881a193a8b8573446d02f6dedae05e69972` |
| Final three OLS FMA callers | `3fd8a635763b00695583757be6db24d751cac28812d96906adf00a308935e8f6` |
| Interrupted fifteen-pair Mac panel | `a2b05c18d1b65da6547ee3088c8d3fdb86a8587050db1534d2f7f2372d0664be` |
| Completed scalar coverage source patch against ded7f627 | `41198869f47dd53d9f24e4610254013f0eee9e0e8e1c47efbe0c8b9b6e7ee1d8` |
| Completed scalar coverage source snapshot | `6ecbaa1535fb22e1328ecec0a3789d1608ef87d2861e302cf13b789d7b6de3c3` |
| Completed scalar coverage full gate | `7f65358d49940d4a0d8ce33c4f4b48bd61a2791fb23ebd17b85570430175c9bf` |
| Completed scalar coverage Mac plugin | `272e4c6019864ee3951533bedd8f7595b910f4e062f0ed75dc7d79e570fccea7` |
| Completed scalar coverage Mac replay | `9ae793ecebf0a7ea307b55a99f4ffeab01eab1991e52412c2154fccf9cb4eab9` |
| Completed scalar coverage licensed fixtures | `9397e27d95a4515abac5be37f451eaeb22436fe690b213a016707403e1becbf3` |
| Completed scalar coverage OLS Break | `6e34ae90d9ae35896ca63ce58bd1dd3b893325aa47dcfbe3cf9dfe2f763e3238` |
| Completed scalar coverage PPML Break | `f5aada3ea8df93ebc394b615bb07031d17c06bbe03831bcbf2f1ade2b1a2f3fb` |
| Completed scalar coverage zero-FMA panel | `eeb417e48c2f51073f2d17b1e1bb241cabd766096b53ea59e1b59ca65665adc6` |
| Completed scalar coverage incomplete native panel | `0cc0fd1fb2fc3eec625feee4fd673713d00812de66459f39bf247aed0981cfdf` |
| Completed scalar coverage failed Mac timing | `8fa0455b9d9da448277d26ea1111606fb711a55c3076c8bf35d5724178ad5bdb` |
| Cached-dispatch source patch | `4aa37b80f717a716942de0a541f5f00f2823dd739beb4737ef6de9302d7911be` |
| Cached-dispatch source snapshot | `ab073eb7574581140e0cc2493bf5d2064e6ff30d534eaae3f767830d1ed658b8` |
| Cached-dispatch full gate | `be1b8276e83a3c79f68d4cf15a95354110557426b3b377ca0addcc99c5237a10` |
| Cached-dispatch Mac plugin | `68c21a90ef763b78c1029aeeb41011606cf0c210225873d9695b5ccacb67edc1` |
| Cached-dispatch Mac exact replay | `5c23e755bc40806ba7f7325a374cb43fee88f21f75d2c96df47767f82ca35cdf` |
| Cached-dispatch Mac timing | `04b36b0d19550c41d86a16af8a528812e8320dc9861190330b4b3ba1b980c055` |
| Cached-dispatch Mac host before | `ec198af26f055c7e09d42ef7a4ea4da9764d2796140decd5ebc9aba799bfc051` |
| Cached-dispatch Mac host after | `0be00fb52fbffd29d0a2883a163f0cceb02349681edab1d11e2462e269ee23dd` |
| Cached-dispatch zero-FMA panel | `fdd0fedf6ed8440d3566040f43a207df4a367e239dfe4f5432d14f53524fcf1d` |
| Cached-dispatch Linux release | `447fe148222d8339a603b043135ca9921088f68ab3298bafa53b8d3bfe096b48` |
| Cached-dispatch Linux profile | `27b66f821c647ed20b01c0556847271fde362c932ea3649a9a4e2616283804f4` |
| Cached-dispatch full-size native panel | `694f6c53b43ffa3da6a88d570d8a53ea2eb4adaa920e27528d9253ec85be206d` |
| Cached-dispatch SGE accounting | `8df3211a5a97d8ef604a3774dc7c0e59f9b270972579b1097a4a23444d65b113` |
| Completed scalar coverage Linux release | `898115854130c8b369d5c41ec3e1424a028a828ec7217db028a821ee6c29ab29` |
| Completed scalar coverage Linux profile | `442cc82692a43c2f51a914b9f29976bb83ff9b1fa6e88c6985f5e71d9b86d6b3` |
| Native panel summary | `26592189e7ab6d8050eadadfbd29316938794ee4a1b44b3651f51f076580a245` |
| Symbolized instruction summary | `c147ce2c3e750e61387d811c31b6135ac8020dc3854c72be348f784de2ab2158` |
| SGE measurement accounting | `c0fa1e5b453c85ee57b5d1212d23ab706a0e4e83466acbc7528adaedae145b9e` |
| Caller attribution | `2088d1978995a04c75b59bd532fbaa62fd5e7245acd7b421d1b4d400d59e2cf0` |
| Timed-out panel | `8e37e5a1c0d3b3f07012ca6b381f9a22b94dc50e5b7106fa5786ec7850bd85ae` |
| Unattributed probe | `3395ce0cc79eba5e68b4a50783b4fc7a692a38740d30857cb853711d0337becd` |
| Rejected symbol lookup | `ac363e09a977089061a751286f342cad7221e62be3a7554f996a12144166d7e2` |

## Accepted evidence

### Campaign-shaped replay tooling (G3), 2026-10-11

The large replay generator adds explicit unrestricted-candidate,
nondeterministic and sample-only-output options, plus an AKM topology with
five observations per worker and 80% persistence. Existing defaults remain
unchanged. The topology uses the oracle RNG; it is not a byte reproduction of
the frozen SCC campaign. All SCC comparisons continue using frozen inputs.
This is an unchanged native artifact: only oracle/tooling code and documentation
change. Production identity against `4d2e873b` passes for all 169 runtime
modules and 58 artifact inputs. No estimator arithmetic, tolerances, routes, sample or rank semantics
change.

The full repository gate and all 42 benchmark-tool tests pass. All 73 default
replay cases are byte-identical before/after. Generator witnesses check worker
shape, determinism, persistence and the required sample output. Small OLS/PPML
native probes each repeat exactly twice, with both certificates passing and
21 physical columns in the two-FE/sixteen-regressor/unit-weight example. The
shared replay wire module stays unchanged. Two helper-placement attempts failed
the unused-code lint before the helper was scoped to the large generator;
no lint or assertion was relaxed. Missing matplotlib/reportlab initially
prevented report tests; installing them in the task-local environment allowed
all tests to run.

| Check | SHA-256 |
| --- | --- |
| G3 commit-boundary full gate | `a90b12905211d5ac131d49374c686ed112bcca20f2f76ba4d7f7937c4855f030` |
| Native probe results | `4b787115fc017526b454195855766cbb7d487af3bdceac3ab6d7c59d1b8d67b2` |
| Default-case identity | `2a5c4447ef7863d449cd265f32623ec70e8b4b3e3fcbb99e3122e72a49de6003` |
| Benchmark-tool tests | `d66b9fcaa555d76fca0295b0d5ed0758e164bfe611c32548618db32f17a30913` |
| Rejected shared helper | `d8828b7d404c9f9bf09681dd76f1b0d6f97ba332e818750dd221d6f6fcbbf3cb` |
| Rejected helper visibility | `6f1175eefd54a2024d761e1c8909eabab09b8ad9afc445e576b972a7e7f4e5bd` |
| Missing matplotlib | `715a473dcc33ff91c61b30f72fdd8805b14d85f4d99a149cb56af49db3d8654d` |
| Missing reportlab | `e52be28333988596fb55d1221cce967f5f1fdafb566501bc3d5df76c6d34c5bc` |

### Performance-plan tooling, 2026-10-11

G1/G2 are arithmetic-preserving tooling changes with an unchanged production
artifact relative to `cad23f41`: the production-identity check passes for all
168 runtime modules and 58 artifact inputs. The campaign captures OLS runtime
rows 18–21 and all 13 PPML phase rows, and new manifests explicitly retain
separate default/strict OLS arms. Strict OLS uses `fixef.tol=1e-11` and
`ssc(K.exact=TRUE)`, reporting covariance DoF separately from raw `nparams`.
Historical campaign manifests and retained observations are unchanged.

All 42 benchmark tests and `scripts/run_tests.sh` pass on Mac. The R witness
uses fixest 0.14.2 and checks disconnected IID/robust OLS coefficients and
covariance against explicit-dummy least squares and an independent sandwich.
A licensed Stata 19 probe executes the campaign metadata block for both models,
returns normally with `CAMPAIGN_METADATA_PASS`, and records four OLS fields
and PPML total plus 13 phases, including an unobserved phase as missing.
Local documentation links and the complexity ratchet pass. No new estimation,
sample, rank, omission, separation, solver, tolerance or certificate behavior
is introduced, and no performance gain is claimed.

Failed setup attempts remain recorded here: the default shell selected Python
3.9 without the scientific packages, and the first gate lacked Cargo on PATH.
The successful checks used a disposable Python 3.13 environment and the installed
pinned Rust toolchain. A disconnected clustered diagnostic with strict fixest
reported `df.K=13`; its expected nesting-adjusted count was 7. That comparison
is not a passing covariance witness. Strict clustered campaign cells retain
their actual count and must pass the existing comparison gate; no native
convention or comparison tolerance was changed to accommodate fixest.

SHA-256 records: benchmark tests
`35b70ecc6a18a22e7d21fba5d7662fa389f180dd8b34009b8cf4221f3a990d97`;
full gate
`e9e85645d44ffa168e8d039381dad1e7c0765137c8b5dc502958b4b7c9d211fb`;
licensed metadata probe
`c0e664c0b33221fa48372db5fece7f8fa9c4d4e91bd0b62f5e273d49cf92d903`;
OLS metadata
`63600d9f89e51edc7d60779df4a7cebc11ec904b62c0dae1b6e0a74b010429dc`;
PPML metadata
`afd7b1d23cae9af6a224b19c7c2fc0960f0b248e1a9fc333e1e0417bd11e6120`.

A downstream audit caught that the comparison whitelist initially omitted the
new `ols-*-K.exact=TRUE` policy label. The follow-up admits that OLS label and
tests that differing correction DoF still fail. All 42 benchmark tests pass;
log SHA-256
`6deb4315f564ba536984df97986fef3111afad4568dd98360ef4fa487bf09860`.
This follow-up changes no runtime or comparison tolerance.

Artifact digests are SHA-256 prefixes; the ledger gives full values.

| Date | Scope | Host | Artifact | Result |
| --- | --- | --- | --- | --- |
| 10-07 | Integrated runtime: 375 families, 98 holdouts, 6,375 native commands at 1–16 workers | Mac | `20a9d3d9` | No scientific failures, exact repeats, stable decisions |
| 10-07 | Single-runtime cleanup: gate, 64 licensed fixtures, 73 replay cases, staged installs, Break | Mac | `14dd962c` | Pass; one parity fixture only after its repair |
| 10-07 | Windows build, PE audit, loader, Stata smoke (`a8cdd888`) | Windows | — | Pass; the first run failed |
| 10-08 | Handover integration `e3380ede`: 1,647 tests, 73 exact replay cases, mapped fixtures | Mac | `66e35883` | Pass |
| 10-08 | Merged FMA group means: Cachegrind, SCC 7968913 | Linux | `07e5e590` | Exact; group-function instructions −26% and −54% |
| 10-08 | Guarded Linux loading (SCC 7969139) and Stata 19 load-time A/B | Linux | `45c54141` | Pass; bit-identical captures |
| 10-08 | Windows rounding and loading, clean `e3380ede` | Windows | — | Pass |
| 10-08 | Wide-Gram dispatch and `within` CSR pin `297f32d9`, quiet timing on 22 workloads | Mac | `5eb93024` | Exact; largest upper bound 1.044 |
| 10-09 | Windows controller repair rerun, `81c51e7e` | Windows | `08995120` | Pass |
| 10-09 | Cleanup phase 1 (`f5eace5c`–`c4d91623`) against baseline `9debf9bf` | Mac | `49ab9e29` | Exact replay; timing within allowance |
| 10-09 | Phases 2–3 (`09fc5e4c`–`261536ac`); ulp scope `296cbc23` rebaselined replay | Mac | `c0f9aff9` | Exact replay; timing within allowance |
| 10-09 | Phase 4 shipped fault plans (`5dcaaf81`–`e97d355e`) | Mac | `fdd48d56` | Exact replay; timing within allowance |
| 10-09 | Integration checkpoint `8fb67bb5`: Mac gate, Stata full (68), Break, Linux build and stores, Windows driver | All three | `c4363362`, `91933073`, `961afc7c` | Pass, except full staging (see below) |
| 10-09 | Saved-output fixture `9c08204b` on Windows | Windows | `ac92ad81` | Pass; Windows batched stores verified |
| 10-09 | Measurement-driven work (`02936956`–`7dd56c01`); one-pass group-mean ulp scope; baseline `d4b68d26` | Mac | `4c1a432f` | Scoped replay; quiet timing pass |
| 10-09 | Windows FMA kernels and module pin `d5eb37a7`, `04a0e255` | Windows | `c3783aec` | Pass after one intermittent loader crash, fixed by the pin |

Timing rows use paired-median bootstrap upper bounds against max(5%, 1 ms) on
the frozen workloads. They are bounded native-call observations on one host,
not release or end-to-end Stata claims.

## Failures, resolutions and gaps

- Resolved on Mac 10-10: `ols_three_fe_batch_oracle.do` had failed with
  `r(459)` on every binary: explicit
  LSMR stopped its recurrence at `tolerance()` and certified per-level FE
  moments at that same tolerance, leaving one weighted column at 1.0167e-8
  against 1e-8. The same gap remained at the 1e-14 final residual projection
  used for saved outputs. In a 128-configuration sweep at 100,000 rows, 2 of 64
  failed on the base plugin. Johannes's decisions A and A' (2026-10-10, commits
  `5413cfe` and `a551feb`) make explicit OLS LSMR iterate to a hundredth of the
  tolerance, floored at 1e-15 and never looser than it, and certify at the
  tolerance. With that, the native witnesses
  `explicit_lsmr_three_fe_batch_matches_explicit_dummy_oracle` and
  `explicit_lsmr_final_residual_projection_keeps_a_margin` pass, and the sweep
  has no failures. Mean LSMR iterations change from 16.7 to 20.7 in the main
  projection and from 16.1 to 13.7 at the residual step on strong graphs.
  Instructions on one 100,000-row explicit-LSMR case rise 3.7%. Linux replay
  against `e237c217` passes under the declared scope
  `tests/replay/scopes/2026-10-10-explicit-lsmr-direct-tolerance.json`: only
  ols-10 and ols-13 change (coefficients at most 1.4e-15 relative, covariance
  at most 2.1e-10 relative), and 71 cases stay exact with 2- and 4-worker
  checks. The licensed fixture and Mac boundary timing now pass; see the
  qualification record below.
- Full release staging failed at `8fb67bb5` because its input held only the
  Mac plugin; only the Mac review archive passed.
- One passing Windows run after the module pin bounds, but cannot exclude, a
  residual loader race; the mapped-after-release assertion is deterministic.
- The LSMR helper's unpreconditioned retry after a positive-definiteness
  breakdown keeps the full allowance and reports only its own iterations.
- Default solver routing is unchanged pending a decision: automatic PCG
  exhausted a reduced budget on weak and chain graphs where LSMR certified.
- Deferred review items: PPML-10 (result-changing), FE-11, OLS-4, FE-16 and
  the PLUG-12 runtime missing value (next wire bump).
- Not adopted: PGO (three-FE regression), the experimental all-FMA build and
  the automatic PCG continuation (failed an 80-level chain at `iterate(128)`).

## Standing caveats

- Mac-only qualification does not establish Linux, SCC, Windows or Intel-host
  Stata evidence; Rosetta exposes no FMA/AVX hardware path.
- FFI allocation tests observe the whole process and run serially; a parallel
  invocation fails by design and is not a runtime defect.
- Retained native allocation tests do not claim exhaustive detection of every
  new allocation.

## 2026-10-10 improvement programme

PR #31 was fast-forwarded from `140dda27` to `06ed5f94` on 10-10. Each stack was
checked on Linux with `./scripts/run_tests.sh` and the 73-case replay against a
`140dda27` baseline plugin (`e237c217`), with zero exact differences; commit
messages record each result. Behaviour changes are confined to admission and
failure paths: the separation report bank's memory charge (plan D9; admission
only loosens), Break reaching saved-FE recovery and long serial row passes, the
PPML validator keeping `r(1)`, PPML rejecting string numeric inputs at layout,
test fault plans compiled out of shipped builds (plan D4, reversing phase 4's
choice), and the OLS multiway PSD certificate floor becoming `f64::MIN_POSITIVE`
(plan D1; it can only reject, and every replay case was unchanged). At
`2c34533`, Linux Cachegrind on two 100,000-row single-worker cases (OLS two-FE
robust; PPML two-FE clustered) compared the base plugin `e237c217` with
`3a4fdc32`: exact outputs, total instructions −0.0027% (OLS) and −0.015% (PPML),
against repeat spreads of at most 5,415 instructions. This is an instruction-count
observation, not timing. The explicit-LSMR decisions (`5413cfe`, `a551feb`) are
the programme's only deliberate result change. After them the Linux replay
anchor moved from `e237c217` (`140dda27`) to `ba350134` (`71c289a`), which
reproduces the declared scope against the old anchor and replays exactly
against itself. The Mac qualification and replay re-anchor are recorded below.

Mac follow-up found two qualification defects at `5d548274`. Its replay had
71 exact cases and only `ols-10`/`ols-13` differences, but both input hashes
differed from the Linux scope; they match the existing Mac corpus. Separate
Mac scope `2026-10-10-explicit-lsmr-direct-tolerance-macos.json` preserves the
Linux scope unchanged and covers two additional one-ulp fields: adjusted R²
and the rank condition proxy. Input-derived 70-digit Decimal witnesses now
check both fields at rtol 1e-12/atol 1e-14. Scoped replay passes all 73 cases
with 2/4-worker checks against a fresh `140dda27` Mac plugin (`4842d6f3`).

The bounded separation report bank also retained an unintended logical ceiling:
600 rows, 257 zero outcomes with separate indicator regressors, then positive
outcomes, with `separation(simplex) keepsingletons`, succeeded on the base but
returned `r(3499)` at the PR tip. Separating initial reservation from the full
logical ceiling and reserving exact-FE follow-up records fixes it. The native
257-generation witness and public sample/intercept/robust-variance checks at
1/4 workers pass; all 73 replay cases remain exact against the original PR
plugin (`04157bcc`). Both that public fixture and
`ols_three_fe_batch_oracle.do` pass on Mac Stata/MP 19. These focused results
use candidate plugin `2d2dd904`; final integration results follow.

| Final qualification (10-10) | Artifact / receipt SHA-256 prefix | Result |
| --- | --- | --- |
| Mac build and replay, merged `06ed5f94` | plugin `336015be`; replay `c8e08323` | Universal audit and ABI/load pass; 73 scoped cases and 146 worker checks pass. Re-anchored from `4842d6f3`; only ols-10/13 differ. |
| Mac timing against `140dda27` | timed plugin `8c1b779e`; summary `18e5f922`; host observations `d67117e9` | All 18 workloads exact and within max(5%, 1 ms), 15 alternating pairs each. Largest bootstrap upper bound 1.04017 (K=129, four workers); paired medians 0.97417–1.00678. Johannes accepted practical quiet; brief background bursts and every pair were retained. Earlier seven-pair upper bound 1.05118 failed; two interrupted confirmations were inconclusive. No release-speed claim. |
| Licensed Mac Stata/MP 19 and PTY Break | full `4ea95203`; hidden `1cab8469`; development `4a8fe8dc`; Break `6e34ae90` / `756572c6` | Full 69/69, ppml-hidden 12/12, development 64/64, including the three-FE oracle; OLS/PPML Break pass. Initial development failures led to fixture repairs in `4f6d0427`; tracing was disabled for allocation assertions. |
| Bounded Windows build/load | plugin `de000832`; receipt `f18dba21` | Native build, ABI/load and licensed smoke pass; machine stopped. |
| Bounded SCC Linux build/load and report-bank regressions | job `8016714`; plugin `eff64e7b`; result `03b58901`; accounting `220ed385` | glibc 2.28 audit, ABI/load, both UBSan variants and both targeted tests pass; scheduler failed=0/exit=0, 216 s, maxvmem 3.886G. Original job `8016626` failed linking system UBSan; explicit GCC 12.2 fixed it and is pinned in campaign setup by `06ed5f94`. |

Full hashes are in the documentation commit. Evidence built at `ac7a7a9` and
`4f6d0427` is reused through `06ed5f94`: all 168 production modules and 58
artifact inputs are identical; `run_tests.sh` passes at the final source.

### Packed SCC campaign after PR #31

Run `20261010T155133Z-pr31-5e2951a4` uses merged source `5e2951a4`
(archive SHA-256 `7b168204`) and Linux plugin `a259496b`. Its report and
complete per-configuration comparison are retained under that run in
`/projectnb/welfgr/fereg/runs/`, in `report/` and `comparison/`; compact
accounting is in `collected/`. This is a descriptive comparison with the
reconstructed summaries of `20261007T171717Z-akm-current` (production
`7942263`), not a universal speed or platform claim.

| Evidence | Result |
| --- | --- |
| Setup `8016862`, smoke `8016895`, production `8016905.1–4` | All six accounting records have failed=0/exit=0; sentinels, frozen identities and all 492 planned fit positions verified. glibc 2.28 audit, ABI/load, UBSan guards, 40 harness tests and 11 native smoke oracles pass. Production used 28 slots on 28-core nodes, without the exclusive consumable. |
| Scientific outcomes and unavailable cells | Native repeatability and cross-worker checks pass for 55/56 configurations; four-million-row three-sparse PPML times out as before. Across 164 arm cells: 131 eligible, 25 asterisked, 8 without timing, versus 129/18/17 previously. The capture repair exposes seven remaining reference discrepancies and makes two strict-fixest cells eligible; no formerly eligible cell becomes ineligible. All failed fits and reference smoke failures remain reported. |
| Timing and scaling | Native current/previous median ratio 0.9521 across 55 configurations; four-worker medians 0.9390 OLS / 0.9582 PPML. OLS AKM at 8/16 workers is 20.2%/20.4% slower on the same node, despite nearly unchanged fixest controls; the follow-up below isolates and repairs the cause. Its 16-worker speedup falls from 2.573× to 1.692×. PPML AKM gains only 2.6% in elapsed time from 8 to 16 workers. No explicit-LSMR arms were scheduled. These separate campaigns do not replace the Mac qualification timing. |
| Memory and report | Native peak sampled RSS 5.032 GiB, essentially unchanged; median 1.120 versus 1.174 GiB across 51 resolved cells. Unresolved memory stays NR. All 19 PDF pages rendered and visually inspected. PDF SHA-256 `ba133153`, configuration summary `f0ab36f9`, full comparison CSV `6c8b9cc9`, retained-file manifest `a4bb00bc`; full hashes are in the documentation commit. |

### OLS high-worker slowdown investigation

Three bounded SCC experiments on `scc-zj4` used nine alternating or rotating
pairs per arm at 1/4/8/16 workers, with 28 slots on a 28-core node and no
exclusive consumable. The frozen AKM input has one million rows, 16 regressors,
200,000 worker and 10,000 firm levels, iid inference and seed 739391 (input
SHA-256 `5fd1a5cf`). Full-size warmups precede timing; diagnostic traces follow
it. Inputs and the timed Stata command are unchanged from the campaigns.
Reproducers, observations and accounting are retained under
`/projectnb/welfgr/fereg/dev/20261010-ols-slowdown/`.

Commit `b21244aa` replaced private scalar norm accumulators with repeated writes
to adjacent shared scratch slices. Keeping its contiguous column partition but
privatizing the recurrence removes the regression. The selected repair uses
bounded 64-column stack tiles (1 KiB per active worker), then publishes scratch
once. It preserves each column's observation order and arithmetic and adds no
heap allocation. The intervention supports a shared-scratch implementation
cause, consistent with false sharing; no hardware cache-counter measurement
establishes that mechanism exclusively. Restoring the older per-column loop was
also measured, but retaining blocks performed better at four/eight workers.

| Evidence | Result |
| --- | --- |
| Endpoints, job `8017329` | PR31/October-7 paired ratios at 8/16 workers: 1.0942 / 1.1274; 95% bootstrap intervals [1.0772, 1.1890] / [1.0883, 1.2688]. Norm setup and certificate time increase; operator time and 14 solver iterations remain stable. This reproduces the campaign slowdown with smaller magnitudes under paired measurement. |
| Controlled intervention, job `8017369` | Private-block/control ratios at 1/4/8/16 workers: 1.0278 / 0.8076 / 0.7322 / 0.7981; upper bounds 1.0367 / 0.8358 / 0.7949 / 0.8524. The observed single-worker cost is retained. Exact science and solver signatures across variants; scalar norm/scratch witnesses through 257 columns and 1/2/4/8/16 workers pass. Linux glibc 2.28 audits, ABI/load and both UBSan guard variants pass. |
| Direct repaired/October-7 confirmation, job `8017720` | Ratios 0.8269 / 0.7950 / 0.8649 / 0.9210 at 1/4/8/16 workers; all upper bounds below 1.0. Coefficients, covariance and model metadata are exact. Historical residual/certificate diagnostics differ already before the repair; current-to-repaired diagnostics are exact. Peak sampled RSS remains about 650 MiB. All three jobs have failed=0/exit=0 (181/697/172 s). |
| Mac science and transactions, plugin `c5190816` | `run_tests.sh`, universal audit/load, 73 exact replay cases and 292 checks at 2/4/8/16 workers pass. Working-tree licensed profiles: full 69/69 and ppml-hidden 12/12, including OLS/PPML PTY Break. Source patch and plugin hashes bind these profiles; their source label is the parent commit. No fresh Windows check is claimed. |
| Mac timing and future coverage | All 20 frozen workloads retain exact science/signatures. Seven-pair run passes 19: PPML/four-worker upper 1.06765 fails during observed backup/sync activity. One predeclared 15-pair confirmation passes (upper 1.00211); pooling all 22 pairs also passes (1.00538). Largest upper among the other 19 is 1.03420. Practical-quiet observations and every failed pair remain retained; no release-speed claim. The new eight/sixteen-worker K=20 guards pass, and the verifier now rejects unlisted/missing/modified workloads. |

The repair's Linux plugin is `e5eed20e`, built from `5e2951a4` plus the measured
private-block patch; its production tokens equal the integrated repair. Full
artifact, source-patch and result hashes are in the repair commit. The old 18
frozen workload hashes are unchanged. These are bounded workload observations,
not a universal speed or platform claim.

### Packed SCC campaign after the OLS repair

Run `20261010T192805Z-ols-repair-e29164e6` compares source `e29164e6`
(archive SHA-256 `3129d28a`, Linux plugin `57cc9789`) with the actual preceding
PR31 campaign `20261010T155133Z-pr31-5e2951a4`. Compact results, all 164
per-configuration arm comparisons and the 19-page visually inspected PDF are
retained under the new run in `/projectnb/welfgr/fereg/runs/`, in `collected/`,
`comparison/` and `report/`. Full hashes are in this documentation commit.

| Evidence | Result |
| --- | --- |
| Setup `8018263`, smoke `8018275`, production `8018282.1–4` | All six scheduler records have failed=0/exit=0; sentinels, frozen identities and 492 fit positions verified. glibc 2.28 audit, ABI/load, both UBSan variants, 41 harness tests and 11 native smoke oracles pass. Production used 28 slots on 28-core nodes without exclusive. |
| Scientific and failure outcomes | Native repeatability and cross-worker checks pass for 55/56 configurations; the same four-million-row three-sparse PPML case times out. All 164 arm cells retain eligibility and failure reasons: 131 eligible, 25 asterisked, 8 NR. The 15 reference smoke capture failures and 18 oracle failures are unchanged. |
| OLS scaling on identical inputs and `scc-zj4` | AKM one-million-row times at 1/4/8/16 workers are 2.2325/1.2916/1.0426/0.9551 s; current/previous ratios 1.0342/0.7961/0.7134/0.7486. This confirms the controlled repair at 8/16 workers while retaining the 3.4% single-worker cost; fixest controls remain within 1%. Sixteen-worker speedup improves from 1.692× to 2.337×, but 8→16 still saves only 8.4%. |
| Breadth, PPML and memory | Four-worker median ratios are 0.8857 OLS / 1.0124 PPML. Native ratios across 55 eligible cells range 0.7134–1.0870; increases remain visible. PPML AKM robust 1/4/8/16 times are 12.8068/6.4174/5.3236/4.9661 s on a different node, with only 6.7% improvement from 8→16. Peak sampled native RSS is 5.03249 GiB versus 5.03227; common 51-cell median RSS ratio 0.99999. Unresolved memory remains NR. |
| Cross-campaign input audit `8018721` | All 150 successful records in 50 configurations with identical numeric inputs have exact captured estimator science, including all OLS configurations. Five PPML inputs generated on different nodes differ only in response values by at most two ulps (verified hashes; failed=0/exit=0). Their 15 fits differ only in coefficient/covariance rounding, within unchanged SPEC tolerances; discrete metadata and reported objectives remain exact across all 165 successful fits. The three existing unavailable records remain unavailable. |

SHA-256 prefixes: PDF `5dc95256`, configuration summary `4a3440b3`, full
comparison `c4c91585`, retained-file manifest `5603b923`. The complete comparison
flags input and host changes.
Separate campaigns are descriptive: they neither replace Mac qualification nor
establish universal speed or platform claims. No explicit-LSMR arms were run.

### General scaling investigation against fixest

The [detailed investigation](fixest_benchmarks.md#generalization-follow-up)
uses unchanged runtime `e29164e6`, Linux plugin `57cc9789` and pinned fixest
0.14.2. Reproducers, frozen synthetic inputs, captures and CPU samples remain
under `/projectnb/welfgr/fereg/dev/20261011-fixest-general-scaling/`.
Compact results and full hashes are in the linked report's retained evidence.

| Evidence | Result |
| --- | --- |
| Phase/solver panel `8022196`, holdout `8022206`, CPU sample panel `8022215` | All 70 planned fits complete; all three scheduler records failed=0/exit=0, 284/519/90 s. The holdout extends to 16m rows, 4/64 controls, skewed/weak networks and 28 workers. Native captured science agrees across workers; all four original fixed-worker repeats are exact. Four default weak-network reference comparisons remain ineligible; they are not counted as speed wins. |
| Generalization and memory | Fixed-cardinality, 16m-row OLS takes 20.441/11.758 s at 4/28 workers versus fixest 7.427/6.829; PPML 63.258/54.484 versus 31.992/25.888. Peak sampled native RSS is 6.422/10.614 GiB for OLS/PPML. These are single instrumented fits, not qualification timings. |
| Root causes | Dense two-FE PPML setup remains about 8.9 s at 4/16 workers. Its serial neighbour-pair assembly grows 8× for 4× rows; annotation attributes 96.33% of builder-local sampled cycle weight to that loop. Dense three-FE OLS MAP controls take 22–24% less time than auto in the sampled panel, while retaining scientific agreement. Runtime and solver defaults are unchanged. |
| Independent weak-network audit `8022296` | Direct sparse OLS agrees with native slopes within 1.11e-13, RSS within 7.22e-9 and exact graph rank/DoF. Tight fixest convergence plus exact FE counting resolves the OLS discrepancy; two strict PPML arms time out at 120 s and remain unavailable. Audit failed=0/exit=0, 318 s, 1.947G; original launcher `8022280` exited 1 before science and was repaired for four-slot affinity. |

The [compact follow-up evidence](../bench/results/scc-20261011-scaling-diagnostics.json)
has SHA-256 `3d864721`; its full hash and retained-file manifest hash are in
the documentation commit. The stale PPML help description of auto selecting
LSMR was corrected to match the existing MAP-owned policy. `run_tests.sh`,
the 16 focused help checks, unchanged-production identity and local link checks
pass; the Mac run reports the expected Linux mock-plugin audit skip. No new
Mac timing, Windows qualification or release-performance claim is made.


## Direct decision-loss policy review and pre-fit freeze (2026-10-22)

Independent GPT Pro and Claude Fable 5.1 reviewed the complete current source
and all retained calibration observations, then each read the other's complete
proposal. Both recommend Pro's existing joint two-gate formulation; no synthesis
or third formulation is selected. The [review record](../bench/results/two-fe-policy-revision-review-20261022.json)
retains their complete substantive reports, original identities, disclosed
errors and withdrawn proposals. Two reviewer extraction-root strings are
replaced by an explicit presentation placeholder; their original hashes remain.

The [prospective specification](two_fe_auto_gates.md) declares the affected
scientific scope and witnesses, 21 features, fixed finite class (at most 8,464
pairs), exact common-origin loss, narrow pooling predicate, tie order, wire
encoding, owner-level episode and dependency compilation before the fit. The
84 no-episode cells remain in the coverage denominator; missing treatment costs
are unknown. The 36 expired Force32 contexts may pool only after exact scientific
and trajectory agreement and the source-bound two-byte request-parent comparison,
with every raw usage checked against its own raw limit. No unpooled replacement
model may be selected. Pre-16 observation refusal cannot silently move an episode
or consume a construction attempt; preserved route/ledger eligibility still
controls which checkpoint can select the episode.

Fable disclosed two exploratory component fits and a margin/horizon grid;
neither is a prospectively frozen candidate. The record preserves both runs,
including the future-startup double charge, fractional/integer evaluator
mismatch, understated weighted errors and conflicting final-credit prose. Its
proposed held-out model selection and changed budget were withdrawn. Pro's
literal resource-equality claim was false and has been corrected narrowly.
No failed evidence was erased and no runtime coefficient was installed.

The lead and bounded Astra independently reviewed the specification and offline
fitter. Focused synthetic witnesses cover common-origin costs and signed suffix,
warmup exclusion, joint reachability/body diversity, fixed cuts, exact ties,
canonical encoding, strict pooling and malformed provenance. No real-data gate
fit, new native workload, held-out observation or default activation has occurred
at this freeze. Existing runtime evidence is reused because runtime production
and artifact inputs are unchanged. Tooling changes receive focused checks.

The loss is observer-excluded projection-suffix time; it does not establish
complete-command improvement. Fable's pre-fit prediction is a role gate that
builds non-final first-eligible actions at16 and never builds at32. If selected,
that rule requires no proxy banks/scans but has no remaining-work or cost
sensitivity. Intermediate-length solves, fill-heavy factors, different RHS
schedules and excluded later opportunities remain explicit economic risks.
The expected role split is a falsifiable prediction, not a target imposed on
the fit. Development and final qualification budgets and protected incumbents
remain unchanged; there is no performance-pass or activation claim here.

The first source-bound execution at `8ee565b7` stopped in collection before
any gate evaluation: the Linux archive lacks the Darwin-only recovery fields.
The failure and log identity are retained in the review record. The bounded
collector repair distinguishes the two historical schemas explicitly, checks
the bound complete/no-error 864-call summaries and exact nine-call cells, and
rejects recovery markers inconsistent with that platform. It changes no
observations, feature/cut rules, loss, tie order or fitting budget.

## Selected direct-loss gates and independent audit (2026-10-22)

After the collector-only repair was committed at `ca090b06`, the single
[frozen fit](../bench/results/two-fe-policy-gates-fit-20261022.json)
completed with exit 0. Its SHA-256 is
`5b42c6e24862e0a4c008aa9a1b7d0256e8d335e2ac82d50d9f530a73b2bff573`.
The [audit record](../bench/results/two-fe-policy-gates-audit-20261022.json)
retains the exact execution/log identity, independent decoder source and report,
descriptive diagnostics and all three harmful training choices. The independent
audit reproduced all 108 raw/pooled cell costs and decisions, 192-cell coverage,
204 unavailable labels, the 56 × 52 grids, 1,479 admissible pairs, both primary
loss ties and the exact 128-byte encoding. It also checked all 1,728 archived
calls and the 240 recorded plus 120 warmup pooling comparisons. Its two
validator-only failures preceded enumeration and remain recorded with their
log hashes; correcting those checks changed no observations or selected model.

The selected gate builds at step 16 when the first eligible initial action is
packed or outcome, defers final residuals, and never grants at step 32.
Unsupported regressor-only and one-off episodes close Unknown before that
predicate. It projects 72 Force16 and 36 Never choices; the other 84 cells have
no episode and unknown counterfactual regret. The three harmful builds occur
on high-degree-weak Fast seed 2026102201: Darwin W8 loses 10.165624 ms;
Linux W4 loses 0.4990565 ms and W8 loses 17.335576 ms against Never.
No second model was fitted to avoid these losses.

The selected model's pooled projection-suffix empirical regret averages
`56000513/384` ns per coverage cell. Re-evaluating this same model with unpooled
costs gives `58195663/384` ns; this diagnostic selects no replacement.
No identical cheap-exposure tuple repeats within platform and checkpoint age,
so the descriptive setup-variation table for such groups is empty. Remaining-
work tokens are also descriptive, not literal step forecasts or selected
runtime dependencies. Original numeric arrays were not recreated by this audit;
it verified retained fingerprints, trajectories and source bindings.

This evidence stage contains no new native calls, held-out observations or
public activation. The selected rule needs no progress banks/scans, but its
callback and caller costs must still be measured in complete-command tests.
It is a fitted candidate with known training losses, not a fastest-command or
qualification result. The separate held-out and 42-cell schedules remain fixed.

## Selected structural gate implementation (2026-10-22)

The [implementation record](../bench/results/two-fe-policy-gates-implementation-20261022.json)
binds the selected fit, changed source, focused checks, four discriminating
mutations and independent runtime/tooling reviews. The hidden
`routing_two_gates_v1` invocation implements the selected first eligible
initial-epoch step-16 packed/outcome grant, terminal final/unknown deferral and
correction closure across caller actions. Its two-byte owner state is charged
to memory admission. It introduces no progress bank, observation scan, price
forecast or timing-based routing. Public successor activation remains disabled.

Real Fast/Low caller witnesses cover chunks, final residuals, retired RHS lanes,
analytic OLS/projection science, W1/W4 repeats, direct routing after economic
closure, consumed/fresh accuracy keys, memory refusal and pending cancellation.
All four bounded mutations fail assertions: granting before step16, reopening
after correction, omitting the shell charge and granting on final residuals.
The source was restored. Independent review found no remaining substantive
runtime defect; it did not run another native or timing campaign.

The final `scripts/run_tests.sh --source-only` run exits 0: 315 Python tests
plus seven portable-check Python tests, 87 successful Rust result groups
(2,000 passes including child processes, not a unique-test count), fresh-process
allocator audits, formatting, Clippy, fuzz compilation, portable checks,
complexity and C-bridge/numeric guards. Its log SHA-256 is
`711d92e38cbc64d528c1e43510d244b4d124672393fcc3721c9cc6f3b6a82e55`.
The earlier formatting and needless-lifetime failures remain recorded. Standard
installed-package binding still fails against the development runtime; source
integration does not qualify the installed distribution, which was not changed.

The fixed development adapter passes 116 affected and legacy Python tests and
nine focused tests after its action-boundary repair. Scored calls use
`NATIVE_SECONDS` without diagnostic tracing; warmups carry resource and gate
evidence. Grants and build outcomes must belong to a completed action, match
its role and age, and have stable action ordinals across workers. The manifest
binds the selected fit/wire/specification and the report binds analyzed result
bytes. A reviewer's separate missing-result allegation was withdrawn after
confirming that initialization already existed; its regression test passes.
These runner tests use synthetic native/science substitutes. Actual candidate
builds, native replay/transactions and held-out measurements remain separate
required evidence, with no speed or qualification claim at this boundary.

Completed calibration numeric captures (576 files, 2,334,286,464 bytes) were
removed only after every actual hash matched both its original result and
committed compact record. Current inputs, artifacts, scalar observations and
audit sources remain available during the unfinished workstream. This cleanup
does not erase failed or inconclusive evidence or recreate a numerical oracle.

## Selected structural gate native integration (2026-10-22)

The [native record](../bench/results/two-fe-policy-gates-native-20261022.json)
binds native source `e9be9e64` and CMG `fd77d98d`, fresh Mac/Linux builds,
actual binary hashes, audit/load/ABI/panic checks, commands, scheduler accounting
and all 72 targeted integration calls. Both platforms pass 36 calls across
three existing calibration bodies, Fast/Low and W1/W4, using accepted-public,
hidden candidate and candidate repeat. Independent numerical references, scoped
compatibility, fixed-worker repeats, cross-worker discrete decisions, gate
traces and resource checks pass. These correctness calls have no performance
score and do not consume held-out inputs.

The actual Mac candidate also passes 73 public exact/scientific replays with
146 W4/W8 checks, six mapped Stata fixtures and real optional-builder/explicit-CMG
Break/rollback checks. Independent review rehashed and reparsed retained evidence
on both platforms, including the collected Linux binary. It did not recompute
numerical oracles; Linux numerical columns were not collected for local rehash.
Mac native execution is arm64, not an Intel-Mac execution claim. Shared Break
fixtures and gate-specific source cancellation witnesses cover distinct seams.
The standard installed-plugin binding remains failed, and release packages are
unchanged. Held-out speed, final qualification and public activation remain open.

Completed duplicate native captures and isolated build caches may now be removed
after verifying the hashes retained in this record. The unfinished workstream
retains its current inputs, candidate and protected baseline artifacts.

## Selected structural gate held-out development freeze (2026-10-22)

The [prospective freeze](../bench/results/two-fe-policy-gates-development-freeze-20261022.json)
contains both actual artifact/input/reference/source manifests before any timing:
eight held-out bodies at seed 2026104201, Fast/Low, W1/W4/W8, original public
`f35f3a4d`, accepted public `1cdb225e` and selected hidden native `e9be9e64`.
Each platform has one retained warmup and two recorded calls per arm/cell:
432 calls, 120 seconds/call, 14,400 seconds/platform, no retries or additional
sampling. Order seed is 2026102205; final seed 2026105201 remains untouched.
All source and numerical input bindings agree across platforms.

The protected original has fresh audited Mac/Linux builds with the candidate's
compiler settings and build date. The selected model and native candidate are
unchanged. Minimal gate/resource traces use only warmups; complete native-call
`NATIVE_SECONDS` scores include caller/policy costs without diagnostic tracing.
Linux uses one verified isolated physical host; Mac retains ordinary active
desktop noise. The two recorded calls do not establish independent-host
uncertainty or qualify performance. No fit or model reselection follows this
panel. The separate 42-cell qualification and exact activation gates remain.

Before this freeze, 72 completed duplicate Mac input/column captures totaling
3,390,566,334 bytes were hash-verified against the committed native record and
removed with the completed 559,294,252-byte candidate build cache. Current
comparison inputs, artifacts and compact observations remain. The baseline
archive's final-printer failure and successful independent source verification
are retained; no native measurement failed or was replaced during preparation.
