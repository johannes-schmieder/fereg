# Development

Scientific behavior is defined by [SPEC](SPEC.md); ownership and boundaries by
[ARCHITECTURE](ARCHITECTURE.md). Work in coherent commits on the branch specified
by [AGENTS.md](../AGENTS.md) and use
independent oracles to establish correctness. The repository has one runtime.

## Local checks

Python tooling uses Python 3.11 or newer. Scientific Python helpers require
NumPy, pandas and SciPy. Rust uses the pinned toolchain and Cargo.lock.

```sh
./scripts/run_tests.sh
./scripts/build_macos.sh
./ci/run_stata_ci.sh full
```

The full entrypoint covers Rust, Clippy, allocation/FFI audits, runtime complexity,
the mock SPI bridge, portable shell/Python/strict-C checks and the exact-rational
FE boundary-response and cancellation witnesses. Licensed Stata profiles also include `smoke`,
`differential`, `fereg-regression`, `ppml-scientific` and `ppml-hidden`. The
`development` profile runs every `tests/stata/development` fixture, and `triage`
lists the remaining fixtures no other profile or script runs, for Johannes to map
into a verdict profile or retire.
See [the Stata runner](../ci/README.md). Target mapped tests during development;
run the applicable integration checks before committing runtime changes.

For an unpublished experimental source copy, `./scripts/run_tests.sh --source-only`
runs every source integration check while explicitly omitting the installed-package
checker. Its verdict does not establish the installed package's source binding,
inventory, plugin hashes/ABI or Mata-object integrity. The default command remains
strict; it never falls back to this mode after a failure. Unknown or additional
arguments fail before checks start. Preserve any failed default invocation.
Source integration is a development prerequisite, not package admission: separately
build and audit/load the actual candidate, bind its source and tools, and run the
declared scientific and transaction checks before interpreting an experiment.
Keep the installed runtime unchanged while testing private patches. Main must pass
the default entrypoint with matching installed artifacts before a distribution
change is committed; source-only success never authorizes publication or promotion.

## Compact candidate qualification

