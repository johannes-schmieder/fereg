# Licensed Stata/Rust CI runner

StFixest uses a private, repository-level GitHub Actions runner on the Mac
Studio for licensed Stata/MP and the ordinary quick Rust/C/source lane. The
runner is privileged: workflow code executes as the logged-in `johannes`
macOS account. Never enable this workflow for a public repository, a public
pull request, an untrusted fork, or an untrusted collaborator branch.

## Machine and licensed software

- Host: Apple Silicon Mac Studio `Mac14,14`, Apple M2 Ultra, 24 CPU cores
  (16 performance and 8 efficiency), 192 GB RAM, `arm64`.
- Operating system at installation: macOS 26.5.2 (build 25F84).
- Stata: Stata/MP 18, bundle version 18.0.130.
- Stata executable:
  `/Applications/Stata/StataMP.app/Contents/MacOS/stata-mp`.
- Working batch form:
  `/Applications/Stata/StataMP.app/Contents/MacOS/stata-mp -q -b do FILE.do`.
- Rust: pinned toolchain 1.85.1 with `rustfmt`, `clippy`, and the
  `aarch64-apple-darwin` and `x86_64-apple-darwin` targets installed.
- C/C++: Apple clang from the full Xcode installation.

Stata 18 on this Mac returns shell code zero even after an uncaught Stata
error. CI therefore never treats the process code alone as authoritative. The
generic harness captures `_rc` and writes `stata.status`; project fixtures also
require exactly one terminal `END_FIXTURE|<name>|rc=0` sentinel and persist a
per-fixture status file. A zero shell code plus `stata_rc != 0`, a missing
sentinel, or a missing status file is a failure.

The user Stata profile is
`/Users/johannes/Library/Application Support/Stata/profile.do`; it was empty at
installation. CI resets Stata state, stages only Git-tracked repository files,
uses fresh temporary PLUS/PERSONAL/OLDPLACE/SITE directories, and binds every
fixture to the exact freshly built plugin and SHA-256. Development artifacts
are never installed into the permanent Stata PLUS tree.

## Repository interface

Run profiles locally from the repository root:

```sh
./ci/run_stata_ci.sh version
./ci/run_stata_ci.sh smoke
./ci/run_stata_ci.sh syntax
./ci/run_stata_ci.sh quick
./ci/run_stata_ci.sh mata-unit
./ci/run_stata_ci.sh plugin-build
./ci/run_stata_ci.sh plugin-load
./ci/run_stata_ci.sh numerical-small
./ci/run_stata_ci.sh differential
./ci/run_stata_ci.sh benchmark-small
./ci/run_stata_ci.sh full
```

Profiles are data-driven by `ci/stata_profiles.json`. `version` is the minimal
unattended Stata launch/error probe. `smoke` builds, audits, signs, loads, and
self-tests a universal macOS plugin before checking the public fail-closed
boundary. `quick` adds the normal parser, loader/reload, and postestimation
fixtures. `full` runs all of those plus the prepared-OLS boundary, hidden
eclass transaction, hidden numerical parity, and interactive Control-C
rollback fixture. The other profile names select bounded subsets without
machine-level reconfiguration.

Each run occurs in a fresh temporary tree. A supervisor holds the same
`/private/tmp/varcomp-kss-stata-ci.lock` used by the varcomp_kss runner, so the
two repository-level runners cannot execute licensed Stata concurrently. On a
timeout it terminates only the profile process group and escalates to a kill if
needed. Sanitized local evidence is copied to `.ci/stata/run/`, which Git
ignores.

The normal workflow is `.github/workflows/stata-ci.yml`. Pushes to `main` and
`codex/**` run `ci/run_rust_quick.sh` and the Stata `quick` profile sequentially
on this Mac. There is deliberately no `pull_request` trigger. Manual dispatch
can select any Stata profile and does not redundantly run the ordinary Rust
lane. `.github/workflows/hosted-qualification.yml` is manual-only and contains
the GitHub-hosted Linux/Windows/macOS release-qualification jobs.

Full logs, per-fixture statuses, the exact universal candidate plugin, its
SHA-256, and `receipt.json` are retained as an Actions artifact for 30 days.
The publisher job commits:

