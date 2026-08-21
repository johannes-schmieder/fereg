# Validation record

This record describes local Checkpoint A evidence collected on 2026-08-21. The
original licensed-Stata smoke claim is retracted: macOS was misclassified as
Linux, and the runner accepted Stata's zero process status without proving that
the do-files completed. The corrected evidence below requires an exact terminal
sentinel from each fixture. It does not certify estimator correctness, broad
Stata compatibility, other platforms, or a release artifact.

## Source state

- Base: PR #1 merge `2980a2eccc97fd4cefa51dd763d2146dcbdec089` on `main`.
- Historical validation target: Checkpoint A baseline code commit
  `80d9283d38fab520d546d50937feb6bffba02ca3`, later merged in
  [PR #2](https://github.com/johannes-schmieder/StFixest/pull/2).
- Corrected loader validation base: `main` at
  `fd5cd42378152fae9ad60a4a753d42a701bbbaea` after merged PR #3.
- Frozen references: unchanged.
- Official SPI source check: `plugin/stplugin.c` SHA-256
  `7f954e5985c53bb80533d0cbd5df794c0aaa94de2a69013e3b8c610d9011c69a`,
  matching `references.lock.toml`.

The repair source now has a reproducible commit identifier. The local logs
remain the primary evidence for the macOS and licensed-Stata runs.

## Local construction environment

- Host: Apple-Silicon macOS arm64, Darwin 25.5.0.
- Rust: pinned `rustc 1.85.1` (`aarch64-apple-darwin`).
- Cargo: 1.85.1.
- C compiler: Apple clang 21.0.0 from Xcode.
- The default `/usr/bin/python3` is Python 3.9.6 and cannot run the repository's
  `tomllib` checks. Successful validation used:

  ```bash
  PATH="/Users/johannes/.rustup/toolchains/1.85.1-aarch-apple-darwin/bin:/opt/homebrew/bin:$PATH"
  ```

  This selects the pinned Rust tools and Homebrew Python 3.14.5.

`build_evidence/local/toolchain.txt` records the host and compiler discovery;
the PATH qualification above is necessary to interpret the successful runs.

## Passed locally

The detailed transcript is `build_evidence/local/rust-tests.txt`.

- Repository invariants passed.
- 60 Python protocol, binary-audit, package-contract, numerical-source,
  randomized/property, parser-static, and release-staging tests passed.
- Portable shell syntax, Python compilation, TOML parsing, strict C via `cc`
  and Clang, and the mock-SPI bridge test passed.
- The Linux GNU/ELF mock-plugin audit is skipped on macOS after platform
  detection; it was not counted as a macOS pass.
- `cargo fmt --all -- --check` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  passed.
- `cargo test --workspace --all-targets --all-features` passed 76 unit tests:
  68 in `stfixest-core`, 5 in `stfixest-plugin`, and 3 in
  `stfixest-testkit`.
- `cargo test --workspace --release --all-features` passed the same 76 unit
  tests. There were no Rust doc tests to execute.
- `cargo test -p stfixest-plugin --features ffi --lib` passed all 5 plugin
  tests, including null-argument rejection and panic containment.

## macOS build evidence

The detailed transcript is `build_evidence/local/macos-build.txt`.

The first build attempt could not resolve `static.crates.io` inside the
restricted network environment. A permitted retry downloaded the missing
pinned crates and completed successfully; the final recorded return code is 0.

The resulting local development artifact:

- is one universal Mach-O plugin with exactly `x86_64` and `arm64` slices;
- exports exactly `_pginit` and `_stata_call` in each slice;
- has no unresolved symbols in either slice;
- depends only on audited macOS system libraries/frameworks in either slice;
- passed the final host-native ABI/self-test harness, including version,
  diagnostics, incompatible-protocol rejection, and injected-panic containment;
- has SHA-256
  `db265ee715fc147026321c13dc7eaaf9362a89a7c4778873c7fda8b3f628cf09`.

The audit inspected both slices, but the ABI harness ran on the arm64 host. It
does not prove execution on Intel hardware.

## Licensed-Stata smoke evidence

The initial Stata 18 MP attempt failed before entering the plugin. The loader
used `real(c(stata_version))`, but Stata 18 supplies a numeric `c()` value, so
the guard returned type-mismatch `r(109)`. The loader repair compares
`c(stata_version)` directly.

That earlier run is not accepted as evidence. A second loader defect remained:
native macOS Stata 18 reports `c(os)==Unix`, so the loader selected
`stfixest_linux.plugin`. The old runner also failed to set the repository
ado-path itself and checked only the process return code, which can remain zero
after a do-file error.

After selecting macOS when `c(os)==MacOSX` or when `c(os)==Unix` and
`c(machine_type)` starts with `Mac`, licensed Stata 18 MP running natively on
Apple Silicon reported `Unix` / `Mac (Apple Silicon)`, loaded the universal
`stfixest_macos.plugin` arm64 slice, and passed:

- `stfixest, version`;
- `stfixest, doctor threads(32) verbose`, reporting
  `native_selftest=pass`;
- `stfixest, selftest threads(32)`, reporting `overall=pass`;
- the `stfeols` and `stfepois` fail-closed checks, each returning `r(498)`
  without changing `_N` or `c(k)`; and
- the reload loop containing 100 drop/reload/`selftest threads(2)` calls; and
- all current parser and fail-closed postestimation fixtures.

Every accepted fixture emitted `END_FIXTURE|<name>|rc=0` after its final
assertion. The runner rejects a missing or nonzero sentinel even if Stata exits
zero and runs each fixture through a wrapper that prepends the repository
`stata/` and `dist/` directories to the ado-path. Raw Stata logs contain license
details and are not committed; `build_evidence/local/stata-smoke.txt` is the
sanitized receipt.

The installation and 100-reload fixtures also passed under Rosetta. That run
reported `Unix` / `Macintosh (Intel 64-bit)` and loaded the x86-64 slice from
the same universal plugin. It is emulated loader/slice evidence, not native
Intel-host certification.

## GitHub Actions policy

GitHub Actions is disabled for this project and the repository workflow files
have been removed. This validation record relies only on commands run locally
or on the applicable target-native host; no CI result is claimed.

## Not validated

- Any enabled estimator invocation or successful estimate posting.
- Any `reghdfe` or `ppmlhdfe` reference capture or parity comparison.
- Estimation-sample, coefficient, VCE, DoF, prediction, `margins`, or `estat`
  parity.
- Real Linux or Windows builds, dependency audits, or loader harnesses.
- Intel-host execution of the macOS x86-64 slice.
- Cross-platform release staging from three target-native artifacts.
- Clean-machine, sanitizer, Miri, fuzz, leak, peak-memory, or performance gates.
- Target-native Linux and Windows build, loader, and licensed-Stata evidence.

The locally built plugin is therefore development evidence only. It must not be
described or distributed as a production-certified release.
