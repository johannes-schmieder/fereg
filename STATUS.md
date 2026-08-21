# Checkpoint A status

Checkpoint A has local macOS source/build evidence, but it is not merged as a
baseline-repair commit and is not a release candidate.

| Area | Status | Evidence / remaining gate |
|---|---:|---|
| Repository baseline | Merged | `main` and the repair branch start at PR #1 merge `2980a2eccc97fd4cefa51dd763d2146dcbdec089`. |
| Checkpoint A repairs | Uncommitted | The working tree restores the official `stplugin.c` hash/ownership contract, repairs bridge thread binding and guards, hardens binary audits, and makes the Linux mock-plugin audit platform-specific. |
| Frozen governing files | Preserved | `AGENTS.md` SHA-256 is `7c43ce4128a993f1613fd129133c2d5df65eef70bc33bb4ebac4e2a4b430c07a`; official `stplugin.c` again matches its frozen SHA-256. No frozen reference was changed. |
| Python/portable/C checks | Passed locally | 60 Python tests plus shell, bytecode, TOML, strict C, Clang, and mock-SPI checks passed. The GNU/ELF mock-plugin audit is correctly skipped on macOS and remains required on Linux. |
| Rust formatting/lints | Passed locally | `cargo fmt --all -- --check` and workspace/all-target/all-feature Clippy with `-D warnings` passed. |
| Rust debug tests | Passed locally | 76 unit tests: 68 core, 5 plugin, and 3 testkit. |
| Rust release tests | Passed locally | The same 76 unit tests passed under `--release`; zero doc tests were present. |
| Rust FFI test | Passed locally | `cargo test -p stfixest-plugin --features ffi --lib`: 5 passed. |
| macOS universal plugin | Built and audited locally | The `x86_64` and `arm64` slices expose only `_pginit` and `_stata_call`, have only allowed OS dependencies, and have no unresolved symbols. |
| macOS ABI harness | Passed locally | The final universal artifact passed the host-native version/doctor/protocol/panic-containment harness. This does not execute the Intel slice on Intel hardware. |
| Licensed Stata 18 installation/safety smoke | Passed locally after loader repair | The first attempt failed before plugin entry with `r(109)` because the loader called `real()` on numeric `c(stata_version)`. After comparing `c(stata_version)` directly, Stata 18 MP passed version, doctor at 32 requested threads, native/overall self-tests, both fail-closed estimator checks, and 100 reload/self-test calls. |
| Licensed-Stata parser and parity suites | Not run | Metadata semantics, reference-command behavior, enabled estimation, postestimation, and scientific parity remain open. |
| Linux x86-64 | Not built locally | Python audit tests exercise ELF rules, but no real Linux artifact was built, audited, or loaded. |
| Windows x86-64 | Not built locally | Python audit tests exercise PE/COFF rules, but no real Windows artifact was built, audited, or loaded. |
| GitHub Actions | No Checkpoint A evidence | Actions was only recently enabled; there is no baseline-repair commit or associated job set yet. |
| `stfeols` | Unsupported / fail-closed | Licensed Stata and native tests observe `r(498)`; the smoke fixture confirms no observation/variable-count change and no estimate is posted. |
| `stfepois` | Unsupported / fail-closed | Licensed Stata and native tests observe `r(498)`; the smoke fixture confirms no observation/variable-count change and no estimate is posted. |
| Release status | Not certified | No cross-platform Stata, clean-machine, parity, performance, or memory certification exists. |

The exact validation record and toolchain caveat are in `VALIDATION.md`.
