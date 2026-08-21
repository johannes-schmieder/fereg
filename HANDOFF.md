# Checkpoint A handoff

> Correction: the licensed-Stata smoke evidence originally described below was
> invalid because macOS was routed to the Linux plugin and Stata's zero process
> status was treated as success after do-file errors. The follow-up loader now
> uses `c(machine_type)` for `Unix` Mac runtimes, and the runner requires an
> exact fixture terminal sentinel. See `VALIDATION.md` and
> `build_evidence/local/stata-smoke.txt` for the superseding evidence.

The repository is based on merged PR #1 at
`2980a2eccc97fd4cefa51dd763d2146dcbdec089`. The repair source is committed at
`80d9283d38fab520d546d50937feb6bffba02ca3` and was merged in
[PR #2](https://github.com/johannes-schmieder/StFixest/pull/2). It restores
the official SPI source contract, repairs bridge behavior, hardens the binary
audit, and makes the Linux-only mock-plugin test skip explicitly on macOS.
Preserve the fail-closed estimator boundary.

## Current evidence

The baseline's local native build validation passed:

- 60 Python tests and all portable/C/mock-SPI checks;
- Rust formatting and all-target/all-feature Clippy with warnings denied;
- 76 debug and 76 release Rust unit tests;
- the explicit 5-test plugin FFI run;
- a universal macOS plugin audit for both `x86_64` and `arm64` slices;
- the host-native final-plugin ABI/self-test harness.

The last bullet in the original handoff claimed a licensed-Stata pass after
only the numeric version-guard repair; that claim is retracted. The corrected
loader and sentinel runner now pass native arm64 Stata 18
version/doctor/selftest, fail-closed estimator checks, the 100-call reload loop,
and all current parser/postestimation fixtures. Rosetta passes the installation
and reload fixtures as emulated x86-64 evidence.

Use `build_evidence/local/toolchain.txt`, `rust-tests.txt`, and
`macos-build.txt` for native build evidence. Use the corrected sanitized
`build_evidence/local/stata-smoke.txt` receipt for licensed Stata; the former
raw-log hash is retracted. `VALIDATION.md` explains both loader failures, the
sentinel-based verification, the Python PATH requirement, and the limits of
this evidence.

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
4. GitHub Actions is intentionally disabled. Complete the Linux and Windows
   target-native build, audit, loader, and licensed-Stata checks manually and
   record their exact commands and evidence before treating this baseline as
   cross-platform validated.

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
