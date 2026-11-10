# Local Stata and Rust checks

GitHub Actions are paused. Use local Mac checks, authorized SCC jobs and the
private Windows test machine; do not dispatch GitHub workflows.

## Setup

Use the toolchain in rust-toolchain.toml, with rustfmt and Clippy, Apple clang,
both Apple targets, and Stata/MP on the Mac (currently version 19). The default
Stata binary is
`/Applications/Stata/StataMP.app/Contents/MacOS/stata-mp`; runner configuration
may override it. Use synthetic fixtures and retain the shared licensed-runner
lock. Keep tokens, licensed logs, local account paths and confidential data
out of Git.

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
`older-stata` runs the fixture written in Stata 15 syntax; run it with the oldest
supported release (Stata 15) as well as the current one. The other fixtures
declare `version 18.0` and need Stata 18 or newer.
Build the universal Mac plugin with `./scripts/build_macos.sh`.

The current formal runner checks a committed source archive in an isolated
temporary tree; it does not validate arbitrary uncommitted files. For working
tree development fixtures, use ci/run_fixture_profile.py and its documented
arguments. Task outputs belong under one `target/tmp/<task>/` directory; fixed runner
outputs may use their documented defaults.
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

A fixture that needs an input a source-tree run lacks declares it with a
`* fixture-requires:` line: `staged-package` (the `staged-install` profile,
run by `scripts/run_staged_stata_tests.py --profile staged-install` against an
extracted release archive), `windows` (run by `scripts/windows_ci_runtime.do`
in the Windows qualification) or `input-trace` (`scripts/run_stata_tests.py`
sets the solver trace for that process and the trace must pass
`scripts/check_input_trace.py`). A run that cannot provide the input does not
start the fixture: when the fixture was named explicitly (every profile) it
fails as not run, and in a whole-tree run (`run_stata_tests.py` without paths)
it is reported as not run with its reason. Not-run fixtures never count as
passed.

## Workflow boundaries

The retained definitions are manual; no remote run is required during the pause.
Follow [AGENTS](../AGENTS.md) and [development checks](../docs/development_workflow.md).
Linux builds and licensed Stata checks run on SCC; Windows checks use the private
Windows test machine. Mac checks run locally. See
[validation](../docs/VALIDATION.md) for the tested artifacts and coverage.
