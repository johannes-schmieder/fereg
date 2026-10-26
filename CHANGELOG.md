# Changelog

## 0.5.0 beta.6 — 2026-10-25

- OLS Auto with three or more categorical-intercept effects starts LSMR with a
  diagonal preconditioner and builds the additive Schwarz preconditioner only
  when the projected work is large. With three unit-weight effects, a short
  certified MAP pilot may finish first. Every route must pass the requested
  FE-moment certificate.
- On Linux, designs with four, eight and sixteen effects and a small third
  effect ran in about 0.60-0.68 of beta.5's time. One near-confounded
  three-effect design is slower (about 1.06-1.13 times); this is a documented
  exception.
- Results agree with beta.5 within the documented tolerances; sample, rank
  and omission decisions are unchanged. Iteration counts and the reported
  solver can differ. Two-effect, one-effect, slope, explicit-solver and PPML
  behavior is unchanged.

## 0.5.0 beta.5 — 2026-10-24

- Added Linux x86-64 and Windows x86-64 binaries alongside universal macOS.
- Tested native loading and installed OLS/PPML behavior on each platform.
- Updated installation instructions and preserved the actual build revision for
  each binary. Numerical calculations and solver selection are unchanged.

## 0.5.0 beta.4 — 2026-10-24

- OLS results identify the actual absorption and final residual-check methods.
  Two-effect Auto reports `cg` without multigrid and `cmgschur` when it applies
  a CMG preconditioner. A certified MAP continuation reports `map` and fallback.
- Added `e(fereg_solver_used)`, `e(fereg_solver_residual)` and
  `e(fereg_solver_fallback)`, readable matrix labels and normal-output diagnostics.
  The existing 2-by-16 result matrix and numeric codes remain compatible.
- Updated the help, user guide, architecture and protocol documentation to the
  accepted Auto defaults; removed superseded proposals and obsolete campaign text.
- Numerical calculations, solver selection, iteration budgets and PPML behavior
  are unchanged by this reporting correction.

The package supplies Linux x86-64, Windows x86-64 and universal macOS plugins.
Stata 18 or newer is required. See [installation](docs/installation.md) and
[validation](docs/VALIDATION.md) for supported behavior and checks.
