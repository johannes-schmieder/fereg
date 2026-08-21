# Checkpoint A handoff

The repository is based on merged PR #1 at
`2980a2eccc97fd4cefa51dd763d2146dcbdec089`. The repair source is committed at
`80d9283d38fab520d546d50937feb6bffba02ca3` in
[draft PR #2](https://github.com/johannes-schmieder/StFixest/pull/2). It restores
the official SPI source contract, repairs bridge behavior, hardens the binary
audit, and makes the Linux-only mock-plugin test skip explicitly on macOS.
Preserve the fail-closed estimator boundary.

## Current evidence

Local Apple-Silicon validation passed:

- 60 Python tests and all portable/C/mock-SPI checks;
- Rust formatting and all-target/all-feature Clippy with warnings denied;
- 76 debug and 76 release Rust unit tests;
- the explicit 5-test plugin FFI run;
- a universal macOS plugin audit for both `x86_64` and `arm64` slices;
- the host-native final-plugin ABI/self-test harness; and
- licensed Stata 18 MP version/doctor/selftest, fail-closed estimator checks,
  and a 100-call reload loop after repairing the loader's numeric version guard.

Use `build_evidence/local/toolchain.txt`, `rust-tests.txt`, and
`macos-build.txt`, plus `stata_smoke.log` (SHA-256
`e863e49fb21d264cb11b831505a474953927677ee159c8db73d2985cfbb1b353`),
as the primary local transcripts. `VALIDATION.md` explains the initial Stata
`r(109)` loader failure, its repaired verification, the Python PATH requirement,
and the limits of this evidence.

## Required boundary

Do not enable either estimator. `stfeols` and `stfepois` must continue to fail
closed with `r(498)` and must not post estimates. Internal numerical source and
unit tests are not Stata-facing scientific certification.

Do not call the universal macOS artifact a release. Broader licensed-Stata
parser/parity coverage, real Linux and Windows artifacts, Intel-host macOS
execution, clean-machine validation, reference parity, and performance/memory
gates remain open.

## Before merging Checkpoint A

1. Review the complete draft-PR diff against `2980a2e` and confirm that no
   frozen reference changed.
2. Re-run the local suite with the pinned Rust and Homebrew Python first on
   `PATH`:

   ```bash
   PATH="/Users/johannes/.rustup/toolchains/1.85.1-aarch-apple-darwin/bin:/opt/homebrew/bin:$PATH" ./scripts/run_tests.sh
   PATH="/Users/johannes/.rustup/toolchains/1.85.1-aarch-apple-darwin/bin:/opt/homebrew/bin:$PATH" ./scripts/build_macos.sh
   ```

3. Preserve the passing licensed-Stata smoke log and its hash. Run the remaining
   metadata-parser fixtures and record the exact executable/version, commands,
   output, and any failures.
4. Resolve the repository's **Billing & plans** issue, then rerun the
   [CI](https://github.com/johannes-schmieder/StFixest/actions/runs/32484658566)
   and [Source Snapshot](https://github.com/johannes-schmieder/StFixest/actions/runs/32484658624)
   checks on draft PR #2. All jobs were prevented from starting because GitHub
   reports failed recent payments or a spending limit requiring an increase.
   This is not a code failure, but it is not a CI pass and must not be waived.

## Open platform and scientific gates

- Build, audit, and run the final-plugin harness on real Linux x86-64 and
  Windows x86-64 hosts.
- Execute the macOS Intel slice on Intel hardware where available.
- Run clean-machine Stata installation and reload tests on every supported
  platform.
- Capture the pinned `reghdfe` and `ppmlhdfe` references.
- Independently audit the Rust numerical core before exposing an estimator.
- Complete Stata-facing sample, coefficient, VCE, DoF, saved-output, and
  postestimation parity before removing any fail-closed guard.

## Precise continuation after the baseline repair is merged

```text
$orchestrator Continue StFixest Checkpoint B: independently audit and certify the Rust numerical core before enabling estimator actions.
```
