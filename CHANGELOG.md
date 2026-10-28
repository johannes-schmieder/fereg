# Changelog

## Unreleased

- OLS Auto with three or more categorical-intercept effects no longer falls
  back to slow alternating projections on large sparse panels because of an
  overstated memory estimate: when the additive-Schwarz bound refuses LSMR,
  fereg counts the factor-pair components that need nested scratch exactly and
  admits against that bound. The ten-million-row sparse three-FE benchmark
  design now finishes in about 65 seconds (four workers) instead of not at
  all. Fits the old bound admitted are unchanged bit for bit; sample, rank and
  omission decisions are unchanged.
- The same exact count now also admits explicit `solver(lsmr)` (OLS, borrowed
  and owned designs), the automatic owned-design LSMR attempt and the explicit
  PPML LSMR preflight when the level-count bound refused them, instead of an
  `r(909)` refusal or the MAP fallback. Owned designs with slopes keep the
  level-count bound. Fits the old bound admitted are unchanged bit for bit.
- Many-FE OLS Auto builds the pinned solver's design (including its locality
  sort) once for the Jacobi trial and reuses it when the trial escalates to
  additive Schwarz. Results are bit-identical.
- Many-FE OLS Auto releases its column-major RHS transport once its LSMR
  sessions hold their own copies, and OLS Fast releases the projected matrix
  after the finite solve. The campaign sparse three-FE design peaks about
  100 MB lower at one million rows on the Mac and 1.3 GiB lower at ten million
  on Linux; results, routes and memory admission are unchanged.
- Developer: hidden `inject=routing_many_windows_v1` (replay wire and the
  hidden Stata adapter) qualifies bounded many-FE LSMR session windows, at
  most one live session per worker with the Jacobi escalation barrier at each
  window's end. Public Auto is unchanged.
 (beta.10 development)

- OLS with one fixed effect and the bounded three-FE MAP pilot skip the
  complete initial FE certificate when one exact group proves it fails, and
  scan their inputs for nonfinite values in parallel; results are
  bit-identical.
- OLS designs with more than 63 controls compute raw column moments in wide
  column bands and build their `[y, X]` matrix in row tiles instead of cell by
  cell, and accumulate their cross-products in a private buffer beyond 64
  columns, removing the jumps in run time at 63 to 65 controls; results are
  bit-identical, and the fast memory estimate counts the new buffers.
- Developer phase timing (`FEREG_PHASE_TIMING`) splits the MAP wrapper, dense
  projection setup, score certificate, PPML one-FE profile and loader pipeline.

## 0.5.0 beta.9 — 2026-10-28

- CMG `5c5b5b2b`: the multigrid hierarchy stops at 32 levels instead of 128,
  and every CMG memory admission assumes at most 32 levels. Pre-build CMG
  admission and the retained charge of two-FE OLS Auto shrink by about 474 MB
  and 1.3 GB on 1.4M- and 5.6M-row AKM panels, so a tight memory limit that
  refused the CMG preconditioner can now admit it. Hierarchies of at most 32
  levels (the recorded AKM hierarchies have 3) are bit-identical; a deeper
  one would stop at the cap with an iterative terminal inside the admitted
  memory.
- OLS Auto with two categorical-intercept effects builds its aggregated
  (worker, firm) cells on the worker team, bit for bit as the serial build,
  and constructs them before the diagonal pilot so the pilot's Schur products
  use the cell operator; the serial cell product and construction poll for
  Break once per block instead of once per cell. Pilot low bits change within
  the certified tolerances; sample, rank, omissions and every discrete decision
  are unchanged, and results stay independent of the worker count.
- CMG `f6a044c9`: a multigrid level's row-operator matvec runs on the worker
  team when its entries times lanes reach the parallel threshold, not only when
  its row count does, so dense coarse levels with few rows no longer run
  serially. Arithmetic-preserving; the stale `f45c3b8b` notices are corrected.
- PPML Auto with two fixed effects merges helper products, applies the finite
  cross block and forms per-frame level masses of its joint FE solves in
  parallel; results are bit-identical to the serial passes.
- A canonical PPML probe that finishes within its 32 steps is reused as the
  first column's solve instead of being repeated; results are unchanged.
- Developer phase timing (`FEREG_PHASE_TIMING`) reports PPML joint frames,
  canonical products, shared-CMG column solves and PPML absorptions.

## 0.5.0 beta.8 — 2026-10-27

- OLS Auto with two categorical-intercept effects applies its CMG
  preconditioner on the worker team (row operators and interleaved columns
  when memory admits them) and runs the aggregated cell operator in parallel,
  with results independent of the worker count.
- The multigrid hierarchy for two-effect designs no longer detaches weakly
  connected firms into singleton groups, so it coarsens in a few levels on
  worker-firm panels (OLS Auto, `solver(cmgschur)` and PPML reuse).
- A two-effect Auto fit whose diagonal recurrence has stalled builds the
  multigrid preconditioner at iteration 4, 8 or 12 instead of 16.
- Without `residuals()`, `xbd()`, `d()` or saved fixed effects, OLS in the
  default memory mode certifies its within residual directly instead of
  absorbing it a second time; `e(fereg_solver)` reports stop code 5.
- On synthetic Veneto-shaped AKM panels on an M2 Ultra, two-effect OLS Auto
  took 0.43-0.55 of its previous native time at 4 and 8 workers. Results
  agree within the documented tolerances; sample, rank and omission decisions
  are unchanged.

## 0.5.0 beta.7 — 2026-10-26

- PPML Auto with exactly two fixed effects reuses one CMG preconditioner across
  IRLS iterations, in the joint FE solves and in the canonical products of the
  first iteration and the strict checkpoint. A solve that is still slow at its
  checkpoint builds a missing hierarchy or rebuilds one from earlier weights;
  an estimation builds at most three.
- PPML publication fits enter the strict checkpoint only once the accepted
  predictor is predicted to settle within the strict envelope. Weakly connected
  designs pass their first checkpoint instead of repeating it at a 100-times
  tighter tolerance.
- Canonical PPML products solve small graph components directly and no longer
  assemble dense reduced matrices for iterative solves.
- PPML iterations with fixed effects make fewer, parallel passes: fresh input
  checks over fixed row blocks, residuals from projected columns, a one-pass
  Gram, and parallel refinement, product writes and coupling preparation.
- On Linux with 4 and 28 workers, two-effect designs with 2% worker mobility
  ran in 0.32-0.38 of beta.6's time at one and four million rows; AKM and
  dense two-effect designs ran in 0.73-0.84.
- Results agree with beta.6 within the documented tolerances; sample, rank,
  omission and separation decisions are unchanged. Iteration counts can
  differ. OLS is unchanged bit for bit.

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
