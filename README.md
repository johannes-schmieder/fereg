# StFixest — pre-estimator development snapshot

StFixest is a Rust-first Stata package intended to provide high-dimensional
fixed-effect OLS/WLS (`stfeols`) and PPML (`stfepois`). The repository contains
substantial native preprocessing and numerical-core source, but neither public
estimator is supported yet.

## Current boundary

`stfeols` and `stfepois` deliberately fail closed with `r(498)` and post no
estimate. A narrow licensed-Stata installation/safety suite now passes in
native Apple-Silicon Stata on macOS. The same installation fixtures pass under
Rosetta, which is emulated x86-64 evidence rather than native Intel-host
certification. Reference parity, enabled estimation, clean-machine testing,
cross-platform native validation, and performance/memory certification remain
open. This repository must not be presented as a production release.

## Implemented development components

- A versioned Stata/plugin protocol, typed error mapping, panic containment,
  diagnostics, native self-tests, scoped threading, and a narrow SPI bridge.
- Metadata-only Stata parser prototypes for factor terms, absorbed terms and
  slopes, clusters, and candidate samples.
- Native sample preparation, weight validation, compact factorization and
  interaction coding, blockwise model-matrix generation, singleton pruning,
  connected components, and memory preflight.
- Certified MAP and modified-LSMR fixed-effect paths.
- Stata-independent OLS/FWL, rank, covariance, DoF, and fit-statistic kernels.
- Stata-independent PPML IRLS, certificate, covariance, and `fe`/`simplex`/`ir`/
  `mu` separation kernels.
- Build machinery for one self-contained plugin per platform, binary
  dependency/export audits, mock-SPI tests, and target-native ABI harnesses.

These claims describe source and local tests, not Stata-facing estimator
availability.

## Checkpoint A evidence

On 2026-08-21, local Apple-Silicon validation passed 60 Python tests, portable
C/mock checks, formatting, strict Clippy, 76 debug and 76 release Rust unit
tests, and the explicit plugin FFI suite. A universal macOS development plugin
was built; both slices passed binary audits and the arm64 host-native ABI
harness passed. The original licensed-Stata smoke claim was later found to be
invalid: the loader mistook Stata's `c(os)==Unix` macOS runtime for Linux, and
the old runner trusted a zero subprocess status even when a do-file aborted.
After repairing platform selection and requiring an exact fixture sentinel,
native arm64 Stata 18 MP passed version/doctor/selftest, both fail-closed
estimator checks, the 100-call reload loop, and all current parser and
postestimation fixtures.

No licensed-Stata reference-parity suite, real Linux or Windows build, enabled
estimator run, or production certification has passed. See
`STATUS.md`, `VALIDATION.md`, and `HANDOFF.md` for the exact boundary and
continuation.

The baseline repair was merged in [PR #2](https://github.com/johannes-schmieder/StFixest/pull/2),
and the numerical-core follow-up was merged in
[PR #3](https://github.com/johannes-schmieder/StFixest/pull/3).
This project does not use GitHub Actions. Validation is performed through the
documented local and target-native commands below, with evidence recorded under
`build_evidence/local/`.

## Local build and test

The pinned Rust toolchain is 1.85.1. On the validated macOS host, put the pinned
Rust tools and Homebrew Python ahead of the system Python:

```bash
PATH="/Users/johannes/.rustup/toolchains/1.85.1-aarch-apple-darwin/bin:/opt/homebrew/bin:$PATH" ./scripts/run_tests.sh
PATH="/Users/johannes/.rustup/toolchains/1.85.1-aarch-apple-darwin/bin:/opt/homebrew/bin:$PATH" ./scripts/build_macos.sh
```

Native build entry points for the remaining targets are:

```bash
./scripts/build_linux.sh
```

```powershell
./scripts/run_tests.ps1
./scripts/build_windows.ps1
```

Each native build is designed to audit and dynamically load the final
Rust-linked artifact before writing its checksum. Passing the script on one
platform is not evidence for another platform or for licensed Stata.

## Design rule

Stata is the control plane. Rust is the data and computation plane. Each future
estimation invocation must make one substantive plugin call and post results
only after the native calculation and all scientific certificates succeed.
