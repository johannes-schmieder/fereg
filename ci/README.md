# Local Stata and Rust checks

GitHub testing is paused by Johannes. Run applicable tests locally on this Mac;
do not dispatch remote or licensed runner workflows until he resumes them.

## Setup

Use the toolchain in rust-toolchain.toml, with rustfmt and Clippy, Apple clang,
both Apple targets, and Stata/MP 18 on the Mac. The default Stata binary is
`/Applications/Stata/StataMP.app/Contents/MacOS/stata-mp`; runner configuration
may override it. Use synthetic fixtures and retain the shared licensed-runner
lock. Keep tokens, licensed logs, local account paths and confidential data
out of Git.

The self-hosted runner labels are self-hosted, macOS, ARM64, stata, stata-mp,
stata18 and fereg. Manage the installed runner through its local svc.sh
status/start/stop commands; the GUI account must stay logged in. Locate the
runner through GitHub Settings → Actions → Runners rather than committing its
host-specific installation directory or service identity.

## Commands

From the repository root, run `./scripts/run_tests.sh` for fmt, Clippy, native
tests and mock-SPI checks. It uses the optimized ci profile with debug
assertions; FEREG_TEST_PROFILE can select another declared Cargo profile.
The Windows entry point is scripts/run_tests.ps1 and requires MSVC tools.

`./ci/run_stata_ci.sh <profile>` selects a profile in ci/stata_profiles.json.
Useful profiles include version, smoke, syntax, quick, mata-unit, plugin-build,
plugin-load, numerical-small, differential, fereg-regression, ppml-scientific,
ppml-hidden, benchmark-small and full. Use quick for the OLS screen and
ppml-scientific for the public PPML fixtures; neither stands in for the other.
Build the universal Mac plugin with `./scripts/build_macos.sh`.

The current formal runner checks a committed source archive in an isolated
temporary tree; it does not validate arbitrary uncommitted files. For working
tree development fixtures, use ci/run_fixture_profile.py and its documented
arguments. Local outputs stay in ignored .ci/stata/run/ or target/ directories.
Preserve source/plugin identity when comparing runtime revisions.

The development profile contains standalone fixtures with terminal sentinels;
parameterized capture helpers must have a maintained driver supplying their
inputs. Retired campaign helpers are not standalone regression tests. Run
`FEREG_SOLVER_TRACE=1` only for targeted diagnosis: trace logging allocates and
therefore invalidates the zero-allocation audit assertions in a full profile.

Stata shell exit zero alone is insufficient. Inspect Stata return codes,
per-fixture status and the terminal END_FIXTURE sentinel. The installed-package
tests must load the extracted package with an isolated ado path. Interactive
Break and transaction failures must restore previous results and outputs.

## Workflow boundaries

The retained definitions are manual; no remote run is required during the pause.
Follow [AGENTS](../AGENTS.md) and [development checks](../docs/development_workflow.md).
Current qualification is Mac-only. Platform build scripts are retained for future
use; their existence does not establish fresh platform evidence.
