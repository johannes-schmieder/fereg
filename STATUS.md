# Checkpoint A status (loader correction)

Checkpoint A is merged, but its original licensed-Stata smoke claim has been
retracted and superseded by the fail-closed runner evidence below. The project
is not a release candidate.

| Area | Status | Evidence / remaining gate |
|---|---:|---|
| Repository baseline | Merged | PR #2 merged the repaired baseline; PR #3 merged the first numerical-core certificate repair. The loader follow-up is based on `main` at `fd5cd42378152fae9ad60a4a753d42a701bbbaea`. |
| Checkpoint A repairs | Merged, loader follow-up in progress | [PR #2](https://github.com/johannes-schmieder/StFixest/pull/2) restored the official `stplugin.c` hash/ownership contract, repaired bridge thread binding and guards, hardened binary audits, and removed GitHub Actions. The current branch corrects macOS platform selection and fail-closed Stata test evidence. |
| Frozen governing files | Preserved | `AGENTS.md` SHA-256 is `7c43ce4128a993f1613fd129133c2d5df65eef70bc33bb4ebac4e2a4b430c07a`; official `stplugin.c` again matches its frozen SHA-256. No frozen reference was changed. |
| Python/portable/C checks | Passed locally | 72 Python tests plus shell, bytecode, TOML, strict C, Clang, and mock-SPI checks passed. The GNU/ELF mock-plugin audit is correctly skipped on macOS and remains required on Linux. |
| Rust formatting/lints | Passed locally | `cargo fmt --all -- --check` and workspace/all-target/all-feature Clippy with `-D warnings` passed. |
| Rust debug tests | Passed locally | 76 unit tests: 68 core, 5 plugin, and 3 testkit. |
| Rust release tests | Passed locally | The same 76 unit tests passed under `--release`; zero doc tests were present. |
| Rust FFI test | Passed locally | `cargo test -p stfixest-plugin --features ffi --lib`: 5 passed. |
| macOS universal plugin | Built and audited locally | The `x86_64` and `arm64` slices expose only `_pginit` and `_stata_call`, have only allowed OS dependencies, and have no unresolved symbols. |
| macOS ABI harness | Passed locally | The final universal artifact passed the host-native version/doctor/protocol/panic-containment harness. This does not execute the Intel slice on Intel hardware. |
| Original licensed-Stata smoke claim | Retracted | The loader selected Linux when native macOS Stata reported `c(os)==Unix`; the prior runner trusted process status and did not require proof that each fixture reached its final assertion. |
| Licensed Stata 18 installation/safety suite | Passed locally after platform and runner repair | Native arm64 Stata reports `Unix` / `Mac (Apple Silicon)`, loads `stfixest_macos.plugin`, and passes version, doctor at 32 requested threads, native/overall self-tests, both `r(498)` estimator checks without data-shape changes, and 100 reload/self-test calls. All nine current Stata fixtures emit exact success sentinels. |
| Rosetta x86-64 loader exercise | Passed locally, emulated | Rosetta Stata reports `Unix` / `Macintosh (Intel 64-bit)`, selects the universal macOS plugin, and passes the installation and 100-reload fixtures. This is not native Intel certification. |
| Licensed-Stata parser and parity suites | Parser fixtures passed; parity not run | Current parser and fail-closed postestimation fixtures pass under native arm64 Stata. Reference-command behavior, enabled estimation, and scientific parity remain open. |
| Linux x86-64 | Not built locally | Python audit tests exercise ELF rules, but no real Linux artifact was built, audited, or loaded. |
| Windows x86-64 | Not built locally | Python audit tests exercise PE/COFF rules, but no real Windows artifact was built, audited, or loaded. |
| GitHub Actions | Disabled by project policy | Workflows have been removed and repository Actions disabled. Cross-platform validation requires recorded target-native build, audit, loader, and licensed-Stata evidence. |
| `stfeols` | Unsupported / fail-closed | Licensed Stata and native tests observe `r(498)`; the smoke fixture confirms no observation/variable-count change and no estimate is posted. |
| `stfepois` | Unsupported / fail-closed | Licensed Stata and native tests observe `r(498)`; the smoke fixture confirms no observation/variable-count change and no estimate is posted. |
| Release status | Not certified | No cross-platform Stata, clean-machine, parity, performance, or memory certification exists. |

The exact validation record and toolchain caveat are in `VALIDATION.md`.
