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

The incremental Auto workstream uses `codex/auto-routing`. Its prospective
manifest is schema 3 with `experiment_kind=incremental`; historical routing
manifests keep their original evaluation. `routing_incremental_many` and
`routing_incremental_two` are hidden OLS qualification requests. Both public
defaults stay disabled until their separate qualification stages pass.
The original uninjected baseline is `f35f3a4d`. Later stages also protect the
most recently accepted branch Auto, and public activation separately protects
the selected hidden artifact. Explicit solver results describe opportunities;
their timings do not enter admission or selection.

[`bench/routing_stage.py`](../bench/routing_stage.py) prepares distinct, fixed
A/B/C inputs for each family, builds independent A references, and binds reviewed
source/build/scientific evidence to the SCC discovery coordinator. Its
[`routing_imports`](../crates/fereg-oracle/examples/routing_imports.rs) generator
supplies fresh no-FE, one-FE, slope and saved-effect inputs with independent
dense references. Run all private output under `target/tmp/auto-routing/`.
Preparation is not qualification: fresh allocation-local native science,
isolation checks, reviewed discovery, Mac B12 selection, exact hidden/public
equivalence and untouched Mac C24 confirmation remain mandatory. The Mac
panels time protected Auto comparisons only; the SCC explicit frontier remains
diagnostic. See the [prospective scientific scope](SPEC.md#incremental-auto-qualification-2026-10-17).

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
Task outputs (candidate plugins, A/B builds, worktrees, data, logs, profiles) go
under `target/tmp/<task>/` and are deleted when the task is reported, after
recording hashes and summaries; `scripts/clean_target.sh` lists everything else
in `target/` except Cargo's cache and `target/replay-baseline/`, and `--apply`
removes it. Keep large generated inputs and outputs out of Git.
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
  before starting the full panel. C/D is one such combined workstream; its
  subpoints do not each require a full timing campaign.
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
budget before timing. Shuffle workload order across rounds. For a finite
multi-arm discovery screen, a prospectively declared balanced rotating order is
also permitted; discovery scores do not substitute for finalist confirmation.
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
Use it to freeze a practical measurement budget, not to widen the allowance.
Separate discovery, shortlist selection and untouched finalist confirmation.
Choose one finalist before opening its confirmation results. Ties favor the
predeclared simpler/incumbent policy. No fallback winner or repeated sampling
until success is permitted after confirmation. An unresolved required comparison
leaves promotion inconclusive; it does not silently pass or create a
benchmark-specific routing exception.

Use verified isolated SCC allocations for broad algorithm discovery, scaling
and Linux performance evidence. Candidate and reference share the same node,
CPU placement, physical-core/SMT policy, library thread limits and input bytes.
Record allocation and host identity; requesting slots alone does not prove
whole-node isolation. Mac selection and confirmation cover the relevant workload
families and worker counts, particularly concurrent builder/solver routes.
Linux results do not establish Mac speed or justify automatic promotion of the
Linux winner. This protocol changes performance measurement only; scientific
tolerances, model semantics and transaction checks are unchanged.

Earlier failed/excluded timing attempts retain their original status. Do not
retroactively apply this protocol to rescue them. Reuse valid scientific and
performance evidence when its artifacts and affected scope remain applicable;
a newly changed public-default artifact still requires its mapped checks.

The following continuation rule applies to historical campaigns or a new
campaign that explicitly declares such a continuation before its confirmation
data. A fixed terminal confirmation cannot invoke it after seeing its outcome.
For an unchanged candidate, baseline, harness and frozen inputs, an inconclusive
cell may receive one bounded continuation without rerunning already-passing
cells. Predeclare eligibility, additional paired repetitions, host conditions,
total budget and the final analysis rule; preferably include them in the initial
timing plan. Retain all valid original and additional pairs, their pairing and
run identities, and analyze them together. Preserve the max(5%, 1 ms) allowance
and the confidence standard, accounting for any outcome-dependent selection or
repeated looks. A naive rerun until a bootstrap interval passes is not a valid
continuation. The existing timing tool reports individual runs; a combined claim
needs an explicit analysis implementing the declared rule, rather than replacing
the original summary with the most favorable run. A predeclared fixed larger
panel remains an alternative. If the artifact or measurement inputs change,
reassess evidence validity and qualify the affected behavior on the new artifact.

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
