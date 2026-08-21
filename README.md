# StFixest 0.1.0 — Phase 0–1 architecture package

StFixest is a Rust-first Stata package intended to provide high-dimensional
fixed-effect OLS/WLS (`stfeols`) and PPML (`stfepois`). This handoff implements
the specification's **Phase 0 and Phase 1** architecture and safety boundary.
It does **not** claim estimator functionality.

## What works in this package

- A dependency-free Rust workspace with a safe scientific core and a narrow FFI
  crate.
- Versioned protocol handshake, typed return-code mapping, panic containment,
  deterministic diagnostics, and native self-tests.
- Local scoped-thread execution using the Rust standard library, with work-bounded
  thread counts and deterministic chunking suitable for 16/32-core servers.
- Metadata objects and validators for ordinary factor terms, absorbed terms,
  absorbed slopes, cluster interactions, variable roles, and candidate samples.
- Collision-safe deterministic numeric/string factorization and tuple
  interaction coding as independently tested preprocessing primitives.
- Official Stata SPI 3.0 C sources and a minimal C bridge.
- Explicit scientific, weight, memory, and performance contracts, with licensed-reference captures marked pending rather than invented.
- Stata 18+ ado control-plane prototypes for plugin selection, `version`,
  `doctor`, `selftest`, candidate-sample creation, and metadata-only parsing.
- Native build scripts and CI for Linux x86-64, Windows x86-64, and a universal
  macOS plugin containing x86-64 and arm64 slices.
- Mock-SPI C tests, dynamic plugin-load/export tests, target-native final-plugin
  harnesses, 38 deterministic and randomized Python contract/property tests,
  binary audits, strict GCC/Clang checks, and clean-machine plans.

## What deliberately does not work yet

`stfeols` and `stfepois` parse their documented Phase 0–1 surface and then fail
closed with `r(498)`. Fixed-effect solvers, OLS/WLS, PPML IRLS, separation,
VCE, DoF, FE recovery, prediction, `margins`, and estimator `estat` are later
phases. Returning an uncertified estimate would violate the governing contract.

## Build and test

On a machine with Rust 1.85.1 and a platform C toolchain:

```bash
./scripts/run_tests.sh
./scripts/build_linux.sh       # Linux
./scripts/build_macos.sh       # macOS/Xcode, builds universal binary
```

On Windows PowerShell with Visual Studio Build Tools:

```powershell
./scripts/run_tests.ps1
./scripts/build_windows.ps1
```

Each native build audits and dynamically loads the final Rust-linked artifact
before writing its checksum. Generated files are placed in `dist/`. See
`HANDOFF.md` for the exact local Codex/Stata validation sequence.

## Source handoff versus installable release

The handoff ZIP contains the complete source, tests, CI, and packaging system,
but no placeholder `.plugin` binaries. Target-native CI produces Linux,
Windows, and universal macOS plugins; `scripts/stage_release.py` verifies their
checksums and assembles the one-file-per-platform installable release.

## Stata installation after native build

Copy the files in `stata/` and the applicable `dist/stfixest_*.plugin` into one
personal ado directory. Then run:

```stata
stfixest, version
stfixest, doctor
stfixest, selftest
```

The loader supports Stata 18 and later and chooses the platform-specific plugin
without launching another Stata process or loading a native sidecar.

## Design rule

Stata is the control plane. Rust is the data and computation plane. Every later
estimation command must make one substantive plugin call and must post results
only after the native calculation and certificates succeed.
