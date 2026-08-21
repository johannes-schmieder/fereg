# Validation record

## Construction environment

The package was assembled in a Debian x86-64 sandbox with GCC 14.2, Clang 17,
and Python 3.13. Rust/Cargo, licensed Stata, the Windows SDK, and Xcode were not
installed. Native DNS/network access and executable/archive transfer were unavailable, so
installing the pinned Rust toolchain was attempted through multiple official and
mirrored routes but could not proceed. This record does not claim a compiled Rust
plugin or a Stata run.

## Executed successfully in the construction sandbox

The exact final transcript is stored in `build_evidence/sandbox_validation.txt`.
The available suite covers:

- repository and source-boundary invariants;
- 38 Python protocol, randomized factorization/interaction, thread-partition,
  memory, Rust-source-boundary, parser-static, binary-audit, packaging, and
  release-staging tests;
- strict C11 compilation of official `stplugin.c` and the bridge with warnings
  as errors, plus GCC static analysis;
- every exposed SPI wrapper against a strict mock `ST_plugin` table;
- dynamic loading of a mock Linux `.plugin`, `pginit` negotiation, required
  exports, and hidden internal dispatch;
- version, doctor/32-thread, protocol-mismatch, and panic-return behavior through
  the same C entry path using a faithful native test dispatcher;
- shell syntax checks, Python bytecode compilation, and TOML parsing;
- deterministic source-ZIP creation and manifest verification.

## Encoded but not executable in this sandbox

- `cargo fmt --check`, clippy, Rust unit/integration tests, Miri, sanitizers, and
  fuzzing;
- linking and dynamically loading the actual Rust static library on Linux;
- Windows/MSVC and macOS universal builds;
- licensed-Stata plugin loading, reload-loop behavior, parser semantics, and
  reference-command captures.

These are mandatory handoff gates. The build scripts fail unless the final
Rust-linked artifact passes its target-native binary audit and loader harness;
the Stata fixtures remain a separate licensed validation step.
