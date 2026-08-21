# Local Codex and licensed-Stata handoff

A ready-to-paste local-agent brief is in `LOCAL_CODEX_PROMPT.md`. Preserve the
Phase 0–1 fail-closed estimator boundary while validating the package.

## 1. Verify the source handoff

From the extracted repository root:

```bash
sha256sum -c ../stfixest-0.1.0-phase01-handoff.zip.sha256   # adjust path as needed
python3 scripts/check_repository.py
```

Verify `MANIFEST.sha256` inside the archive before editing. The source handoff
contains no pretend native plugin; those must be produced by target-native
builds.

## 2. Establish the toolchain

Install rustup and select the pinned Rust 1.85.1 toolchain from
`rust-toolchain.toml`. Install the host C toolchain. Do not introduce OpenBLAS,
MKL, OpenMP, Python, Julia, or another numerical runtime dependency.

## 3. Run all non-Stata tests

```bash
./scripts/run_tests.sh
```

Windows Developer PowerShell:

```powershell
./scripts/run_tests.ps1
```

Resolve every formatting, compiler, clippy, Rust test, C bridge, packaging, and
repository-audit failure. Do not weaken an invariant to make a test pass.

## 4. Build and exercise the host plugin

- Linux x86-64: `./scripts/build_linux.sh`
- macOS/Xcode: `./scripts/build_macos.sh`
- Windows x86-64 Developer PowerShell: `./scripts/build_windows.ps1`

Each build script performs the binary dependency/export audit and then loads the
final Rust-linked plugin in a target-native harness. It must pass version,
doctor with `threads(32)`, incompatible-protocol rejection, and injected-panic
containment before the checksum is written.

For a foreign-linker failure, ask the pinned compiler for its exact target list
with `rustc --print native-static-libs --crate-type staticlib`; reconcile that
list without adding non-OS runtime dependencies.

## 5. Licensed Stata 18+ validation

Put `stata/` and `dist/` on an isolated ado path, then run:

```stata
clear all
adopath ++ "<repository>/stata"
adopath ++ "<repository>/dist"
which stfixest
stfixest, version
stfixest, doctor threads(32) verbose
stfixest, selftest threads(32)
do tests/stata/installation/phase01_smoke.do
do tests/stata/installation/reload_loop.do
```

Expected: diagnostics pass; repeated loading is stable; `stfeols` and
`stfepois` return the explicit Phase 0–1 `r(498)` without changing the data or
posting an estimate.

## 6. Parser review

Run every file in `tests/stata/parser/`. Compare returned matrices/macros with
Stata's `fvexpand` and `_ms_parse_parts` results. Verify base/omission and
level metadata, TS markers, grouped absorbed slopes, string FE/cluster sources,
interaction tuples, `if/in`, and unchanged `c(k)`. The parser must not use
`fvrevar`, `egen group()`, or dummy expansion.

## 7. Cross-platform assembly

Build the other targets with native CI. Place the three plugins and their
`.sha256` files together and run:

```bash
python3 scripts/stage_release.py \
  --source . \
  --plugins native-artifacts \
  --output stfixest-0.1.0-phase01-release.zip
```

The staging command rejects missing, empty, or checksum-mismatched artifacts.

## 8. Evidence and Phase 2 boundary

Store command output, tool versions, dependency reports, plugin hashes, Stata
versions, and failures under `build_evidence/local/`; update `STATUS.md` and
`VALIDATION.md` from evidence. Before Phase 2, complete the pinned reference
captures in `docs/reference_capture_status.md`. Do not enable estimator actions
until the corresponding scientific phase gate passes.
