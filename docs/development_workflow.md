# Development

Scientific behavior is defined by [SPEC](SPEC.md); ownership and boundaries by
[ARCHITECTURE](ARCHITECTURE.md). Work on main in coherent commits and use
independent oracles to establish correctness. The repository has one runtime.

## Local checks

Python tooling uses Python 3.11 or newer. Scientific Python helpers require
NumPy, pandas and SciPy. Rust uses the pinned toolchain and Cargo.lock.

```sh
./scripts/run_tests.sh
./scripts/build_macos.sh
./ci/run_stata_ci.sh full
```

The full entrypoint covers Rust, Clippy, allocation/FFI audits, runtime complexity
and the mock SPI bridge. Licensed Stata profiles also include `smoke`,
`differential`, `fereg-regression`, `ppml-scientific` and `ppml-hidden`.
See [the Stata runner](../STATA_CI_RUNNER.md). Target mapped tests during development;
run the applicable integration checks before committing runtime changes.
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

Use `scripts/run_plugin_replay.sh` for real-plugin independent dense science and
fixed-thread repeat checks. A prior binary may additionally serve as an exact
regression comparator. It is not a second installed runtime or scientific authority.
Declare intentional numerical scope before changes; preserve unaffected results.
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
Remove temporary captures, profiles, copied source and obsolete builds after
recording useful results. Keep large generated inputs and outputs out of Git.
The compact benchmark entrypoint is documented in [bench](../bench/README.md).

Use two evidence tiers, with identified artifacts, predeclared workloads,
measurement budgets and stopping rules:

- Arithmetic-preserving changes require zero exact replay differences and
  unchanged solver signatures. Instruction counts or function-level CPU samples
  may establish reduced computational cost, together with no wall-time regression
  beyond max(5%, 1 ms) on the standard workloads. Instruction counts are
  deterministic for a fixed execution; CPU samples are statistical observations
  and must report uncertainty. Wall-time gains smaller than the allowance remain
  observations and do not by themselves fail an otherwise supported change.
- Result-changing changes require declared numerical scope, independent witnesses
  and the full quiet-timing protocol on the qualification host. Unaffected
  scientific results remain exact; affected results must satisfy SPEC.

Release performance claims require quiet Mac timing. For clean wall timings,
run no builds or agents and coordinate unrelated workloads without terminating
them. Retain unsuccessful observations until the experiment is closed. Missing
or inconclusive evidence never passes a claim it cannot support. Report costs
as well as gains; the per-workload allowance remains max(5%, 1 ms).
No ongoing obligation to rerun retired historical campaigns exists.

Native distribution remains subject to [licensing](licensing.md).

Report slow workload families, memory, failed cells and scaling alongside aggregate
time. Unexplained material scaling cliffs fail qualification. Changing the default
max(5%, 1 ms) allowance requires Johannes's decision before the affected implementation.

The complexity inventory counts runtime Rust source after removing whitespace,
comments and test-only items. It follows declared modules, including non-test cfg
variants. Per-crate lines, long files/functions and sensitive dispatch conditions
ratchet; decreases lower the inventory and growth needs exact scoped reasons.
The inventory was relocated unchanged from the completed program; its historical
growth explanations remain attached to the same measurements.