- `.ci/stata/results/<tested-sha>.json` for every completed run;
- `.ci/stata/latest.json` only when the tested SHA is still the newest
  non-receipt commit on that branch.

Receipt-only commits use `[skip ci]`, and both receipt paths are excluded from
workflow triggers. A remote client must match the complete `tested_sha`; it
must not infer that the current branch passed merely because `latest.json`
exists.

Schema version 1 records at least `tested_sha`, `run_id`, `run_attempt`,
`profile`, overall `status`, `stata_status`, `rust_status`, `rust_rc`,
`failure_kind`, `process_rc`, `stata_rc`, Stata
version/edition, platform, runner name, timestamps, test counts, and the exact
plugin SHA-256. Failure kinds distinguish launch errors, timeouts, crashes,
missing status/log/sentinel, ordinary Stata errors, build/toolchain errors,
changed candidates, and missing outputs.

## Runner installation and operation

- Installation: `/Users/johannes/actions-runners/stfixest-stata`.
- Runner version at installation: 2.336.0; automatic updates are enabled.
- Runner name: `macstudio-stata-mp18-stfixest`.
- Scope: repository-level, private
  `johannes-schmieder/StFixest` repository only.
- Labels: `self-hosted`, `macOS`, `ARM64`, `stata`, `stata-mp`, `stata18`,
  `stfixest`.
- Work directory: `_work` below the installation directory.
- LaunchAgent:
  `/Users/johannes/Library/LaunchAgents/actions.runner.johannes-schmieder-StFixest.macstudio-stata-mp18-stfixest.plist`.

Manage it from the installation directory:

```sh
./svc.sh status
./svc.sh stop
./svc.sh start
```

The service runs as a per-user LaunchAgent, so `johannes` must remain logged
into the macOS GUI session. AC system sleep is disabled (`sleep 0`); display
sleep is harmless. No inbound port, SSH listener, or public endpoint is used.
The runner connects outbound to GitHub.

The service arrangement was proved with actual work, not only an idle/online
indicator. On 2026-08-22, push run `32614167293` passed Stata and Rust for
`ae4d400`; deliberate Stata failure run `32614279636` recorded `r(9)` for
`d1e9497` while Rust remained green; and fix run `32614371462` passed both lanes
for `955bc89`. The deliberate failure is retained only on the setup branch and
its per-SHA receipt, not as a commit on `main`. PR #10 was squash-merged as
`4bc9d71244c29903966b47dd3441b6508472f1d0`; final main run `32614591791`
passed its 11-fixture quick profile (including the StFixest smoke fixture) and
the Rust/C/source lane.

A restricted automation shell can incorrectly report `Stopped` because
`svc.sh` uses an unqualified launchd lookup. The authoritative check is:

```sh
launchctl print \
  "gui/$(id -u)/actions.runner.johannes-schmieder-StFixest.macstudio-stata-mp18-stfixest"
```

Look for `state = running`, an active PID, and a non-failing last exit code.
For a stuck/offline runner, also inspect the repository's Settings → Actions →
Runners page and these bounded logs:

```sh
tail -n 200 "/Users/johannes/Library/Logs/actions.runner.johannes-schmieder-StFixest.macstudio-stata-mp18-stfixest/stdout.log"
tail -n 200 "/Users/johannes/Library/Logs/actions.runner.johannes-schmieder-StFixest.macstudio-stata-mp18-stfixest/stderr.log"
```

To unregister, stop and uninstall the LaunchAgent with `./svc.sh stop` and
`./svc.sh uninstall`, then remove the runner in repository settings (or run
`./config.sh remove` with a newly generated short-lived removal token). To
re-register, obtain a new short-lived registration token from the same page,
reuse the recorded name/labels/working directory, and reinstall/start the
service. Registration and removal tokens must never be committed, logged, or
added to this document.

## Security assumptions

- The repository remains private and all workflow writers are trusted.
- Fork pull-request workflows remain disabled and no self-hosted workflow has a
  `pull_request` trigger.
- Workflow artifact paths are explicit and never include the home directory.
- Only synthetic fixtures are authorized; confidential research data is out of
  scope unless separately approved.
- The runner executes one Stata profile at a time across StFixest and
  varcomp_kss through the shared file lock.
- A workflow change has the same practical power as this macOS account and
  must be reviewed accordingly.
