# Implementation status

As of 2026-08-21, `main` includes pull request #1 at merge commit
`2980a2eccc97fd4cefa51dd763d2146dcbdec089`. The Checkpoint A repair source is
committed at `80d9283d38fab520d546d50937feb6bffba02ca3` and published as
[draft PR #2](https://github.com/johannes-schmieder/StFixest/pull/2). This is
development evidence, not a release or production certification.

## Implemented source

- Phase 0-1 contracts, metadata-only Stata parsing, the versioned protocol,
  diagnostics, self-tests, mock SPI, panic containment, and cross-platform
  build/audit machinery.
- Rust-first sample preparation, weight validation, compact categorical and
  interaction coding, and blockwise ordinary factor-column generation.
- Iterative singleton pruning, connected components, weighted fixed-effect
  projection, MAP and modified-LSMR paths, preconditioning, automatic routing,
  and independently recomputed FE certificates.
- Stata-independent OLS/FWL, rank handling, fit statistics, DoF policy, and
  homoskedastic, robust, and multiway-clustered covariance kernels.
- Stata-independent PPML family, IRLS, certificates, covariance, and separation
  kernels (`fe`, `simplex`, `ir`, and `mu`).

These are source and local-unit-test claims. They do not mean that either Stata
estimator is supported.

## Checkpoint A repair status

- The official SPI 3.0 `plugin/stplugin.c` owns `pginit()` again and matches the
  SHA-256 frozen in `references.lock.toml`:
  `7f954e5985c53bb80533d0cbd5df794c0aaa94de2a69013e3b8c610d9011c69a`.
- The separate StFixest bridge binds the Stata calling thread on the first
  `stata_call`, rejects SPI access from worker threads, validates pointers and
  lengths, and preserves the narrow C/Rust boundary.
- The macOS test runner now skips the GNU/ELF-only mock-plugin audit instead of
  passing Linux linker flags to Apple `ld`. The Linux-specific test remains a
  Linux gate.

## Locally validated

On an Apple-Silicon macOS host with pinned Rust/Cargo 1.85.1 and Xcode Apple
clang 21.0.0:

- 60 Python contract/property tests passed;
- portable shell, Python, TOML, strict C, Clang, and mock-SPI checks passed;
- `cargo fmt --all -- --check` passed;
- Clippy passed for the workspace, all targets, and all features with warnings
  denied;
- 76 Rust unit tests passed in debug and the same 76 passed in release
  (68 core, 5 plugin, and 3 testkit);
- the explicit plugin `ffi` test invocation passed all 5 plugin tests;
- a universal `stfixest_macos.plugin` was built with `x86_64` and `arm64`
  slices; both slices passed architecture, dependency, undefined-symbol, and
  exact-export audits; and the host-native ABI/self-test harness passed;
- licensed Stata 18 MP loaded the macOS plugin, passed `version`,
  `doctor threads(32)`, and `selftest threads(32)`, observed the intended
  `r(498)` result from both estimators without changing the dataset, and
  completed 100 plugin reload/self-test calls.

The first licensed-Stata attempt exposed a loader-only type mismatch before the
plugin was entered: `real(c(stata_version))` returned `r(109)` because the Stata
18 `c()` value is already numeric. The guard now compares `c(stata_version)`
directly, and the repaired smoke run passed.

The non-Stata transcripts are under `build_evidence/local/`; the final Stata
transcript is `stata_smoke.log`.

## Unsupported and unresolved

- `stfeols` and `stfepois` remain explicitly unsupported and fail closed with
  `r(498)` without posting estimates.
- No licensed-Stata metadata-parser, enabled-estimator, or reference-parity
  suite has passed yet; only the installation/safety smoke described above has
  passed.
- No real Linux x86-64 or Windows x86-64 plugin has been built or loaded in this
  checkpoint.
- No clean-machine, numerical parity, performance, memory, or cross-platform
  release gate has passed.
- GitHub Actions is enabled and both PR workflows triggered, but every job was
  prevented from starting by GitHub's account billing/spending restriction.
  The [CI run](https://github.com/johannes-schmieder/StFixest/actions/runs/32484658566)
  and [Source Snapshot run](https://github.com/johannes-schmieder/StFixest/actions/runs/32484658624)
  therefore provide neither a code pass nor a code failure. The repository
  owner must resolve **Billing & plans** and rerun the PR checks.
