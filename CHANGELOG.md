# Changelog

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

The package supplies a universal macOS plugin, with licensed installation testing
on Apple Silicon. Stata 18 or newer is required. Linux and Windows binaries are
not included. See [installation](docs/installation.md) and
[validation](docs/VALIDATION.md) for supported behavior and checks.
