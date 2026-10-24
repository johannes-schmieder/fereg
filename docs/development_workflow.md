# Development

Follow [SPEC](SPEC.md), [ARCHITECTURE](ARCHITECTURE.md) and [AGENTS](../AGENTS.md).
The repository has one runtime. Completed acceptance is in [VALIDATION](VALIDATION.md).

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

Use `bench/qualify.py plan`, `run` and `report` for a fixed candidate, current
inputs/references and a bounded schedule. See [bench](../bench/README.md#fixed-candidate-qualification)
for the interface. Record actual revisions/build settings and binary, harness,
input and reference identities once per job, with lightweight drift checks.
Do not add recursive history checks or prerequisite receipt chains.

Keep 42-cell routing coverage and add cases only for the affected mechanism.
Protect original and last accepted public Auto with max(5%, 1 ms). Require
independent numerical witnesses, exact/scoped science, resource gates, fixed-worker
repeatability and cross-worker agreement. Activation must match the selected
hidden invocation exactly. Explicit solvers and PPML remain covered by feature tests.

### Controlled timing protocol (approved 2026-10-14)

Freeze the candidate, balanced randomized complete-solve blocks, warmups, observation
policy and budgets before collecting results. Compare arms in the same allocation.
Linux is primary; use general eligible SCC nodes with verified whole-host isolation,
terminal scheduler accounting and physical hosts as the independent units.
Use practical paired Mac measurements and mapped integration for Mac behavior.
Ordinary desktop/OS noise stays in the observations. Keep competing builds, tests,
estimation and agent tools off the measured host. Exclude externally verified
interference only under the predeclared rule and bounded retry allowance.
Never discard unfavorable timings or retry scientific/native failures for a pass.

One report concludes the round. Preserve failures and discuss inconclusive science
or performance with Johannes; do not retune or resample automatically. Reuse valid
unchanged evidence. Arithmetic-preserving changes retain exact science/signatures
and may use instruction counts or function-level CPU samples with uncertainty.
Run broad suites at integration boundaries, not after each prose-only edit.

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

## Outputs and release maintenance

Put task outputs under one `target/tmp/<task>/` directory. Retain only current
inputs/artifacts and evidence needed to resume; delete superseded data and caches.
Commit concise results before final cleanup. Git history preserves old experiments;
ignored captures are not retained evidence. Keep the current replay baseline.
`scripts/clean_target.sh --apply` removes other task outputs when no active task
uses them. Delete expendable build caches when requested or disk limits require it.

Main is an installation source. Runtime changes need a matching native package,
source archive and actual installation checks before publication; follow
[release maintenance](release_process.md). Reuse completed performance evidence
when runtime production tokens and artifact inputs are unchanged. GitHub Actions
remain paused. Report failed or missing checks and platform limitations honestly.