Johannes's 2026-10-18 decision replaces mandatory A/B/C stages and recursive
historical proof validation with one fixed-candidate round. The maintained
entrypoint is [`bench/qualify.py`](../bench/qualify.py): `plan` fixes actual
artifacts, input/reference files, worker counts and budgets; `run` collects one
shard/replica; `report` combines completed work. See the [configuration and
commands](../bench/README.md#fixed-candidate-qualification). A smoke run checks the
tooling and cannot qualify a candidate. Do not restart old campaigns to use the
new entrypoint, or repeatedly resample until a pass.

Record source revisions and build settings with the actual binary and harness
hashes. The plan hashes each input and numerical reference once; each job
verifies these files once before execution and checks for drift during work.
There is no dependency on old selections, receipts, generator executables or
Python-environment history. Independent reference mathematics and current native
outputs still require their normal checks. Reuse valid evidence when production
artifacts and affected scope are unchanged.

Keep the existing 21 routing families at one and four workers (42 cells).
Compare the candidate with original Auto and the last accepted Auto; share calls
when the actual binary and prepared request are identical. If activating public
Auto, also require exact agreement with the selected hidden invocation. Explicit
solver frontiers are optional investigations, not part of every round. The
mapped runtime, scientific and transaction tests still cover OLS/PPML features,
explicit requests, memory controls, cancellation and rollback.

Run Linux shards in parallel through [`bench/qualify.sge`](../bench/qualify.sge).
All arms of a comparison share the same verified isolated whole-node allocation;
slot count alone is insufficient. Collect terminal scheduler accounting, and
aggregate Linux uncertainty at the physical-host level rather than treating
many solves on one node as independent observations. Use focused serial Mac
measurements and integration checks for Mac behavior. Linux evidence cannot
establish a Mac performance claim. The controlled timing rule below retains
max(5%, 1 ms), balanced complete-solve blocks and the existing confidence standard.

The incremental Auto workstream remains on `codex/auto-routing`, protecting the
original uninjected baseline `f35f3a4d` and the last accepted branch Auto.
Many-FE passed and was activated in `d5b06cf3`; Johannes then authorized the
separate two-FE qualification. The selected GATES policy passed Mac/Linux
qualification and the approved public/hidden comparison; its development-branch
activation is recorded in [validation](VALIDATION.md#selected-two-fe-gates-branch-activation-2026-10-23).
Activation is limited to OLS Auto/Auto categorical
intercept designs; exact hidden/public equivalence, independent numerical
witnesses and the [declared scientific scope](SPEC.md#incremental-auto-qualification-2026-10-17)
remain required.

Preserve the historical many-FE failures as failures. The first witness attempt
failed before native science; the later repaired attempt completed native
science but timing setup failed before timed calls. Its valid science and current
inputs may be reused: a timing-setup repair does not require regenerating them
or replaying A/B. The report records missing or failed work honestly. Diagnose
operational failures and attempt bounded repairs, retaining completed evidence.
Ordinary resume uses only untouched work under the same plan; the reviewed Mac
recovery below additionally permits local interference retries. If repairs do not resolve
a blocker, science fails or timing remains inconclusive, discuss it with Johannes
before abandoning the candidate or moving to the next step. No tool selects a
replacement, launches another round or activates a default automatically.

Johannes's latest 2026-10-18 direction makes Linux the primary performance test
and permits rerunning individual Mac tests that encounter interference. Preserve
complete cells whose original scientific and observation evidence is valid;
discard the entire affected partial cell and measure only it and unfinished
cells. Historical runs retain their failed status. A flat recovery manifest binds
the immediate source plan, results, raw observations and retained scientific
snapshots, without replaying older continuation chains.

The Mac recovery observer retains ordinary desktop activity and small known-tool
bursts as noise. It flags at least one CPU-second within a valid sample interval
of at most 7.5 seconds, or a known competing process runnable at 20% CPU or more.
These are practical sampled screens, not proof of no interference; synchronous
cell-tail samples can split CPU-delta intervals, and macOS CPU percentages can
reflect earlier activity. All timings are retained. Seal each complete cell with
a tail observation, then invalidate only an attempt whose measurement span
overlaps flagged activity. Require two clear sampled intervals within a bounded
120-second wait before its single retry. Allow at most four retried cells in the
recovery. Persistent activity, exhausted caps or observation faults stop new work
while preserving completed evidence. Scientific/native failures and unfavorable
or inconclusive performance do not trigger retries. The existing max(5%, 1 ms)
allowance, uncertainty gate and scientific checks remain unchanged.

The completed many-FE recovery retained 26 complete cells and ran the affected cell plus
15 unstarted cells. Its maximum is 2,940 new calls, including warmups and four
possible retries, and 6,878 cumulative calls including both failed segments.
The 12,747-second execution budget keeps cumulative execution within the original
four hours. There is one final performance assessment, not repeated looks until
success. See the [Mac recovery rules](../bench/README.md#results-repair-and-cleanup).
New Mac qualification plans apply the same bounded cell policy from the start,
without requiring a failed run or prior-evidence manifest. Count all distinct
execution arms and discarded attempts against the frozen call cap. Resume,
continuation and recovery cannot reset a fresh plan's budgets. Two-FE proceeds
only under its separate fixed-candidate plan; no main merge or release-package
changes are authorized.

GitHub testing is paused. Mac remains the default qualification host; Johannes
additionally requested bounded Linux and Windows build checks on 2026-10-07.
Existing GitHub build definitions remain inactive.

`scripts/build_linux.sh` and `scripts/build_windows.ps1` build the native plugins
and run their binary audits and actual-plugin loader checks. The Linux audit
rejects glibc symbol versions newer than 2.28, so a release Linux build needs a
glibc 2.28 build environment (for example AlmaLinux 8 or Debian 10); on newer
hosts the build produces a plugin but the audit fails. The private Windows
controller's `stata-do` profile uses `windows-ci.do` for an exact-source build,
checksum verification and licensed OLS/PPML smoke checks. A successful build is
not a full scientific or performance qualification for that platform.

Windows failures have two independent reports: the controller's process outcome
(reported failure, exit without a marker, invalid marker, launch failure or
timeout) and the last project stage, with native and Stata return codes. Read
`WINDOWS_CONTROLLER_OUTCOME`, `WINDOWS_LAST_STAGE`, `WINDOWS_STAGE_REASON` and
the optional `WINDOWS_NATIVE_RC`, `WINDOWS_STATA_RC` and `WINDOWS_RUST_ERROR`
lines from the accepted Windows wrapper. Its source-bound JSON receipt retains
completed stages, elapsed time and the verified plugin hash. A timeout reports
the stage that was still running; it does not establish that stage's cause.

| Last stage | Next investigation |
| --- | --- |
| `startup`, `toolchain`, `source_identity` | Driver startup, installed tools or exact source manifest |
| `cargo_build` | Native exit code and bounded Rust error codes/source locations |
| `exact_fma`, `fused_kernels`, `numeric_guards` | The named arithmetic or native safety test |
| `c_bridge`, `plugin_link`, `binary_audit`, `native_loader` | C compilation, linkage, ABI or actual-plugin loading |
| `checksum` | Plugin checksum and verification receipt |
| `stata_smoke`, `single_runtime` | The corresponding Stata fixture and Stata return code |
| `unknown` | Controller startup or missing/invalid diagnostic data; no project result is established |

`scripts/windows_ci_report.ps1` writes stage data and an atomic final marker;
`scripts/windows_ci_diagnostics.py` validates a fixed set of fields before they
leave Windows. Raw Stata logs, exception text and environment values are never
collected. Native build failures remain the primary diagnosis even when Stata
subsequently reports a missing plugin. Success requires every stage and the
current plugin hash, in addition to the final marker.

Run collector tests with `python3 -m unittest discover -s tests/python -p
test_windows_ci_diagnostics.py`. On Windows PowerShell 5.1,
`ci/test_windows_diagnostics.ps1 -Controller <installed-controller-path>` tests
the actual process loop, partial markers, early exits and timeout tree cleanup
using isolated child processes. It does not invoke licensed Stata or AWS.
`ci/windows_controller.patch` records the shared controller integration against
its accepted baseline. Changes to the installed controller or collector require
the Windows skill's maintenance workflow and matching hash pins in the wrapper;
editing a project copy alone does not update the shared service.

`./scripts/run_tests.sh` also compiles the separate `fuzz/` package. Bounded
fuzzing uses `cargo fuzz` (nightly) from `fuzz/`, for example
`cargo +nightly fuzz run raw_abi_args -- -max_total_time=60`; record target,
toolchain, duration, corpus hash and outcome as described in
`tests/fuzz/README.md`.

Use `scripts/run_plugin_replay.sh` for real-plugin independent dense science and
fixed-thread repeat checks. A prior binary may additionally serve as an exact
regression comparator. It is not a second installed runtime or scientific authority.
Declare intentional numerical scope before changes; preserve unaffected results.
The explicit-LSMR scope dated 2026-10-10 has separate Linux and `-macos` files:
the generator's platform `sin`/`exp` calls do not promise identical input bytes.
Each file binds exact frozen inputs; the Mac inputs match the earlier committed
Mac corpus. The Mac scope additionally names adjusted R-squared and the finite
rank condition proxy, independently checked by the 70-digit dense-dummy oracle
in `crates/fereg-oracle/python/replay_ols_reference.py`. Rank, sample, omission,
fixed-thread repeats and all fields outside the chosen scope remain exact.
The replay harness bounds the corpus at one million rows and ten million cells;
local large-case profiling may compile it with larger `REPLAY_MAX_ROWS` and
`REPLAY_MAX_CELLS` (see `tests/mock_spi/replay_io.h`). Such timings are local
observations unless qualified under the evidence rules below.

## Native and frontend boundaries

SPI runs on the main thread. Rust workers use owned memory. Keep panic containment,
fallible allocation, Break handling, validation-before-publication and rollback.
Original factor slots/base flags, independent certificates and both bias modes
remain mandatory. Different workers need scientific agreement and stable decisions;
a fixed deterministic worker configuration must repeat exactly. Bitwise identity
across platforms or processor features is not required. Processor-dependent
fast paths retain independent certificates, scientific witnesses and stable
discrete decisions; the portable path remains supported and tested.

Delete production code reachable only from its own tests instead of hiding it
behind test configuration. Keep independent reference implementations and their
consumers in fereg-oracle or fereg-testkit; preserve unique scientific witnesses
when removing an obsolete production pipeline.

The three-FE LSMR hot path is in the pinned `within`/`schwarz-precond` dependency
from `johannes-schmieder/within`. Its changes follow the same scientific and
evidence requirements and enter this repository through a reviewed pinned
revision bump.

After editing `_fereg_ppml_result_mata.ado`, rebuild its three `.mo` files and
receipt using the actual Stata 18 compiler with `scripts/build_ppml_mata.py`.
A newer compiler's version statement does not ensure Stata 18 compatibility.
Staging validates the source/object hashes. There is no runtime compilation fallback.

## Outputs and performance

Use `target/` for one current build/test cache and `dist/` for the current plugin.
Task outputs (candidate plugins, comparison builds, worktrees, data, logs, profiles) go
under `target/tmp/<task>/` and are deleted when the task is reported, after
recording hashes and summaries; `scripts/clean_target.sh` lists everything else
in `target/` except Cargo's cache and `target/replay-baseline/`, and `--apply`
removes it. Keep large generated inputs and outputs out of Git. During an unfinished
workstream keep only its current artifacts, inputs and applicable evidence;
remove superseded build trees, archives, logs and harness copies.
The compact benchmark entrypoint is documented in [bench](../bench/README.md).

Use the following evidence tiers, with identified artifacts, predeclared workloads,
measurement budgets and stopping rules:

- Arithmetic-preserving changes require zero exact replay differences and
  unchanged solver signatures. Instruction counts or function-level CPU samples
  may establish reduced computational cost, together with no wall-time regression
  beyond max(5%, 1 ms) on the standard workloads. Instruction counts are
  deterministic for a fixed execution; CPU samples are statistical observations
  and must report uncertainty. Wall-time gains smaller than the allowance remain
  observations and do not by themselves fail an otherwise supported change.
- Result-changing changes require declared numerical scope, independent witnesses
  and controlled paired timing on the qualification host. Unaffected
  scientific results remain exact; affected results must satisfy SPEC.
- Unchanged artifacts: a commit that only changes tests, documentation or
  tooling passes `python3 scripts/check_complexity.py --compare-production HEAD~1`
  (identical runtime token streams after removing comments, whitespace and
  test-only items; byte-identical C bridge, manifests, lockfile, toolchain and
  vendored sources). It needs `./scripts/run_tests.sh` only, with the prose-only
  exception below. Test/tooling edits still require their applicable checks.
- Prose-only documentation/instruction changes: verify the commit changes no
  executable code or artifact inputs, check local links/references and run
  `git diff --check`. Reuse valid runtime evidence; no build or full-suite rerun
  is required. A changed scientific rule authorizes no untested implementation.
- Workstream boundaries: arithmetic-preserving commits within one workstream
  share one wall-time nonregression check at its end rather than one per commit.
  Complete production edits, focused correctness checks and targeted profiling
  before starting the qualification round; individual commits do not each
  require a full timing campaign.
- Declared ulp-scope changes: affected fields named in the replay
  `--affected-scope`, all other fields and discrete decisions exact, independent
  witnesses on the affected fields and instruction counts within noise. They
  reuse the prior timing evidence.

`bench/time_exact.py` retains the historical seven-alternating-pair procedure
and its paired-median bootstrap summaries. Existing accepted evidence remains
valid. New qualification uses the controlled block protocol below; do not mix
its new estimand with historical paired-ratio samples. The frozen standard
workloads are regenerated and hash-verified by `bench/make_standard_cases.py`,
using the clocked harness from `bench/build_timing_harness.sh`.

### Controlled timing protocol (approved 2026-10-14)

Ordinary macOS services and desktop bursts are recorded measurement noise.
Their process CPU readings alone neither reject admission nor abort a whole
panel. Deliberate competing builds, test suites, substantial estimation jobs
and agent computation still do not share the measured host. Record host
conditions with a lightweight monitor. Sleep, severe memory pressure, verified
competing compute or other host faults may invalidate an affected complete
measurement block only under rules frozen before collection. Retain excluded
and incomplete blocks with reasons and supporting evidence. Never classify a
block using its candidate/reference timing outcome. Replacement attempts, if
allowed at all, count against a fixed declared cap. A failed scientific check,
solver timeout or changed artifact is not an OS-noise exclusion.

For a reference/candidate comparison, use balanced randomized ABBA/BAAB blocks
of complete native estimations, including setup. Freeze inputs, artifacts,
worker counts, warmups, cache policy, order seed, block/attempt count and wall
budget before timing. With several protected references, execute one randomized
arm permutation and its reverse, balancing orientations across blocks. Shared
reference calls are recorded once. A separate discovery experiment may inform
which candidate to freeze, but is not a required stage of qualification.
All arms retain the same scientific and resource checks. Checkpoint complete
blocks so an unrelated later interruption does not erase earlier valid work.

Let B and N be the geometric means of the two reference and two candidate
complete-solve times in a block, in seconds. Acceptance tests the median of
E = N - B - max(0.05 B, 0.001). The median block ratio N/B is a separate
descriptive effect-size summary, not the tested quantity. Use the attainable
one-sided order-statistic upper bound with upper-tail alpha 0.025: the smallest
rank k for which P[Binomial(n, 0.5) >= k] <= 0.025 gives upper bound E_(k).
If no rank is attainable, the upper bound is infinite and the result inconclusive.
Pass only if the upper bound is at most zero; retain ties. This coverage is
conditional on independent block scores with a common median, not an
unconditional guarantee about a noisy host. Freeze round/session identities and
the actual analysis unit before collection. Report ordinary
means, upper-tail behavior and minima or low quantiles as descriptive diagnostics;
minima cannot establish acceptance. Implement max(5%, 1 ms) consistently with
the chosen block estimand, accounting for reference-time uncertainty rather
than treating an estimated denominator as known. Name the estimand and exact
analysis rule in the frozen protocol. Preserve pairing and shared-reference
correlations in uncertainty calculations. State independence, common-distribution
or common-median assumptions; a few sessions containing many correlated solves
do not provide many independent observations. Approximate intervals must be
labeled as such, not presented as exact guarantees.

A small fixed A/A pilot on representative costs and worker counts can diagnose
order effects, dependence and attainable precision before candidate timing.
It cannot establish independence or common medians from a small sample.
Use it to choose a practical measurement budget, not to widen the allowance.
Freeze one candidate before the qualification round. No fallback winner or
repeated sampling until success is permitted. An unresolved required comparison
leaves promotion inconclusive and requires discussion with Johannes; it does
not silently pass or create a benchmark-specific routing exception.

Use verified isolated SCC allocations for broad algorithm discovery, scaling
and Linux performance evidence. Candidate and reference share the same node,
CPU placement, physical-core/SMT policy, library thread limits and input bytes.
Record allocation and host identity; requesting slots alone does not prove
whole-node isolation. Focused Mac qualification covers the relevant workload
families and worker counts, particularly concurrent builder/solver routes.
Linux results do not establish Mac speed or justify automatic promotion of the
Linux winner. This protocol changes performance measurement only; scientific
tolerances, model semantics and transaction checks are unchanged.

Earlier failed/excluded timing attempts retain their original status. Do not
retroactively apply this protocol to rescue them. Reuse valid scientific and
performance evidence when its artifacts and affected scope remain applicable;
a newly changed public-default artifact still requires its mapped checks.

The current runner has one fixed budget and no automatic timing continuation.
An operational repair may resume untouched work under the same unchanged plan
while retaining already completed cases. Do not replace partial or unfavorable
measurements with a new attempt. Changing artifacts or measurement inputs requires
reassessing evidence validity; unresolved cases go to Johannes with the actual
failure or uncertainty. Historical continuations keep their recorded scope and
outcome and impose no obligation to rerun those campaigns.

Johannes's 2026-10-11 clarification makes performance coverage proportional to
measured cost. For FMA work, inspect complete executed library-call totals and
their instruction contribution, but a handful of constant-per-fit calls outside
hot loops need not trigger another implementation and full timing cycle. Record
the remaining contribution and justify its negligible cost; do not infer absence
from a top-functions listing. This supersedes a literal zero-library-call reading
of [performance-plan B](fable_performance.md#b-compile-the-remaining-mul_add-loops-to-hardware-fma-preserving).
The already qualified B artifact and all earlier observations remain unchanged.
Exact replay, independent scientific certificates, supported portable execution
and the standard wall-time allowance remain required.

Mac release performance claims require controlled paired Mac timing. For it,
run no competing builds, tests or agent tool workloads on the measured host.
Independent analysis on separate infrastructure may continue only if it neither
consumes measured-host resources nor reads or changes the running artifacts.
Coordinate unrelated workloads without terminating them. Retain unsuccessful
observations until the experiment is closed. Missing or inconclusive evidence
never passes a claim it cannot support. Report costs
as well as gains; the per-workload allowance remains max(5%, 1 ms).
No ongoing obligation to rerun retired historical campaigns exists.

Native distribution remains subject to [licensing](licensing.md).

Report slow workload families, memory, failed cells and scaling alongside aggregate
time. Unexplained material scaling cliffs fail qualification. Changing the default
max(5%, 1 ms) allowance requires Johannes's decision before the affected implementation.

<a id="complexity-inventory"></a>
The complexity inventory counts runtime Rust source after removing whitespace,
comments and test-only items. It follows declared modules, including non-test cfg
variants. Per-crate lines, long files/functions and sensitive dispatch conditions
ratchet; decreases lower the inventory and growth needs exact scoped reasons.
The inventory was relocated unchanged from the completed program; its historical
growth explanations remain attached to the same measurements. Refresh it with
`python3 scripts/check_complexity.py --write-inventory FILE` and copy the result
into `tests/reference/complexity.json`; growth entries record
`{metric, before, after, reason}`.

## Scientific validation

### Ownership and cell-operator investigation (2026-10-23)

Johannes authorized implementing the recovered ownership and first-occurrence
cell ideas against `c16c0b55`, with one selected combined finalist and one final
qualification. The [declared scope](SPEC.md#ownership-and-cell-operator-experiment-scope-2026-10-23)
precedes implementation. Preserve current public behavior during discovery and
all historical failures. Internal developer injections independently select
moment omission, projected-score ownership and eager/deferred cells; there is
no new user option or solver and no CMG policy retuning.

Discovery regenerates the twelve historical families (weak, high-degree weak,
dense, repeated, nonmovers, hubs, disconnected, dynamic weights, wide,
inactive-zero, Low and easy) at W1/W4 with data seed `2026106301`. Each panel
uses one isolated SCC host, one warmup per arm/cell and six randomized mirrored
blocks, complete native-command times, 120 seconds per call and 3,600 seconds
per panel including warmups and correctness checks. Panel 1 compares baseline,
moment omission, score ownership and both. Panel 2 compares baseline, selected
ownership O, eager cells S and O+S. Only measured wasted eager construction or
a complete-command regression permits panel 3: baseline, O, O+eager S and
O+deferred S. Order seeds are `2026106303`, `2026106304`, `2026106305`.
Deduplicate identical arms before execution; each four-arm panel has at most
1,248 calls. Separate diagnostics allow at most three traced calls per arm/cell
and 600 seconds and never replace scored observations.

Ownership admission requires exact science/signatures, demonstrated eliminated
work and no material discovery regression beyond max(5%, 1 ms). Savings below
that allowance can support ownership only when removed work is independently
verified. Cells additionally require a material complete-command gain over the
corresponding no-cell arm and no material regression. Among eligible
configurations, minimize the mean paired complete-command time difference from
baseline, equally weighting all 24 settings; exact ties prefer fewer changed
mechanisms. Investigate the retained inactive-zero W4 ownership regression with
the independent moment/copy arms, copy/allocation counts and downstream access.
Scientific failures stop dependent work. Unexplained ownership regressions,
exhaustion of the single cell redesign or inconclusive qualification require
discussion. No unfavorable observation is removed and no unchanged artifact is
resampled for a pass. Retain diagnosed operational failures and bounded repairs.

Final qualification uses `bench/qualify.py`, original public `f35f3a4d`, accepted
public `c16c0b55` and one frozen candidate. The standard 42 cells gain eight
bodies at W1/W4: nonmovers (20,000 rows/1,000 workers/100 firms), hubs
(64,000/1,000/100), inactive-zero (20,000/1,000/100), three robust designs
below/at/above the actual 32 MiB packed-cache boundary, a 600,000-row repeated
cell design and a 480,000-row weak design. Freeze shapes, witnesses, hashes and
schedule before candidate execution; an oracle failure stops without resizing
or substitution. Qualification data/order seeds are `2026106302`/`2026106306`.
Linux uses twelve distinct physical hosts per cell, two mirrored blocks per
host, three shards and one warmup per arm/cell/host (at most 10,440 calls).
Mac uses 24 mirrored blocks and one warmup per arm/cell (at most 8,526 calls).
The per-call limit is 120 seconds; application budgets are 3,600 seconds per
Linux shard and 14,400 seconds on Mac. The maintained external-interference
rules apply. The 58-cell Mac schedule already consumes all 8,526 authorized
calls, so this prospective round has zero replacement-call allowance; the
historical 42-cell recovery contract is unchanged. Retain externally invalidated
blocks and discuss any resulting inconclusiveness without adding observations.
Failures and slow timings never trigger sampling. All protected one-sided alpha .025 order-statistic upper bounds for
`candidate - baseline - max(.05 * baseline, .001)` must be nonpositive, using
Linux physical hosts as independent units. Report effect sizes separately.

The eight added bodies use data seed `2026106302`, with the family index added
to the seed in the order listed above. The three cache-boundary bodies share
one K16 draw at seed `2026106305`: 4,096 workers, 100 firms and 64 periods.
Their 262,143/262,144/262,145 rows remove the final row, retain the draw, or
append its exact final row. Their packed regressor banks are 32 MiB minus
128 bytes, exactly 32 MiB, and 32 MiB plus 128 bytes. All three are
moment-ineligible under the conservative K+2 construction bound; they test the
boundary without being counted as positive moment-omission coverage. The
nonmover body retains the first 100 workers' mobility and fixes each remaining
worker to its worker-index-modulo-100 firm. Hubs use the maintained d60 design;
inactive-zero uses four paired orthogonal regressors without a structural
constant. The large repeated body repeats each row of a 5,000-worker,
100-firm, 30-period long panel four times. The large weak body has 6,000
workers, 100 firms and 80 periods. All eight use unweighted robust OLS, Fast
memory, truthful `candidate_all=1` and frontend-consistent `input_complete=0`.

Preparation of these additions has one separate four-slot SCC allocation at
3 GB per core, a 1,800-second application budget and a 2,100-second scheduler
budget. It performs no native estimation. Each independent reference has a
120-second limit; only the boundary triplet uses the declared 2 GiB oracle
allowance. The other numerical references retain the default 1 GiB allowance,
and inactive-zero uses the maintained analytic witness. Verify actual retained
row counts and full finite rank, with no resizing, tolerance change or
substitution after failure. Existing input/reference files are not rewritten.
The previous qualification's committed input/reference hashes remain the
binding for the original 42 cells; missing historical scratch is not permission
to substitute a different request or witness.

The predecessor's scratch was removed after its completed activation. Recovery
of its original 42-cell inputs is therefore a separate byte-restoration step,
not a new reference-selection exercise: use the retained `88ab8fb5` Python
generation/oracle source, NumPy 2.2.6 and SciPy 1.16.3, seed `2026105201`, and
the original four replay imports. Record the actual serializer binaries used
for recovery. Accept a recovered request, metadata document or reference only
if its complete SHA-256 equals the prior committed freeze; no approximate
comparison or replacement witness is admitted. Metadata can be restored from
the retained document content, preserving its historical provenance. The step
has a 2,400-second total budget, at most 300 seconds for input reconstruction,
120 seconds per numerical reference, one attempt per file and zero native
plugin calls. Preserve a mismatch or failure and stop the affected final
qualification work rather than trying alternate seeds, inputs or references.

The bounded recovery completed in 26.789 seconds with zero native plugin calls.
All 78 request, metadata and reference files matched the previous freeze
byte for byte. The [restoration record](../bench/results/ownership-original42-restoration-20261023.json)
retains each hash, the actual serializers, pinned numerical environment, source
manifest, helper, commands and budget use. No witness was substituted.

Before final timing, freeze a separate 80-call diagnostic schedule on each
platform: the eight added bodies at W1/W4, with accepted `c16c0b55` once,
uninjected candidate twice, and the selected hidden finalist twice. Its budget
is 2,400 seconds per platform, 120 seconds per call, with zero retries or
replacement calls. It is separate from the three-arm timing caps and the
174-call activation checks. The uninjected candidate must show positive actual
moment construction on eligible W4 bodies and the selected finalist must omit
that work; the old accepted binary need not emit a newly added diagnostic.
W1 and the cache-boundary controls must retain their expected ineligibility.
Require independent witnesses, exact ownership science/signatures, the declared
cell scope if selected, exact fixed-worker repeats, cross-worker agreement and
raw resource caps. These diagnostics attest execution and never replace a
complete-command timing observation. The boundary W4 controls retain positive
raw-moment construction in both arms with zero omission; their ineligibility
applies to omission, not construction. Score diagnostics also require actual
positive copy avoidance below and at the packed-cache limit, and the declared
fallback above it, at both worker counts. Generated/kept widths, avoided
elements and the full retained matrix capacity must match the frozen shapes.

The separate diagnostic runner passed 31 focused tests, including nine runner
tests, with independent high-effort implementation review and bounded Astra
review. Its log SHA-256 is
`37d43b7e0e1ae6b872e494cc068578ee24b652e81070b2d89da9c94976f154e3`.
These checks use synthetic executions; the real 80-call prerequisites remain
unexecuted until the candidate and final manifest are frozen.

Preparation hit one shared packaging defect: the new original-baseline and
input-preparation archives omitted pinned vendored Rayon source. Both failed
before compilation, inputs, references or native estimation. The exact vendor
files were added from the same respective revisions; existing source bytes
were unchanged. The repaired protected-original Mac build passed native audit,
ABI/load and panic containment. The separately prepared SCC input repair
charges its failed job's eight seconds against setup and scheduler budgets,
leaving 172 setup seconds, 1,800 input/reference seconds and 2,092 scheduler
seconds. The [operational record](../bench/results/ownership-qualification-preparation-repair-20261023.json)
retains both failures, source identities, budgets and successful Mac build.

The repaired input allocation completed with zero failed/exit status in 196
scheduler seconds. All eight bodies retain their declared row counts and full
finite rank; all 30 input, metadata and reference files were collected with
identical hashes for Mac. The protected-original Linux build completed in 177
scheduler seconds with actual native audit, ABI/load and panic-containment
checks. The [completed preparation record](../bench/results/ownership-qualification-preparation-complete-20261023.json)
retains both accountings, full input metadata and numerical-reference documents,
source/build identities, and the enlarged Mac/Linux harness bindings. These
steps performed no qualification timing and do not select or activate a
candidate. The earlier packaging failures remain unchanged.

The prospective tooling was checked before generating these inputs: 76 focused
and historical qualification/input tests passed, including actual request
framing, strict per-family shape/seed/weight/reference-budget rejection,
scoped-science failures, three-role source/build binding and unchanged old
qualification contracts. Independent high-effort and bounded Astra reviews
completed; the latter identified the per-family validation gap, which was
repaired and rechecked. The source-only suite passed 2,029 Rust checks,
315 Python tests and the portable checks. Its log SHA-256 is
`80cc4b94f9be14e989a9d4ca74b6468c91f753f43c1fb4a47597a370654dc5e7`;
the final focused-check log SHA-256 is
`2f67c19af712a4a3a668939b165e6b1bf2ef02da047836593d1da7705395724a`.
This validates preparation tooling only. Actual final inputs, diagnostic proof,
candidate selection, measured qualification and public integration are separate
remaining gates. The source-only run does not establish installed-package
admission, which remains the recorded failed prerequisite for release work.

Before activation, run affected source, native ABI/load/panic, mapped Stata,
inference, transaction and Auto Break checks; reuse valid unaffected evidence.
The hidden/public/public-repeat trio covers all 58 cells on each platform
(174 calls/platform), including exact science/signatures and resource/gate
evidence. Derive this experiment's actual hidden transport difference; the old
51-byte amendment is not reusable by assumption. Independent architectural
review precedes performance measurement. Preserve raw call objects losslessly
in compact records under 1 MB, with failed attempts, hashes and accounting.
Only a passing finalist may become public on `codex/auto-routing`; final failure
or inconclusiveness retains the accepted public behavior and cannot silently
substitute another component. Commit/push with `[skip ci]`, then remove task
scratch after retained evidence is verified. Leave no monitor at completion.

The [completed ownership discovery panel](../bench/results/ownership-cells-panel1-20261023.json)
retains all 1,248 calls, 96 separate diagnostic calls, controls, operational
failures, accounting and independent assessment. On one isolated Linux host,
score ownership reduced complete-command time by a mean 0.406 ms, equally
weighting the 24 settings; the combined arm reduced it by 0.421 ms. These are
discovery point estimates, not qualified platform speedups. All 72 paired
comparisons stayed within the material-regression allowance at their point
estimates, with exact ownership science/signatures and no scored retries or
exclusions. Diagnostics verified omitted score copies in 20 of 24 settings,
but zero omitted moment work: every frozen input had `candidate_all=0`, so
the loaded RHS/moment bank was unavailable. The combined arm's nominal 0.016 ms
edge does not demonstrate a moment-omission benefit. At that point Panel 2 awaited Johannes's
interpretation of whether admission requires demonstrated work removal for
each enabled mechanism. The frozen observations and selection rule remain
unchanged, and neither new mechanism is publicly enabled.

The optional eager-cell implementation at `40d7e1ae` also passed the affected
full source suite, actual Mac/Linux native ABI/load/panic checks and 48
uninjected correctness controls per platform against `c16c0b55`; all 24 pairs
per platform matched exactly and passed independent numerical witnesses.
The [platform check record](../bench/results/ownership-cells-eager-platform-checks-20261023.json)
retains the Linux calls and final 0/0 accounting and links the retained Mac
calls. These controls establish unchanged public behavior while the experiment
is disabled. They do not qualify the injected cell operator or establish a
performance gain.

Johannes's subsequent 2026-10-23 amendment explicitly selects both moment
omission and projected-score ownership as configuration O after closing the
moment-coverage gap. The original panel and its conclusions remain immutable;
the nominal difference between scores and both is no longer a selection question.
The separately identified `ownership-moments-coverage-v1` supplement reuses the
frozen weak, wide and positive-weight data, retaining original and changed
request hashes. Every supplementary request truthfully sets `candidate_all=1`
and keeps `input_complete=0`. Seven requests give twelve settings: weak/wide
crossed with robust/one-way clustered inference at W2/W4, plus weak robust W1,
weak robust Low W4, weak IID Fast W4 and positively weighted robust Fast W4.
All other requests use unweighted Fast OLS. Clustering uses the first FE's
existing numeric source with `account_cluster_nesting=0`; its independent CR1
covariance, cluster counts and degrees of freedom are bound to the new request.

Before timing, execute one accepted `c16c0b55` reference and two diagnostic
calls for each of baseline/moments/scores/both in every setting: exactly 108
calls, 120 seconds per call and 1,200 seconds total. Require exact science and
solver signatures against the matching uninjected baseline and accepted
reference, independent numerical witnesses, fixed-worker repeats, cross-worker
agreement within the same model, and unchanged moment-related admission and
transaction behavior. Retain and validate raw memory caps; compare moment-pair
admission after subtracting only the source-defined injected-argument parent
delta (one 24-byte String descriptor plus the UTF-8 argument bytes on these
64-bit hosts). Available-memory limits remain observations. Compare routing
decisions on every diagnostic repeat. Every eligible baseline/scores call must attest a
successfully constructed speculative cache at the actual construction boundary;
moments/both must omit the corresponding positive work without construction.
Counters distinguish workspace bounds from actual retained memory. W1 and
weighted controls construct/omit no moments; IID retains its needed moments;
Low preserves its baseline construction state. Low/IID/weighted controls do
not admit score ownership. Failed preflight blocks all timing.

The supplementary timing schedule has one warmup per arm/setting and six
randomized mirrored blocks: 624 calls, 120 seconds per call and 2,400 seconds
total, order seed `2026106307`. One isolated SCC host runs the frozen sequence;
there are no retries or exclusions. Use all complete-command observations and
the existing per-setting mean-excess nonregression requirement, max(5%, 1 ms).
Report descriptive 95% percentile intervals from 4,096 paired-block bootstrap
draws with seed `2026106308`, resampling all arms/settings jointly by the six
mirrored blocks. These are conditional single-host intervals, not activation
qualification. Phase timing and removed-work evidence explain mechanisms;
neither replaces complete-command measurements. Then run Panel 2 with O=both.
Final qualification retains the original 42 requests and uses truthful
`candidate_all=1`, `input_complete=0` on the applicable eight added numeric
bodies; its frozen manifest and diagnostic schedule must demonstrate positive
moment omission on eligible multiworker cases before an activation claim.
Record the exact manifest/budget amendment before collecting that qualification.
Scientific failure, inability to exercise the optimization, unresolved material
regression or inconclusive final qualification still requires discussion.
The supplement implementation passed the full source-only suite (2,029 Rust
checks and 315 Python tests), 42 focused ownership/oracle runner tests and the
complexity ratchet. The seven additional production lines only attest successful
moment construction under phase diagnostics. Independent review checked the
IID/CR1 mathematics, construction boundary, transport-parent normalization,
request bindings and repeated route/resource checks. These checks establish
readiness for the fixed experiment, not a speed or qualification result; the
installed-package binding remains outside this branch-only source admission.

The first supplementary preflight, SCC job 8261951, failed on the accepted
reference's first clustered request after 45 successful robust diagnostic calls.
The harness incorrectly specified `account_cluster_nesting=0`; the frontend
and guarded public entrypoint require `1` for clustered OLS. The original
protocol above, failed invocation and all observations remain retained in the
[failed preflight record](../bench/results/ownership-moments-preflight-failure-20261023.json).
No supplementary warmup or scored timing call ran. This is a diagnosed request
construction error, not evidence of an optimization's scientific failure.

The bounded continuation changes only that flag on the two clustered requests
and their independent nesting-aware references. Numeric data, model tables,
native artifacts, launcher, arms, seeds, tolerances and timing schedule stay
fixed. The five noncluster inputs, metadata and numerical references are reused
byte-for-byte. The oracle separately checks true projection rank and the existing
OLS nesting-adjusted DoF policy: with first-FE categories L nested in G=L
clusters and nonnested second-FE categories R, A=R−1, D=N−K−A, fit DoF=D−L,
reported DoF=min(D,G−1), and CR1 multiplier=(N−1)/(D−1) × G/(G−1).
These counts and nesting relations are verified from the retained sample.

Reuse exactly completed schedule entries 0–44, preserving their original
source/result identities and raw output/log/array hashes. Execute entries
45–107 once on the same physical host, scc-ua1: 63 new diagnostic calls,
108 successful entries if complete, **109 total attempted invocations** including
the retained unsupported request. Deduct 36 seconds (ceiling of 35.772254411)
from the 1,200-second diagnostic budget, leaving 1,164 seconds; deduct the
48-second failed allocation from the 4,200-second scheduler budget, leaving
4,152 seconds. The existing 120-second per-call cap remains. Only after the
combined preflight passes exact science, independent mathematics, resource,
repeatability and actual-work checks may the unchanged 624-call timing panel
run once. Original Panel 1 remains unchanged and configuration O remains both.
The repair passed 450 benchmark Python tests: 436 in discovery, then the 14
reporting tests after installing the isolated environment's missing documented
dependencies. The first discovery log remains failed for two import errors;
its SHA-256 is `8049b5dee0dc66a7ef6d85aed2d295d43b8c3a2c46b5612fca3fe17b335f0390`.
The reporting check passed with log SHA-256
`d53da78472b3878d117c853fb9423f7d879d3a3a4bf61bff0c276e6c10b982a0`.
These include five continuation/tamper tests and independent dense/pinned
cluster witnesses. Production identity is unchanged across 200 runtime modules
and 58 artifact inputs (check log SHA-256
`357ba8b68526cf9774f7f813c792fbfff997107269de2b6349d9feb25f6418dc`),
so the already valid source-suite, native ABI/load/panic and public-control
evidence applies to the same binaries. No native call was added by these checks.

The [completed supplementary coverage](../bench/results/ownership-moments-supplement-20261023.json)
closes the actual-moment-work gap. Job 8262137 completed the 63 remaining
diagnostics and all 624 timing calls, with 0/0 accounting, 884 scheduler
seconds and reported `maxvmem=915.008M`. The combined preflight has
108 successful entries, including the 45 reused entries; its original failed
invocation remains failed. Separate assessment job 8263568 independently
reconstructed 108 full-column references and all 36 paired comparisons, with
0/0 accounting and zero native calls. Exact science/signatures, fixed-worker
repeats, cross-worker agreement, raw resource caps, observed isolation and
the declared construction/omission controls passed. All eight eligible settings
show positive baseline moment construction and corresponding omitted work;
all four boundary/ineligible controls preserve their required behavior.

On this one Linux host, the equally weighted mean complete-command reductions
over twelve settings were 2.085 ms for moments, 1.200 ms for scores and
3.424 ms for both. Combined reductions in the eight eligible settings ranged
from 1.099 to 9.468 ms; the four wide-design settings saved 8.079–9.468 ms
(about 9.3–11.2% by their mean paired ratios). All 36 comparisons met the
declared discovery nonregression rule. The retained six-block bootstrap
intervals describe uncertainty conditional on this host, not a qualified
platform-wide speedup; controls include small positive timing differences.
No warmup or scored observation was retried or excluded. These results admit
the explicitly selected configuration O=both to Panel 2; they do not activate
either mechanism or waive the final Mac/Linux qualification.

The [completed Panel 2](../bench/results/ownership-cells-panel2-20261023.json)
retains all 1,248 timing calls and 96 separate diagnostics. Job 8263668 and
independent assessment 8264526 both have complete 0/0 accounting. The assessment
reconstructed 192 full-column reference sets, verified exact ownership and
scoped cell science, fixed-worker repeats, cross-worker decisions, original
hierarchies, resources and observed whole-host isolation. No call was retried
or excluded. O remained both ownership mechanisms and had no adverse mean
against B. The equal-weight complete-command differences against B were
−0.604 ms for O, −4.532 ms for eager cells, and −5.080 ms for O+eager.

Eager cells nevertheless fail the workload nonregression requirement. Their
incremental comparison against O contains these adverse means:

| Setting | O+eager minus O | Excess beyond max(5%, 1 ms) | Positive-excess blocks |
| --- | ---: | ---: | ---: |
| Dense W4 | +4.357 ms | +2.023 ms | 4/6 |
| High-degree weak W4 | +5.613 ms | +2.034 ms | 6/6 |
| Hubs W1 | +7.890 ms | +1.988 ms | 6/6 |
| Hubs W4 | +18.638 ms | +14.813 ms | 6/6 |

The incremental equal-weight mean of −4.476 ms cannot offset those regressions.
The [independent assessment](../bench/results/ownership-cells-panel2-assessment-20261023.json)
retains all paired blocks; this one-host panel reports their variation, not
confidence intervals or activation bounds. Both independent reviewers confirmed
the means directly from the retained calls. Cell construction without CMG
installation does not prove waste because eager cells may serve the incumbent
operator. The measured complete-command regressions themselves satisfy the
predeclared trigger for the single deferred-construction redesign.

Panel 3 therefore keeps O=both and the declared B/O/O+eager/O+deferred schedule,
order seed `2026106305`, 1,248 timing calls, and one 96-call diagnostic schedule
within the existing 3,600/600-second application budgets. The new deferred
artifact must construct only following successful CMG commitment at the existing
restart, with real coexistence admission and unchanged fallback/cancellation.
Fresh 48-call uninjected correctness controls per platform have a 600-second
budget and no retries. Bind the preserved Panel 2 result and independent
assessment separately from the changed candidate; requiring their binary hashes
to match would incorrectly forbid the authorized redesign. On the same Linux
host, the already scheduled Panel 3 warmups must exactly match the prior B, O
and O+eager science/signatures and resource/gate/hierarchy evidence. These checks
add no timing calls. Panel 2 remains failed for eager admission; Panel 3's
reference arms do not replace or improve that conclusion. Only the single new
deferred configuration is eligible for new cell admission. Final qualification
and public activation remain pending.

The [deferred implementation checks](../bench/results/ownership-cells-deferred-development-20261023.json)
cover the shared constructor at both successful CMG restart boundaries, joint
CMG/cell admission, immutable weight ownership, exact owner layout, refusal,
cancellation and independent reconstruction. A forced certificate-correction
witness distinguishes historical fixed-checkpoint installation from closed
GATES. Public defaults remain unchanged. The full source suite, 23 benchmark
tooling tests and six affected Stata profile checks pass. The initial fixture
and tooling failures remain recorded. The new licensed Stata fixture is still
pending native builds; its scope is transport, independent dummy OLS and
publication, not proof of cell construction.

Before timing, build each native artifact once with an 1,800-second build
limit and run the fresh 48-call correctness controls above. Linux preparation
uses a separate four-slot, 3 GiB/core allocation with a 2,700-second scheduler
limit covering the build and 600-second controls. Panel 3 stays on the original
physical host with the whole 28-core allocation. Each traced deferred warmup
and diagnostic must pair successful CMG installation with one cell attempt in
the same action; construction before commitment or in a later action fails.
The separate four-slot assessment retains its 600-second analysis and
900-second scheduler limits, with zero native calls. Actual candidate hashes
and the manifest remain to be frozen before execution.

The subsequent tooling amendment strengthens this prospective panel without
changing its native candidate or sampling schedule. Deferred traces must contain
raw-capped action records, close each installation/construction pair before its
action ends, and match the corresponding ownership control's action count,
projection/final classification, preset and native iteration limit. Deleting an
action record or truncating the last action is rejected. Panel 3 uses a separate
execution archive with only the two panel tooling modules changed; the original
native archive remains immutable. The completed uninjected controls are reused
only after their original module hash is bound and AST comparison confirms that
all control-reachable code is unchanged. Only the four non-control entrypoints
`freeze`, `run`, `report` and `compare_warmups` may differ, together with the
Panel 3 module that controls do not execute. No control or timing observation is
repeated by this amendment.

Prospective final qualification also accepts `cells_deferred_both`, alongside
the existing ownership and eager requests. Its actual argument framing adds
50 bytes; raw memory limits are checked before accounting for that difference.
Strict deferred trace and action checks apply to eligible two-FE cases, against
the accepted control, while unaffected cases retain exact comparisons. These
tooling changes do not select a finalist or alter the 58-cell budgets.

The [completed deferred platform checks](../bench/results/ownership-cells-deferred-platform-checks-20261023.json)
retain 48 fresh uninjected controls on each platform, both actual native
audit/ABI/load/panic checks, six mapped Mac Stata fixtures and the existing OLS
and optional-builder Break boundaries. Linux preparation job `8265208` built
successfully but inherited an ABI-only environment setting into its first
replay launch. The harness rejected it before loading the plugin. That failed
launcher and 0/1 accounting remain retained. Repair `8265280` reused the binary,
unset the setting and completed all 48 previously unexecuted controls with 0/0
accounting; five seconds were deducted from the original 600-second control
budget. No successful native observation was repeated. The new cell transport
fixture does not establish actual deferred construction, and the old Break
adapters do not establish activated new public mechanism coverage.

The [completed Panel 3](../bench/results/ownership-cells-panel3-20261023.json)
retains every 1,248 timing call and 96 diagnostic calls losslessly. Jobs
`8265412` and `8265799` have complete 0/0 accounting. The independent assessment
reconstructed all 192 reference sets, verified 72 historical warmups exactly,
and passed scoped science, repeatability, resources, same-action deferred
construction and host observation checks. No observation was retried or excluded.

O+deferred has no adverse mean beyond max(5%, 1 ms) in any of the 24 settings,
against either B or O. Its equally weighted complete-command difference is
−1.279 ms against B and −0.382 ms against O. Only weak W1 demonstrates the
required material gain over O: −2.810 ms, exceeding its allowance by 0.0695 ms.
All six paired blocks are faster, but only four exceed the allowance; this
narrow one-host discovery result is not a final confidence bound. Eager cells
retain four adverse settings and remain ineligible. Deferred construction occurs
in ten settings following ten CMG installations, with six settings reporting
reuse; those counters do not independently establish causal kernel benefit.

Independent high-effort and bounded Astra reviews support the
[sole finalist selection](../bench/results/ownership-cells-finalist-20261023.json):
`cells_deferred_both`, with O still both ownership mechanisms. It satisfies the
declared discovery rule and has the lowest equal-weight mean among eligible
configurations. Public behavior remains unchanged. The fixed 58-cell Mac/Linux
qualification, real 80-call mechanism diagnostics on each platform, and exact
public integration remain required; the narrow discovery gain neither waives
those gates nor permits further sampling or another redesign.

The [final qualification freeze](../bench/results/ownership-cells-qualification-freeze-20261023.json)
binds that sole candidate to both 58-cell manifests and both separate 80-call
diagnostic schedules before native execution. The original 42 inputs and
references remain byte-exact; all eight additions passed full input validation.
The Linux manifest preparation job `8265884` completed with 0/0 accounting and
zero native calls. The record preserves the two pre-native local preparation
defects and their narrow repairs, actual source/build bindings, budgets and
wrappers. Timing requires a successful proof from the exact platform diagnostic
schedule, including terminal Linux accounting. No qualification result or public
activation is established by this freeze.

The final coverage preparation retains the original 42 requests and freezes
the additional packed-cache boundary triplet at 16 finite regressors and
262,143 / 262,144 / 262,145 rows. Start with one 4,096-worker, 100-firm,
64-period numeric design; remove its last row for the lower case and append
one copy of its last row for the upper case. The exact middle body is unchanged.
The independent reference must verify the actual retained row counts and full
finite rank, so the packed score sizes are respectively 32 MiB−128 bytes,
32 MiB and 32 MiB+128 bytes. These are boundary controls: the conservative
moment-omission eligibility bound includes the outcome and intercept columns,
and exceeds 32 MiB in all three cases. Positive moment omission must be
demonstrated on the other eligible added bodies, never inferred from these
boundary labels. Data seed `2026106302` remains fixed; request hints remain
truthful `candidate_all=1`, `input_complete=0`.

Before generating these references, declare a separate 2 GiB modeled workspace
allowance for the independent oracle, within a four-slot, 3 GiB/core analysis
allocation. Its existing 1 GiB default and historical references remain
unchanged. The 2 GiB option is explicit in newly generated reference
certificates and bound by the qualification manifest. The arithmetic,
certificates, numerical tolerances, dimension/work limits and 120-second
per-reference limit are unchanged. This is a reference-generation resource
allowance, not a change to production memory admission or to scientific gates.
The previous modeled bound alone would reject an actual 32 MiB packed case;
the allowance is declared prospectively, with no failed generation, resizing,
reference substitution or additional timing sample used to obtain a pass.
Independent review found no blocker in the allowance or boundary definition.
Validation passed all 16 oracle tests, 54 qualification-tool tests and the
source-only integration suite (2,029 Rust checks, 315 Python tests and the
portable/static checks). Their log SHA-256 values are respectively
`f0774b2cbb9563aed9c6a491f5e358dd8197a20ef2133921dedfce286f8c33e4`,
`4cf7a1b9180cb1dca1503523adcd7296b7524d66fd08f5d1db7392bca0177d89`,
and `07e73a043dbbce8afa2b0c57130fc7dcf2e07e2adbe214bb873dc22392f480fb`.
The first source-check invocation passed its 315 Python tests but failed before
Rust because Cargo was absent from the shell PATH; its failed log is retained
by SHA-256 `fce20a38c0a4f1c2707aa4d948256e312e50fbb498f81191246ba53aea1686b4`.
The successful invocation used the pinned Rust 1.85.1 toolchain. All 200 runtime
modules and 58 artifact inputs remain identical to `3a554699`; existing native
ABI/load/panic evidence applies. Installed-package admission remains unperformed
by this source-only check, and this preparation establishes no timing result.

[SPEC](SPEC.md) defines the scientific contract. [VALIDATION](VALIDATION.md)
records the accepted scope and caveats. No historical campaign is a substitute
for independent scientific witnesses on the responsibilities a change affects.

Dense Rust oracles and Decimal/Python/Mata witnesses cover coefficients,
prediction, covariance and PSD, rank and DoF, FE reconstruction, separation and
both mean-bias corrections. Licensed fixtures cover Stata syntax and metadata,
original factor slots/base flags, publication, rollback and Break. Actual-plugin
replay checks complete transactions, repeatability and stable thread decisions.

Arithmetic-preserving changes require exact unaffected results and available
route signatures. Declared numerical changes retain independent targets, fixed
seeds, deterministic repeats and exact results outside the affected scope.
Missing, failed or inconclusive evidence remains visible. Mac-only qualification
does not establish new Linux, SCC, Windows or Intel-host Stata evidence.
