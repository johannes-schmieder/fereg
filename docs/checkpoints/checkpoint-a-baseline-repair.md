# Checkpoint A — baseline repair and local native evidence

**Status:** merged baseline; original licensed-Stata smoke claim retracted and
superseded by corrected native arm64 sentinel-based evidence. Cross-platform,
clean-machine, reference-parity, and release gates remain open.

**Base:** merged PR #1,
`2980a2eccc97fd4cefa51dd763d2146dcbdec089`.

**Checkpoint source:**
`80d9283d38fab520d546d50937feb6bffba02ca3`, merged in
[PR #2](https://github.com/johannes-schmieder/StFixest/pull/2). The loader
correction is based on `main` at
`fd5cd42378152fae9ad60a4a753d42a701bbbaea`.

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
- the initial licensed-Stata result recorded for this checkpoint is not valid
  evidence. A later audit found that macOS Stata reports `c(os)==Unix`, which
  selected the nonexistent Linux artifact, while the runner accepted Stata's
  zero subprocess status without a fixture completion marker.

After the follow-up platform repair, native arm64 Stata 18 reports
`Unix` / `Mac (Apple Silicon)`, selects `stfixest_macos.plugin`, and passes
version, doctor with 32 requested threads, native and overall self-tests, both
`r(498)` fail-closed estimator checks without dataset-shape changes, the
100-call reload loop, and all current parser/postestimation fixtures. Each
accepted fixture has an exact terminal success sentinel. Rosetta passes the
installation and reload fixtures as x86-64 emulation, not native Intel proof.

Evidence transcripts:

- `build_evidence/local/toolchain.txt`
- `build_evidence/local/rust-tests.txt`
- `build_evidence/local/macos-build.txt`
- `build_evidence/local/stata-smoke.txt` (sanitized corrected receipt; raw
  license-bearing logs are not committed)

The historical checkpoint build SHA-256 was
`db265ee715fc147026321c13dc7eaaf9362a89a7c4778873c7fda8b3f628cf09`.
The plugin rebuilt for the corrected loader evidence has SHA-256
`845814eef1a0d18c7ac1e219bbeaf21a44666afbb315a9b1812ca620037e82d2`.

## Unpassed gates

- GitHub Actions is disabled by project policy. No CI result is claimed;
  Linux and Windows target-native build, audit, loader, and licensed-Stata
  evidence must be collected manually.
- No enabled-estimator or reference-parity suite has passed. Current parser
  fixtures passing is not pinned-reference parity.
- No real Linux or Windows plugin has been built or loaded.
- The x86-64 macOS slice has not run on an Intel host.
- No clean-machine, release, parity, performance, or peak-memory gate has
  passed.
- `stfeols` and `stfepois` remain unsupported and fail closed with `r(498)`.

This checkpoint repairs and validates a development baseline. It does not
certify a production binary or a scientific estimator.
