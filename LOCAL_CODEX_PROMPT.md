# Prompt for the local Codex handoff

Treat this repository as a verification and native-integration task. First read
`AGENTS.md`, `HANDOFF.md`, `VALIDATION.md`, `STATUS.md`, and all ADRs. The
source-bound assignment is Phase 0 and Phase 1 only: do not enable `stfeols` or
`stfepois`, and do not claim estimator functionality.

1. Verify `MANIFEST.sha256` and the external ZIP SHA-256, then run
   `scripts/run_tests.sh` or `scripts/run_tests.ps1` with Rust 1.85.1. Fix every
   formatting, compiler, clippy, test, and linker failure without weakening the
   safety, metadata, or protocol contracts.
2. Build the host plugin with its target-native script. The script must audit
   dependencies/exports and load the final Rust-linked artifact. Confirm that
   only `pginit` and `stata_call` are public and that version, doctor with 32
   threads, protocol rejection, and panic containment pass.
3. In licensed Stata 18 or newer, use an isolated ado path and run every do-file
   under `tests/stata/installation`, `tests/stata/parser`, and
   `tests/stata/postestimation`.
4. Inspect all `stfixest_parse` matrices/macros. Verify `_ms_parse_parts` base,
   level, omit, and TS fields; grouped absorbed slopes; numeric/string
   FE/cluster sources; candidate-sample behavior; and unchanged `c(k)`.
5. Record exact commands, tool versions, plugin hashes, dependencies, Stata
   output, and failures under `build_evidence/local/`. Update status documents
   with observed evidence only.
6. Use native GitHub runners for the other operating systems. Assemble the
   cross-platform release only after every plugin passes its native audit and a
   licensed-Stata smoke test on that platform.

Do not replace official `stplugin.c`/`stplugin.h`, change pinned references,
introduce Python/Julia/BLAS/OpenMP runtime dependencies, call the SPI from a
worker thread, permit a panic to cross the ABI, or post partial estimates.
