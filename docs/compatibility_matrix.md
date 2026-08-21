# Compatibility matrix

The entries below distinguish an encoded build target from a target actually
validated in licensed Stata.

| Stata | OS / architecture | Toolchain | Artifact | Construction-sandbox status | Required release validation |
|---|---|---|---|---|---|
| 18+ | Linux x86-64, glibc 2.28+ | Rust 1.85.1 + GCC/Clang | `stfixest_linux.plugin` | Build script encoded; not compiled here | Load/doctor/selftest on oldest and newest supported Stata |
| 18+ | Windows x86-64 | Rust MSVC + VS 2022 Build Tools, `/MT` | `stfixest_windows.plugin` | Build script encoded; not compiled here | `dumpbin` audit, Stata 18/19 load and repeated reload |
| 18+ | macOS Intel x86-64 | Xcode clang + Rust x86-64 | one slice of `stfixest_macos.plugin` | Loader and slice executed under Rosetta on Apple Silicon; this is emulated evidence only | Run under native Intel Stata where available |
| 18+ | macOS Apple Silicon arm64 | Xcode clang + Rust arm64 | one slice of `stfixest_macos.plugin` | Native Stata 18 version/doctor/selftest and reload fixtures pass | Repeat on clean machine and newest supported Stata |
| 18+ | macOS universal | `lipo` joins both slices | `stfixest_macos.plugin` | Universal audit encoded | Verify `lipo -archs` reports exactly `x86_64 arm64` |

## CPU baseline

Distributed builds do not use `target-cpu=native`. Runtime diagnostics report a
conservative CPU feature path, while all correctness code remains available on
the baseline architecture. Future SIMD kernels must retain a scalar fallback
and be differential-tested.

## Stata interface

The C sources implement Stata SPI 3.0, which is the interface used by Stata 18+
for native C plugins. The ado layer also performs an explicit Stata-major guard
before loading a platform binary.

Stata 18 reports `c(os)==Unix` on both tested macOS execution modes. The loader
therefore uses a `Mac*` match on `c(machine_type)` before treating remaining
Unix runtimes as Linux. Static tests cover Apple-Silicon and Intel Mac labels,
GUI-style `MacOSX`, Unix PC, Windows, and an unknown fail-closed case.
