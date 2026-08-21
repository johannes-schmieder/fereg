# Checkpoint A — baseline repair and local native evidence

**Status:** local macOS native validation and a licensed-Stata 18
installation/safety smoke passed; merge, CI, cross-platform, parser, and parity
gates remain open.

**Base:** merged PR #1,
`2980a2eccc97fd4cefa51dd763d2146dcbdec089`.

## Repair scope

- Restore the official SPI 3.0 `stplugin.c`, including its sole `pginit()`
  implementation, to the SHA-256 frozen in `references.lock.toml`.
- Keep StFixest thread binding and validation in the separate bridge; bind the
  calling thread at the first `stata_call` and reject worker-thread SPI access.
- Strengthen architecture, dependency, export, and unresolved-symbol audits.
- Skip the GNU/ELF mock-plugin audit on non-Linux hosts without converting the
  missing Linux execution into a pass.
- Compare numeric `c(stata_version)` directly in the Stata loader. The initial
  licensed-Stata attempt used `real(c(stata_version))` and failed before plugin
  entry with type-mismatch `r(109)`.

No frozen reference was changed. Estimator actions remain disabled.

## Local evidence

With pinned Rust/Cargo 1.85.1, Xcode Apple clang 21.0.0, and Homebrew Python
3.14.5 selected through the documented PATH:

- 60 Python tests passed;
- portable shell/Python/TOML/C/Clang and mock-SPI checks passed;
- formatting and strict all-target/all-feature Clippy passed;
- 76 debug and 76 release Rust unit tests passed;
- the explicit 5-test plugin FFI run passed; and
- a universal macOS plugin was built and audited with exactly `x86_64` and
  `arm64`, exactly `_pginit` and `_stata_call` exported per slice, allowed OS
  dependencies only, and no unresolved symbols. The host-native final-plugin
  ABI/self-test harness passed.
- after the loader repair, Stata 18 MP passed version, doctor with 32 requested
  threads, native and overall self-tests, both `r(498)` fail-closed estimator
  checks without dataset-shape changes, and a 100-call reload loop.

Evidence transcripts:

- `build_evidence/local/toolchain.txt`
- `build_evidence/local/rust-tests.txt`
- `build_evidence/local/macos-build.txt`
- `stata_smoke.log` (SHA-256
  `e863e49fb21d264cb11b831505a474953927677ee159c8db73d2985cfbb1b353`)

The universal plugin SHA-256 is
`db265ee715fc147026321c13dc7eaaf9362a89a7c4778873c7fda8b3f628cf09`.

## Unpassed gates

- The repair set is not yet represented by a commit or GitHub Actions jobs.
- No licensed-Stata metadata-parser, enabled-estimator, or reference-parity
  suite has passed beyond the narrow installation/safety smoke.
- No real Linux or Windows plugin has been built or loaded.
- The x86-64 macOS slice has not run on an Intel host.
- No clean-machine, release, parity, performance, or peak-memory gate has
  passed.
- `stfeols` and `stfepois` remain unsupported and fail closed with `r(498)`.

This checkpoint repairs and validates a development baseline. It does not
certify a production binary or a scientific estimator.
