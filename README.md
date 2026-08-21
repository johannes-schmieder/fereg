# StFixest — pre-estimator development snapshot

StFixest is a Rust-first Stata package intended to provide high-dimensional
fixed-effect OLS/WLS (`stfeols`) and PPML (`stfepois`). The repository contains
substantial native preprocessing and numerical-core source, but neither public
estimator is supported yet.

## Current boundary

`stfeols` and `stfepois` deliberately fail closed with `r(498)` and post no
estimate. A narrow licensed-Stata installation/safety smoke passes on macOS,
but parser/reference parity, enabled estimation, clean-machine testing,
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
harness passed. After a one-line numeric version-guard repair, licensed Stata
18 MP also passed version/doctor/selftest, both fail-closed estimator checks,
and a 100-call reload loop.

No licensed-Stata parser or reference-parity suite, real Linux or Windows
build, enabled estimator run, or production certification has passed. See
`STATUS.md`, `VALIDATION.md`, and `HANDOFF.md` for the exact boundary and
continuation.

The repair source is committed at
`80d9283d38fab520d546d50937feb6bffba02ca3` in
[draft PR #2](https://github.com/johannes-schmieder/StFixest/pull/2). GitHub
Actions triggered, but every job in the
[CI run](https://github.com/johannes-schmieder/StFixest/actions/runs/32484658566)
and
[Source Snapshot run](https://github.com/johannes-schmieder/StFixest/actions/runs/32484658624)
was prevented from starting by an account billing/spending restriction. This is
not a code failure and is not a CI pass. The repository owner must resolve
**Billing & plans** and rerun the PR checks.

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
