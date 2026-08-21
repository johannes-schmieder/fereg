# Implementation status

This is a source-complete Phase 0–1 handoff. The table separates implemented
source from artifacts that still require a target-native toolchain or licensed
Stata.

| Area | Status | Evidence / remaining gate |
|---|---:|---|
| Governing specification | Implemented | `AGENTS.md` is a byte-for-byte copy and its SHA-256 is locked in `references.lock.toml`. |
| Safe Rust core | Implemented; compilation pending here | Protocol, metadata, deterministic factor/tuple coding, checked memory preflight (including zero-column edge case), scoped threading, and native self-test. `stfixest-core` forbids unsafe code. |
| Rust/C panic boundary | Implemented; target-native execution pending here | `catch_unwind` contains the exported dispatcher; native build harnesses require the injected panic to return 3499. |
| C/SPI boundary | Implemented and GCC-tested | Official SPI 3.0 sources plus a minimal main-thread wrapper. Every wrapper is exercised by the mock SPI test. |
| Plugin exports/load path | Implemented and mock-loaded | A real shared-object mock proves `pginit`/`stata_call`, export hiding, dynamic loading, and ABI initialization on Linux. Native scripts repeat the same checks against the final Rust-linked plugin on every OS. |
| Stata 18+ diagnostics | Implemented; licensed-Stata test pending | Platform loader plus `version`, `doctor`, and `selftest`; 1/16/32-thread native paths are encoded. |
| Metadata-only Stata parser | Implemented prototype; licensed-Stata review pending | Uses `marksample, novarlist`, `fvexpand`, and `_ms_parse_parts`; creates no dummies, groups, or observation-scale FV expansions. |
| Linux x86-64 plugin | Build/CI/audit complete in source | Requires Rust on a glibc-2.28 target-native runner; final loader harness is automatic. |
| Windows x86-64 plugin | Build/CI/audit complete in source | Requires Rust MSVC and Visual Studio Build Tools; `/MT`/`crt-static` and final loader harness are automatic. |
| macOS universal plugin | Build/CI/audit complete in source | Requires Xcode; builds x86-64 and arm64 slices, joins with `lipo`, signs, audits, and loads the final artifact. |
| Cross-platform release ZIP | Implemented in CI | `stage_release.py` verifies all three plugin checksums before assembling one installable package. |
| `stfeols` estimation | Not implemented by assignment | Fails closed with `r(498)` and posts no estimate. |
| `stfepois` estimation | Not implemented by assignment | Fails closed with `r(498)` and posts no estimate. |

## Open Phase 0–1 exit gates

1. Run `cargo fmt`, clippy with warnings denied, all Rust tests, and final-plugin
   loader harnesses on target-native hosts.
2. Load each artifact in licensed Stata 18 or newer and run the installation,
   parser, and fail-closed postestimation fixtures.
3. Record exact hashes, dependencies, OS versions, Stata versions, and logs under
   `build_evidence/local/` before calling any binary a release artifact.
